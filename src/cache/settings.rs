use std::time::Duration;

#[derive(Clone)]
pub struct CacheSettings {
    pub in_memory_ttl: Duration,
    pub cache_cleanup_interval: Duration,
    pub on_disk_ttl: Duration,
    pub file_cleanup_interval: Duration,
    pub database_path: String,
    pub maximum_size: usize,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            in_memory_ttl: Duration::from_secs(30),
            on_disk_ttl: Duration::from_secs(60),
            file_cleanup_interval: Duration::from_secs(10),
            cache_cleanup_interval: Duration::from_secs(5),
            database_path: ":memory:".to_string(),
            maximum_size: 200_000_000,
        }
    }
}

impl From<&crate::settings::CacheConfig> for CacheSettings {
    fn from(conf: &crate::settings::CacheConfig) -> Self {
        Self {
            in_memory_ttl: Duration::from_secs(conf.in_memory_ttl as u64),
            cache_cleanup_interval: Duration::from_secs(conf.cache_cleanup_interval as u64),
            on_disk_ttl: Duration::from_secs(conf.on_disk_ttl as u64),
            file_cleanup_interval: Duration::from_secs(conf.file_cleanup_interval as u64),
            database_path: conf.database_path.clone(),
            maximum_size: conf.maximum_size,
        }
    }
}
