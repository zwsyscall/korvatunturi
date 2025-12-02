use crate::cache::{FileContent, core::FileCache};
use actix_web::{HttpResponse, http::header, web};
use futures_util::TryStreamExt;
use tokio_util::io::ReaderStream;

pub async fn download(cache: web::Data<FileCache>, path: web::Path<String>) -> actix_web::Result<HttpResponse> {
    let file = path.into_inner();
    if let Ok((filename, data)) = cache.fetch_file(&file).await {
        match data {
            FileContent::InMemory(bytes) => {
                return Ok(HttpResponse::Ok()
                    .insert_header((header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)))
                    .content_type("application/octet-stream")
                    .body(bytes));
            }
            FileContent::OnDisk(reader) => {
                let stream = ReaderStream::new(reader).map_err(actix_web::error::ErrorInternalServerError);
                return Ok(HttpResponse::Ok()
                    .insert_header((header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)))
                    .content_type("application/octet-stream")
                    .streaming(stream));
            }
        }
    }
    return Ok(HttpResponse::NotFound().finish());
}
