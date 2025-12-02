use crate::{cache::entry::CacheEntry, signal};

use super::super::core::{FileCache, FileCacheError, SignalAction};
use bytes::Bytes;
use log::{debug, error};
use std::path::PathBuf;
use tokio::{
    fs::{File, read},
    io::BufReader,
};

pub enum FileContent {
    InMemory(Bytes),
    OnDisk(BufReader<File>),
}

impl FileCache {
    // Disk -> data
    pub async fn fetch_to_memory(library: &PathBuf, filename: &str) -> Option<Bytes> {
        let file_path = library.join(filename);
        let data = read(&file_path).await.ok()?.into();
        Some(data)
    }

    pub async fn fetch_reader(library: &PathBuf, filename: &str) -> Option<BufReader<File>> {
        let file_path = library.join(filename);
        let file = File::open(file_path).await.ok()?;
        Some(BufReader::new(file))
    }

    pub async fn fetch_file(&self, uuid: &str) -> Result<(String, FileContent), FileCacheError> {
        // Cache hit route:
        let (size, filename) = {
            let cache = self.cache.read().await;
            match cache.get(uuid) {
                Some(entry) => {
                    // The entry has expired
                    if entry.is_expired() {
                        debug!("Cache hit but expired item");
                        // Return not found and signal to Db to delete the file
                        signal!(self, uuid, SignalAction::Delete);
                        return Err(FileCacheError::NotFound);
                    }

                    // Cache hit and we found the item in memory
                    if let Some(data) = &entry.data {
                        debug!("Cache hit");
                        // Signal to Db that it's been accessed
                        signal!(self, uuid, SignalAction::Accessed);
                        // Return data
                        return Ok((entry.upload_name.to_string(), FileContent::InMemory(data.clone())));
                    }
                    (entry.len, entry.upload_name.to_string())
                }
                None => return Err(FileCacheError::NotFound),
            }
        };

        let space_left = {
            let mut mem_rw = self.cache_mem.write().await;
            mem_rw.reserve(size as usize).is_some()
        };

        // if we have the memory to spare, we can load it to the cache
        // can panic
        if space_left {
            debug!("Cache miss but enough space to load to memory");
            // Cache miss route
            if let Some(data) = Self::fetch_to_memory(&self.library, &uuid).await {
                let mut cache = self.cache.write().await;
                if let Some(entry) = cache.get_mut(uuid) {
                    signal!(self, &uuid, SignalAction::Accessed);

                    // if it's been read already we can just return that data without overwriting the memory
                    if let Some(d) = &entry.data {
                        // Early "free" since another thread has also allocated the memory
                        let mut mem_rw = self.cache_mem.write().await;
                        mem_rw.free(size as usize);

                        return Ok((filename, FileContent::InMemory(d.clone())));
                    }

                    entry.update(data.clone());
                    return Ok((filename, FileContent::InMemory(data)));
                }
            }
            // Ensure we don't magically force bloat into the cache size param
            let mut mem_rw = self.cache_mem.write().await;
            mem_rw.free(size as usize);
            return Err(FileCacheError::NotFound);
        } else {
            // We can't spare the memory so instead we return a reader object
            if let Some(reader) = Self::fetch_reader(&self.library, &uuid).await {
                debug!("Cache miss and not enough ram, returning reader");
                return Ok((filename, FileContent::OnDisk(reader)));
            }
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
