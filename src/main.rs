mod api;
mod cache;
mod frontend;
mod settings;
use crate::cache::{FileCache, settings::CacheSettings};
use crate::settings::Configuration;
use actix_web::{App, HttpServer, middleware::Logger, web};
use ipnet::IpNet;
use log::{debug, warn};
use log::{error, info};
use std::process::exit;
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Set up logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    info!("Loading configuration");
    let config = match Configuration::load(settings::config_path()) {
        Ok(c) => c,
        Err(e) => {
            error!("Error loading configuration: {:#?}", e);
            exit(1)
        }
    };
    debug!("Loaded config: {:#?}", config);

    // This can technically delay panic
    let cache = match FileCache::new(CacheSettings::from(&config.cache), &config.cache_path).await {
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

    // Set up the actix-web logging format
    let base = "\"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T";
    let logging_format = if let Some(h) = config.forward_header.as_ref() { format!("%{{{}}}i {}", h, base) } else { format!("%{{r}}a {}", base) };

    // Middlewear & shared data
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
