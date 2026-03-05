use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CachedOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

impl CachedOutput {
    pub fn from_result(result: &crate::gh::client::CommandResult) -> Self {
        Self {
            stdout: result.stdout.clone(),
            stderr: result.stderr.clone(),
            code: result.code,
        }
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    expires_at: Instant,
    value: CachedOutput,
}

static CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();

fn global_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn cache_get(key: &str) -> Option<CachedOutput> {
    let mut cache = global_cache().lock().expect("cache lock poisoned");
    let Some(entry) = cache.get(key).cloned() else {
        return None;
    };

    if Instant::now() >= entry.expires_at {
        cache.remove(key);
        return None;
    }

    Some(entry.value)
}

pub fn cache_set(key: &str, ttl_secs: u64, value: CachedOutput) {
    if ttl_secs == 0 {
        return;
    }

    let mut cache = global_cache().lock().expect("cache lock poisoned");
    cache.insert(
        key.to_string(),
        CacheEntry {
            expires_at: Instant::now() + Duration::from_secs(ttl_secs),
            value,
        },
    );
}

pub fn cache_invalidate_prefix(prefix: &str) {
    let mut cache = global_cache().lock().expect("cache lock poisoned");
    cache.retain(|key, _| !key.starts_with(prefix));
}

#[cfg(test)]
mod tests {
    use super::{CachedOutput, cache_get, cache_invalidate_prefix, cache_set};

    fn value(name: &str) -> CachedOutput {
        CachedOutput {
            stdout: name.to_string(),
            stderr: String::new(),
            code: Some(0),
        }
    }

    #[test]
    fn cache_round_trip_and_expiry() {
        let key = "test|expiry";
        cache_set(key, 1, value("hello"));
        assert_eq!(
            cache_get(key).map(|entry| entry.stdout),
            Some("hello".to_string())
        );

        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(cache_get(key).is_none());
    }

    #[test]
    fn invalidates_by_prefix() {
        cache_set("pr|search|a", 30, value("a"));
        cache_set("preflight|auth", 30, value("b"));

        cache_invalidate_prefix("pr|");

        assert!(cache_get("pr|search|a").is_none());
        assert_eq!(
            cache_get("preflight|auth").map(|entry| entry.stdout),
            Some("b".to_string())
        );
    }
}
