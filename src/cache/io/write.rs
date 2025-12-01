use crate::{cache::entry::FileOptions, signal};

use super::super::{
    core::{FileCache, FileCacheError, SignalAction},
    entry::CacheEntry,
};
use chrono::Utc;
use log::{debug, error};
use std::io;
use std::time::Duration;
use std::{path::PathBuf, time::Instant};
use tokio::fs::remove_file;
use tokio::fs::write;
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
        let expiration_utc = {
            let diff = match entry.expiration.checked_duration_since(Instant::now()) {
                Some(d) => d,
                None => Duration::from_secs(0),
            };

            Utc::now() + chrono::Duration::from_std(diff).unwrap_or_default()
        };

        sqlx::query(
            r#"
        INSERT INTO cache (uuid, filename, expiration_utc, burn_after_read, read_count)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        )
        .bind(uuid)
        .bind(&entry.upload_name)
        .bind(expiration_utc)
        .bind(&entry.burn_after_read)
        .bind(entry.read_count as i64)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn upload_file(&self, bytes: Vec<u8>, filename: &str, upload_options: FileOptions) -> Result<String, FileCacheError> {
        // Generate UUID
        let entry_uuid = Uuid::new_v4().to_string();
        let filepath = self.library.join(&entry_uuid);

        // Write the file
        if let Err(e) = write(&filepath, &bytes).await {
            if e.kind() == std::io::ErrorKind::StorageFull {
                return Err(FileCacheError::NoSpaceLeftOnDevice);
            }
            return Err(FileCacheError::IoError(e));
        }

        // Fetch potential settings
        let ttl = upload_options.expires_in.map(|s| Duration::from_secs(s)).unwrap_or(self.cache_settings.on_disk_ttl);
        let burn_after_read = upload_options.burn_after_read.unwrap_or(false);

        let entry = CacheEntry::new(filename, Some(bytes.into()), burn_after_read, ttl);

        {
            let mut cache = self.cache.write().await;
            cache.insert(entry_uuid.to_string(), entry);
        }

        signal!(self, entry_uuid, SignalAction::Save);
        Ok(entry_uuid)
    }
}
