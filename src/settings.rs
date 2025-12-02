use serde::Deserialize;
use std::fs;

pub fn config_path() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        return ".\\config.toml";
    }
    #[cfg(target_os = "linux")]
    {
        return "/etc/korvatunturi-box/config.toml";
    }
}

#[derive(Debug)]
pub enum ConfigError {
    #[allow(dead_code)]
    IoError(std::io::Error),
    #[allow(dead_code)]
    TomlError(toml::de::Error),
}

impl Configuration {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path).map_err(|e| ConfigError::IoError(e))?;
        toml::from_str::<Configuration>(&content).map_err(|e| ConfigError::TomlError(e))
    }
}

#[derive(Debug, Deserialize)]
pub struct Configuration {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_service_name")]
    pub service_name: String,

    #[serde(default = "default_source")]
    pub source_code: String,

    #[serde(default = "default_whitelist")]
    pub ip_whitelist: Vec<String>,

    #[serde(default = "default_cache_path")]
    pub cache_path: String,

    #[serde(default)]
    pub cache: CacheConfig,

    #[serde(default)]
    pub forward_header: Option<String>,
}

fn default_port() -> u16 {
    8080
}
fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_service_name() -> String {
    "Korvatunturi".into()
}
fn default_source() -> String {
    "https://github.com/zwsyscall/korvatunturi".into()
}
fn default_whitelist() -> Vec<String> {
    vec!["10.0.0.0/8".into()]
}
fn default_cache_path() -> String {
    #[cfg(target_os = "windows")]
    {
        ".\\cache".into()
    }
    #[cfg(target_os = "linux")]
    {
        "/tmp".into()
    }
}

#[derive(Debug, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_in_memory_ttl")]
    pub in_memory_ttl: usize,

    #[serde(default = "default_cache_cleanup_interval")]
    pub cache_cleanup_interval: usize,

    #[serde(default = "default_on_disk_ttl")]
    pub on_disk_ttl: usize,

    #[serde(default = "default_file_cleanup_interval")]
    pub file_cleanup_interval: usize,

    #[serde(default = "default_database_path")]
    pub database_path: String,

    #[serde(default = "default_maximum_size")]
    pub max_item_size: usize,

    #[serde(default = "default_maximum_size")]
    pub max_cache_memory: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            in_memory_ttl: default_in_memory_ttl(),
            cache_cleanup_interval: default_on_disk_ttl(),
            on_disk_ttl: default_file_cleanup_interval(),
            file_cleanup_interval: default_cache_cleanup_interval(),
            database_path: default_database_path(),
            max_item_size: default_maximum_size(),
            max_cache_memory: default_max_cache_memory(),
        }
    }
}
fn default_in_memory_ttl() -> usize {
    30
}
fn default_on_disk_ttl() -> usize {
    60
}
fn default_file_cleanup_interval() -> usize {
    10
}
fn default_cache_cleanup_interval() -> usize {
    5
}
fn default_database_path() -> String {
    ":memory:".to_string()
}
fn default_maximum_size() -> usize {
    // 200 mb
    200_000_000
}
fn default_max_cache_memory() -> usize {
    // 10 gb
    10_000_000_000
}
