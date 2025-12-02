pub mod core;
mod entry;
mod io;
mod mem;
pub mod settings;

pub use core::FileCache;
pub use entry::FileOptions;
pub use io::FileContent;

use chrono::{DateTime, Utc};
use tokio::time::{Duration, Instant};

fn instant_to_datetime(target: &Instant) -> DateTime<Utc> {
    let diff = match target.checked_duration_since(Instant::now()) {
        Some(d) => d,
        None => Duration::from_secs(0),
    };
    Utc::now() + chrono::Duration::from_std(diff).unwrap_or_default()
}

#[macro_export]
macro_rules! flush_entry {
    ($entry:expr, $uuid:expr, $ttl:expr, $cache_mem:expr) => {{
        if let Some(flushed_size) = $entry.flush($ttl) {
            debug!("Flushed {} from cache", $uuid);
            let mut rw_mem = $cache_mem.write().await;
            // can panic
            rw_mem.free(flushed_size as usize);
        }
    }};
}
