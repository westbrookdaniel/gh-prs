use crate::gh::CommandClass;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE_TTL_PREFLIGHT: Duration = Duration::from_secs(120);
const CACHE_TTL_REPO_CONTEXT: Duration = Duration::from_secs(120);
const CACHE_TTL_PR_SEARCH: Duration = Duration::from_secs(90);
const CACHE_TTL_PR_DETAIL: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    class: CommandClass,
    args: Vec<String>,
    stdin_hash: Option<u64>,
}

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    expires_at: Instant,
    value: T,
}

pub struct CommandCache<T: Clone> {
    entries: Mutex<HashMap<CacheKey, CacheEntry<T>>>,
}

impl<T: Clone> Default for CommandCache<T> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<T: Clone> CommandCache<T> {
    pub fn lookup(&self, class: CommandClass, args: &[String], stdin: Option<&[u8]>) -> Option<T> {
        let key = build_key(class, args, stdin);
        let mut entries = self.entries.lock().expect("cache lock poisoned");
        let Some(entry) = entries.get(&key).cloned() else {
            return None;
        };

        if Instant::now() >= entry.expires_at {
            entries.remove(&key);
            return None;
        }

        Some(entry.value)
    }

    pub fn store(
        &self,
        class: CommandClass,
        args: &[String],
        stdin: Option<&[u8]>,
        ttl: Duration,
        value: T,
    ) {
        if ttl.is_zero() {
            return;
        }

        let key = build_key(class, args, stdin);
        let mut entries = self.entries.lock().expect("cache lock poisoned");
        entries.insert(
            key,
            CacheEntry {
                expires_at: Instant::now() + ttl,
                value,
            },
        );
    }

    pub fn invalidate_pull_request_reads(&self) {
        let mut entries = self.entries.lock().expect("cache lock poisoned");
        entries.retain(|key, _| !is_pull_request_read(key.class));
    }
}

pub fn cache_ttl(class: CommandClass) -> Option<Duration> {
    match class {
        CommandClass::PreflightVersion | CommandClass::PreflightAuth => Some(CACHE_TTL_PREFLIGHT),
        CommandClass::ResolveRepo => Some(CACHE_TTL_REPO_CONTEXT),
        CommandClass::PullRequestSearch | CommandClass::PullRequestList => {
            Some(CACHE_TTL_PR_SEARCH)
        }
        CommandClass::PullRequestDetail
        | CommandClass::PullRequestFiles
        | CommandClass::IssueComments
        | CommandClass::PullRequestReviews
        | CommandClass::PullRequestReviewComments => Some(CACHE_TTL_PR_DETAIL),
        CommandClass::SubmitComment | CommandClass::SubmitReview => None,
    }
}

fn build_key(class: CommandClass, args: &[String], stdin: Option<&[u8]>) -> CacheKey {
    let stdin_hash = stdin.map(|bytes| {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    });

    CacheKey {
        class,
        args: args.to_vec(),
        stdin_hash,
    }
}

fn is_pull_request_read(class: CommandClass) -> bool {
    matches!(
        class,
        CommandClass::PullRequestSearch
            | CommandClass::PullRequestList
            | CommandClass::PullRequestDetail
            | CommandClass::PullRequestFiles
            | CommandClass::IssueComments
            | CommandClass::PullRequestReviews
            | CommandClass::PullRequestReviewComments
    )
}

#[cfg(test)]
mod tests {
    use super::{CommandCache, cache_ttl};
    use crate::gh::CommandClass;
    use std::time::Duration;

    #[test]
    fn cache_round_trip_and_expiry() {
        let cache = CommandCache::default();
        let args = vec!["search".to_string(), "prs".to_string()];

        cache.store(
            CommandClass::PullRequestSearch,
            &args,
            None,
            Duration::from_millis(20),
            "value".to_string(),
        );

        assert_eq!(
            cache.lookup(CommandClass::PullRequestSearch, &args, None),
            Some("value".to_string())
        );

        std::thread::sleep(Duration::from_millis(25));
        assert!(
            cache
                .lookup(CommandClass::PullRequestSearch, &args, None)
                .is_none()
        );
    }

    #[test]
    fn invalidates_pull_request_classes_only() {
        let cache = CommandCache::default();
        let search_args = vec!["search".to_string(), "prs".to_string()];
        let auth_args = vec!["auth".to_string(), "status".to_string()];

        cache.store(
            CommandClass::PullRequestSearch,
            &search_args,
            None,
            Duration::from_secs(30),
            "search".to_string(),
        );
        cache.store(
            CommandClass::PreflightAuth,
            &auth_args,
            None,
            Duration::from_secs(30),
            "auth".to_string(),
        );

        cache.invalidate_pull_request_reads();

        assert!(
            cache
                .lookup(CommandClass::PullRequestSearch, &search_args, None)
                .is_none()
        );
        assert_eq!(
            cache.lookup(CommandClass::PreflightAuth, &auth_args, None),
            Some("auth".to_string())
        );
    }

    #[test]
    fn ttl_policy_disables_writes() {
        assert!(cache_ttl(CommandClass::SubmitComment).is_none());
        assert!(cache_ttl(CommandClass::SubmitReview).is_none());
        assert!(cache_ttl(CommandClass::PullRequestSearch).is_some());
    }
}
