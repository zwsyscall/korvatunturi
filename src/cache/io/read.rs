use crate::{cache::entry::CacheEntry, signal};

use super::super::core::{FileCache, FileCacheError, SignalAction};
use log::error;
use std::{path::PathBuf, sync::Arc};
use tokio::fs::read;

impl FileCache {
    // Disk -> data
    pub(in super::super) async fn fetch_disk(library: &PathBuf, filename: &str) -> Option<Arc<[u8]>> {
        let file_path = library.join(filename);
        let data = read(&file_path).await.ok()?.into();
        Some(data)
    }

    pub async fn fetch_file(&self, uuid: &str) -> Result<(String, Arc<[u8]>), FileCacheError> {
        {
            let cache = self.cache.read().await;
            match cache.get(uuid) {
                Some(entry) => {
                    if entry.is_expired() {
                        signal!(self, uuid, SignalAction::Delete);
                        return Err(FileCacheError::NotFound);
                    }
                    // Cache hit:
                    if let Some(data) = &entry.data {
                        signal!(self, uuid, SignalAction::Accessed);
                        return Ok((entry.upload_name.to_string(), data.clone()));
                    }
                }
                None => return Err(FileCacheError::NotFound),
            }
        }

        // Cache miss route
        if let Some(data) = Self::fetch_disk(&self.library, &uuid).await {
            let mut cache = self.cache.write().await;
            let entry = cache.get_mut(uuid).ok_or(FileCacheError::NotFound)?;
            signal!(self, uuid, SignalAction::Accessed);

            // if it's been read already we can just return that data
            if let Some(d) = &entry.data {
                return Ok((entry.upload_name.to_string(), d.clone()));
            }

            entry.update(data.clone());
            return Ok((entry.upload_name.to_string(), data));
        }
        error!("Backed file is missing despite entry being present in database ");
        // We want to probaly drop the entry from the cache if we land in this branch
        Err(FileCacheError::BackingFileMissing)
    }

    pub async fn fetch_entries(&self) -> Vec<CacheEntry> {
        let lock = self.cache.read().await;
        lock.iter().map(|(_uuid, entry)| (*entry).clone()).collect()
    }
}
