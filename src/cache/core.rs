use super::{
    entry::{CacheEntry, CacheEntryRow},
    settings::CacheSettings,
};
use log::{debug, error, info, trace, warn};
use sqlx::sqlite::SqlitePoolOptions;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::time::interval;
use tokio::{
    fs::read_dir,
    sync::{RwLock, mpsc},
};
use tokio::{select, time::Interval};

pub(super) enum SignalAction {
    Delete,
    Save,
    #[allow(unused)]
    Accessed,
}

#[derive(Debug)]
pub enum FileCacheError {
    NotFound,
    BackingFileMissing,
    NoSpaceLeftOnDevice,
    #[allow(unused)]
    IoError(std::io::Error),
    #[allow(unused)]
    DbError(sqlx::Error),
}

pub struct FileCache {
    // Path of the file storage
    pub(super) library: PathBuf,
    // UUID -> CacheEntry
    pub(super) cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    pub(super) sync: mpsc::Sender<(String, SignalAction)>,
    pub(super) cache_settings: CacheSettings,
    pub max_size: usize,
}

impl FileCache {
    pub async fn new(cache_settings: CacheSettings, library_path: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new().max_connections(1).connect(&cache_settings.database_path).await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS cache (
                uuid TEXT NOT NULL PRIMARY KEY,
                filename TEXT NOT NULL,
                expiration_utc TEXT NOT NULL,
                burn_after_read INTEGER NOT NULL,
                read_count INTEGER NOT NULL
            )
        "#,
        )
        .execute(&pool)
        .await?;
        debug!("sqlite table initialized");

        // Initial feed
        let rows: Vec<CacheEntryRow> = sqlx::query_as(
            r#"
            SELECT uuid, filename, expiration_utc, burn_after_read, read_count
            FROM cache
            "#,
        )
        .fetch_all(&pool)
        .await?;

        let mut cache = HashMap::new();
        for row in rows {
            let (uuid, entry) = row.into();
            if !entry.is_expired() {
                cache.insert(uuid, entry);
            }
        }
        debug!("Cache entries populated");

        // Cache cleanup in case we have orphaned data
        info!("Cleaning up orphaned files");
        let mut paths = read_dir(&library_path).await?;
        while let Some(file) = paths.next_entry().await? {
            if let Some(filename) = file.file_name().to_str() {
                if !cache.contains_key(filename) {
                    if let Err(e) = Self::delete_file(&library_path.into(), filename).await {
                        error!("Error deleting file: {}", e)
                    }
                }
            }
        }

        // Internal queues for sending and receiving events
        let (alert_sender, mut alert_receiver) = mpsc::channel::<(String, SignalAction)>(3000);

        let shared_cache = Arc::new(RwLock::new(cache));
        // Background routines
        tokio::spawn({
            let cache = shared_cache.clone();
            let library = library_path.to_string().into();

            let mut file_interval: Interval = interval(cache_settings.file_cleanup_interval);
            let mut cache_interval: Interval = interval(cache_settings.cache_cleanup_interval);
            async move {
                info!("Starting background routines");
                loop {
                    select! {
                        biased;
                            // Channels
                            maybe = alert_receiver.recv() => {
                                if let Some((uuid, action)) = maybe {
                                    match action {
                                        SignalAction::Delete => {
                                            let remove = {
                                                let mut rw_lock = cache.write().await;
                                                rw_lock.remove(&uuid).is_some()
                                            };

                                            if remove {
                                                if let Err(e) = Self::drop_item(&uuid, &library, &pool).await {
                                                    warn!("Error dropping file: {:#?}", e)
                                                }
                                            }
                                        }
                                        SignalAction::Save => {
                                            let lock = cache.read().await;
                                            if let Some(entry) = lock.get(&uuid) {
                                                if !entry.is_expired() {
                                                    if let Err(e) = Self::push_to_db(&pool, &uuid, &entry).await {
                                                        error!("Failed to save {} to DB: {e}", uuid);
                                                    }
                                                }
                                            }
                                        }
                                        SignalAction::Accessed => {
                                            let mut rw_lock = cache.write().await;
                                            let mut burn_after_read = false;

                                            if let Some(entry) = rw_lock.get_mut(&uuid) {
                                                entry.read_count += 1;
                                                burn_after_read = entry.burn_after_read;
                                            }

                                            if burn_after_read {
                                                if rw_lock.remove(&uuid).is_some() {
                                                    if let Err(e) = Self::drop_item(&uuid, &library, &pool).await {
                                                        warn!("Error dropping file: {:#?}", e)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // File cleanup
                            _ = file_interval.tick() => {
                                trace!("Starting file cache maintenance routine.");
                                let mut expired_entries = Vec::new();
                                // Fetch expired entries
                                {
                                    let lock = cache.read().await;
                                    for (uuid, entry) in lock.iter() {
                                        if entry.is_expired() {
                                            expired_entries.push(uuid.to_string());
                                            continue;
                                        }
                                    }
                                }
                                // Remove expired entries
                                {
                                    let mut rw_lock = cache.write().await;
                                    for uuid in &expired_entries {
                                        debug!("Removing {} from cache", &uuid);
                                        rw_lock.remove(uuid);
                                    }
                                }
                                for uuid in &expired_entries {
                                    if let Err(e) = Self::drop_item(&uuid, &library, &pool).await {
                                        warn!("Error dropping file: {:#?}", e)
                                    }
                                }
                        }
                            // In memory cache cleanup
                            _ = cache_interval.tick() => {
                                trace!("Starting cache maintenance routine.");

                                let mut rw_lock = cache.write().await;
                                for (uuid, entry) in rw_lock.iter_mut() {
                                    if entry.flush(cache_settings.in_memory_ttl) {
                                        debug!("Flushed {} from cache", uuid);
                                    }
                                }
                            }
                    }
                }
            }
        });

        Ok(Self {
            cache: shared_cache,
            sync: alert_sender,
            library: library_path.into(),
            max_size: cache_settings.maximum_size.clone(),
            cache_settings: cache_settings,
        })
    }
}
