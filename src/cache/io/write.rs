use crate::{cache::entry::FileOptions, signal};

use super::super::{
    core::{FileCache, FileCacheError, SignalAction},
    entry::CacheEntry,
    instant_to_datetime,
};
use log::{debug, error};
use std::io;
use std::path::PathBuf;
use tokio::fs::remove_file;
use tokio::fs::write;
use tokio::time::Duration;
use uuid::Uuid;

/// Write
impl FileCache {
    pub(in super::super) async fn delete_file(library: &PathBuf, uuid: &str) -> Result<(), io::Error> {
        let filepath = library.join(uuid);
        debug!("Deleting file: {}", filepath.to_str().unwrap_or("<Unable to display nonunicode path>"));
        remove_file(filepath).await
    }

    pub(in super::super) async fn drop_item(uuid: &str, library: &PathBuf, pool: &sqlx::Pool<sqlx::Sqlite>) -> Result<(), FileCacheError> {
        Self::delete_file(library, uuid).await.map_err(|e| FileCacheError::IoError(e))?;
        Self::delete_from_db(pool, uuid).await.map_err(|e| FileCacheError::DbError(e))?;
        Ok(())
    }

    pub async fn delete_from_db(pool: &sqlx::Pool<sqlx::Sqlite>, uuid: &str) -> Result<(), sqlx::Error> {
        debug!("Deleting {} from persistent database", uuid);
        sqlx::query(
            r#"
            DELETE FROM cache
            WHERE uuid = ?
        "#,
        )
        .bind(uuid)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn push_to_db(pool: &sqlx::Pool<sqlx::Sqlite>, uuid: &str, entry: &CacheEntry) -> Result<(), sqlx::Error> {
        let expiration_utc = instant_to_datetime(&entry.expiration);

        sqlx::query(
            r#"
        INSERT INTO cache (uuid, filename, expiration_utc, burn_after_read, read_count, file_size)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        )
        .bind(uuid)
        .bind(&entry.upload_name)
        .bind(expiration_utc)
        .bind(&entry.burn_after_read)
        .bind(entry.read_count)
        .bind(entry.len)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn upload_file(&self, bytes: Vec<u8>, filename: &str, upload_options: FileOptions) -> Result<String, FileCacheError> {
        // Generate UUID
        let entry_uuid = Uuid::new_v4().to_string();
        let filepath = self.library.join(&entry_uuid);
        // can panic
        let len = bytes.len() as i64;

        // Write the file
        if let Err(e) = write(&filepath, bytes).await {
            if e.kind() == std::io::ErrorKind::StorageFull {
                return Err(FileCacheError::NoSpaceLeftOnDevice);
            }
            return Err(FileCacheError::IoError(e));
        }

        // Extract entry specific settings
        let ttl = upload_options.expires_in.map(|s| Duration::from_secs(s)).unwrap_or(self.cache_settings.on_disk_ttl);
        let burn_after_read = upload_options.burn_after_read.unwrap_or(false);

        // This can panic
        let entry = CacheEntry::new(filename, None, len, burn_after_read, ttl);

        {
            let mut cache = self.cache.write().await;
            cache.insert(entry_uuid.to_string(), entry);
        }

        signal!(self, entry_uuid, SignalAction::NewFile);
        Ok(entry_uuid)
    }
}
