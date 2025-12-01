use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use sqlx::FromRow;
use std::time::Duration;
use std::{sync::Arc, time::Instant};

#[derive(Deserialize, Clone)]
pub struct FileOptions {
    pub expires_in: Option<u64>,
    pub filename: Option<String>,
    pub burn_after_read: Option<bool>,
}

#[derive(Serialize, Clone)]
pub struct CacheEntry {
    pub(super) upload_name: String,
    #[serde(skip_serializing)]
    pub(super) accessed: Instant,

    #[serde(skip_serializing)]
    pub(super) data: Option<Arc<[u8]>>,
    #[serde(skip_serializing)]
    pub(super) expiration: Instant,
    pub(super) burn_after_read: bool,
    pub(super) read_count: i64,
}

#[derive(FromRow)]
pub struct CacheEntryRow {
    uuid: String,
    filename: String,
    expiration_utc: DateTime<Utc>,
    burn_after_read: i8,
    read_count: i64,
}

impl From<CacheEntryRow> for (String, CacheEntry) {
    fn from(row: CacheEntryRow) -> Self {
        let now_utc = Utc::now();
        let now_instant = Instant::now();
        let expiration = {
            if row.expiration_utc > now_utc {
                let diff = (row.expiration_utc - now_utc).to_std().unwrap_or_default();
                now_instant + diff
            } else {
                now_instant
            }
        };

        let entry = CacheEntry {
            upload_name: row.filename,
            accessed: Instant::now(),
            data: None,
            burn_after_read: row.burn_after_read == 1,
            read_count: row.read_count,
            expiration: expiration,
        };
        (row.uuid, entry)
    }
}

impl CacheEntry {
    pub(super) fn new(name: &str, data: Option<Arc<[u8]>>, burn_after_read: bool, ttl: Duration) -> Self {
        Self {
            upload_name: name.to_string(),
            accessed: Instant::now(),
            data: data,
            burn_after_read: burn_after_read,
            expiration: Instant::now() + ttl,
            read_count: 0,
        }
    }

    pub(super) fn update(&mut self, data: Arc<[u8]>) {
        self.accessed = Instant::now();
        self.data = Some(data)
    }

    pub(super) fn is_expired(&self) -> bool {
        self.expiration < Instant::now() || self.burn_after_read && self.read_count > 0
    }

    pub(super) fn flush(&mut self, cache_ttl: Duration) -> bool {
        if Instant::now() - self.accessed >= cache_ttl {
            return self.data.take().is_some();
        }
        false
    }
}
