use crate::cache::core::FileCache;
use actix_web::{HttpResponse, http::header, web};
use bytes::Bytes;

pub async fn download(cache: web::Data<FileCache>, path: web::Path<String>) -> actix_web::Result<HttpResponse> {
    let file = path.into_inner();
    if let Ok((filename, data)) = cache.fetch_file(&file).await {
        return Ok(HttpResponse::Ok()
            .insert_header((header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)))
            .content_type("application/octet-stream")
            .body(Bytes::from(data.to_vec())));
    }
    return Ok(HttpResponse::NotFound().finish());
}
