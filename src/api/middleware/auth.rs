use actix_web::body::EitherBody;
use actix_web::http::StatusCode;
use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use actix_web::{HttpResponse, web};
use askama::Template;
use ipnet::IpNet;
use log::{debug, warn};
use std::future::{Ready, ready};
use std::str::FromStr;
use std::sync::Arc;
use std::{collections::HashSet, future::Future, pin::Pin};

use crate::frontend::Forbidden;

fn extract_client_ip(req: &ServiceRequest, header: &str) -> Option<String> {
    if let Some(forwarded) = req.headers().get(header) {
        if let Ok(val) = forwarded.to_str() {
            return Some(val.to_string());
        }
    }
    None
}

#[derive(Clone)]
pub struct IpWhitelist {
    allowed: Arc<HashSet<IpNet>>,
    forwarded_header: Arc<Option<String>>,
}

impl IpWhitelist {
    pub fn new<I>(ips: I, header: Option<String>) -> Self
    where
        I: IntoIterator<Item = IpNet>,
    {
        Self {
            allowed: Arc::new(ips.into_iter().collect()),
            forwarded_header: Arc::new(header),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for IpWhitelist
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AccessControlMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AccessControlMiddleware {
            service,
            allowed: self.allowed.clone(),
            header: self.forwarded_header.clone(),
        }))
    }
}

pub struct AccessControlMiddleware<S> {
    service: S,
    allowed: Arc<HashSet<IpNet>>,
    header: Arc<Option<String>>,
}

impl<S, B> Service<ServiceRequest> for AccessControlMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let allowed = self.allowed.clone();
        let server_info = req.app_data::<web::Data<(String, String)>>().cloned();

        // Cursed :-)
        let remote_conn = match &*self.header {
            Some(h) => extract_client_ip(&req, h),
            None => Some(req.connection_info().peer_addr().unwrap_or("<no_ip_found>").to_string()),
        };

        if let Some(remote_ip) = &remote_conn {
            if let Ok(ip) = std::net::IpAddr::from_str(&remote_ip) {
                if allowed.iter().any(|range| range.contains(&ip)) {
                    debug!("{}: in whitelist", ip);
                    let fut = self.service.call(req);
                    return Box::pin(async move {
                        let res = fut.await?;
                        Ok(res.map_into_left_body())
                    });
                }
            }
        }

        if let Some(data) = server_info {
            let page = Forbidden { server_name: &data.0 };
            if let Ok(page) = page.render() {
                return Box::pin(async move {
                    let (req, _pl) = req.into_parts();
                    let res = HttpResponse::build(StatusCode::FORBIDDEN).body(page);
                    Ok(ServiceResponse::new(req, res).map_into_right_body())
                });
            }
        }

        // Boring placeholder
        return Box::pin(async move {
            warn!("{:#?}: Missing / invalid IP", remote_conn);

            let (req, _pl) = req.into_parts();
            let res = HttpResponse::build(StatusCode::FORBIDDEN).body("Forbidden");
            Ok(ServiceResponse::new(req, res).map_into_right_body())
        });
    }
}
