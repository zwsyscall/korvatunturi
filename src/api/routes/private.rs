use crate::cache::{FileOptions, core::FileCache};
use actix_multipart::Multipart;
use actix_web::{HttpResponse, web};
use futures_util::StreamExt as _;
use log::trace;
use tokio::time::Instant;

pub async fn upload(cache: web::Data<FileCache>, mut query: web::Query<FileOptions>, mut payload: Multipart) -> actix_web::Result<HttpResponse> {
    while let Some(item) = payload.next().await {
        let max_size = cache.max_size.clone();
        let mut field = item?;
        let mut bytes = Vec::new();
        let read_start = Instant::now();

        while let Some(Ok(data)) = field.next().await {
            bytes.extend(data);
            if bytes.len() > max_size {
                break;
            }
        }
        trace!("Entire fileread took {:#3?}", read_start.elapsed());

        let upload_start = Instant::now();
        let filename = query.filename.take().unwrap_or(field.content_disposition().map(|f| f.get_filename().unwrap_or("upload.bin")).unwrap_or("upload.bin").to_string());
        if let Ok(uuid) = cache.upload_file(bytes, &filename, query.0).await {
            trace!("Upload / write took {:#3?}", upload_start.elapsed());

            return Ok(HttpResponse::Ok().body(uuid));
        }
        break;
    }

    Ok(HttpResponse::InternalServerError().finish())
}

pub async fn status(cache: web::Data<FileCache>) -> actix_web::Result<HttpResponse> {
    let entries = cache.fetch_entries().await;
    Ok(HttpResponse::Ok().json(entries))
}
