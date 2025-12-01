mod api;
mod cache;
mod frontend;
mod settings;
use crate::cache::{FileCache, settings::CacheSettings};
use crate::settings::Configuration;
use actix_web::{App, HttpServer, middleware::Logger, web};
use ipnet::IpNet;
use log::error;
use log::{debug, warn};
use std::process::exit;
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Set up logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Can panic inside configuration load, it's more idioimatic to panic inside there than to create a generic error type to return which to panic on
    debug!("Loading configuration file.");
    let config = Configuration::load(settings::config_path());
    debug!("Loaded config: {:#?}", config);
    let cache_config = CacheSettings::from(&config.cache);

    // This can technically delay panic
    let cache = match FileCache::new(cache_config, &config.cache_path).await {
        Ok(c) => c,
        Err(e) => {
            error!("Error initializing cache: {}", e);
            exit(1)
        }
    };

    let mut whitelist_list = Vec::new();
    for ip_range in config.ip_whitelist.iter() {
        if let Ok(range) = ip_range.parse::<IpNet>() {
            whitelist_list.push(range)
        } else {
            warn!("Error compiling {} to a valid range, ignoring entry", ip_range);
        }
    }

    let logging_format = match &config.forward_header {
        Some(h) => format!("%{{{}}}i \"%r\" %s %b \"%{{Referer}}i\" \"%{{User-Agent}}i\" %T", h),
        None => "%{r}a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T".to_string(),
    };

    let whitelist = api::middleware::IpWhitelist::new(whitelist_list, config.forward_header);
    let server_info = Arc::new((config.service_name, config.source_code));
    let cache_data = web::Data::new(cache);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::new(&logging_format))
            .app_data(web::Data::from(server_info.clone()))
            .default_service(web::to(frontend::not_found))
            .route("/", web::get().to(frontend::index))
            .route("/favicon.ico", web::get().to(frontend::favicon))
            .route("/index.html", web::get().to(frontend::index))
            .service(web::resource("/upload").wrap(whitelist.clone()).route(web::get().to(frontend::upload)))
            .service(
                web::scope("/api")
                    .app_data(cache_data.clone())
                    .route("/download/{id}", web::get().to(api::public::download))
                    .service(web::resource("/status").wrap(whitelist.clone()).route(web::get().to(api::private::status)))
                    .service(web::resource("/upload").wrap(whitelist.clone()).route(web::post().to(api::private::upload))),
            )
    })
    .bind((config.host, config.port))?
    .run()
    .await
}
