use actix_web::{HttpRequest, HttpResponse, web};
use askama::Template;
use log::error;

#[derive(Template)]
#[template(path = "main.html.j2", ext = "html")]
struct MainPage<'a> {
    server_name: &'a str,
    source: &'a str,
}

#[derive(Template)]
#[template(path = "upload.html.j2", ext = "html")]
struct UploadPage<'a> {
    server_name: &'a str,
    source: &'a str,
    server_url: &'a str,
}

#[derive(Template)]
#[template(path = "not_found.html.j2", ext = "html")]
struct NotFound<'a> {
    server_name: &'a str,
}

#[derive(Template)]
#[template(path = "forbidden.html.j2", ext = "html")]
pub struct Forbidden<'a> {
    pub server_name: &'a str,
}

pub async fn index(data: web::Data<(String, String)>) -> actix_web::Result<HttpResponse> {
    let page = MainPage { server_name: &data.0, source: &data.1 };
    Ok(match page.render() {
        Ok(page) => HttpResponse::Ok().body(page),
        Err(e) => {
            error!("error templating: {}", e);
            HttpResponse::InternalServerError().body("Error templating gallery page")
        }
    })
}

pub async fn upload(req: HttpRequest, data: web::Data<(String, String)>) -> actix_web::Result<HttpResponse> {
    let conn_info = req.connection_info();
    let base_url = format!("{}://{}", conn_info.scheme(), conn_info.host());

    let page = UploadPage {
        server_name: &data.0,
        source: &data.1,
        server_url: &base_url,
    };
    Ok(match page.render() {
        Ok(page) => HttpResponse::Ok().body(page),
        Err(e) => {
            error!("error templating: {}", e);
            HttpResponse::InternalServerError().body("Error templating gallery page")
        }
    })
}

pub async fn not_found(data: web::Data<(String, String)>) -> actix_web::Result<HttpResponse> {
    let page = NotFound { server_name: &data.0 };
    Ok(match page.render() {
        Ok(page) => HttpResponse::NotFound().body(page),
        Err(e) => {
            error!("error templating: {}", e);
            HttpResponse::InternalServerError().body("Error templating gallery page")
        }
    })
}

pub async fn favicon() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok().content_type("image/vnd.microsoft.icon").body(&include_bytes!("../../../templates/favicon.ico")[..]))
}
