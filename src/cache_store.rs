use rusqlite::{Connection, OptionalExtension, params};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub payload: Vec<u8>,
    pub is_stale: bool,
}

#[derive(Debug, Clone)]
pub struct SqliteCacheStore {
    db_path: PathBuf,
}

impl SqliteCacheStore {
    pub fn open(db_path: PathBuf) -> io::Result<Self> {
        ensure_parent_dir(&db_path)?;
        initialize_schema(&db_path)?;
        Ok(Self { db_path })
    }

    pub fn open_default() -> io::Result<Self> {
        Self::open(default_cache_db_path()?)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub async fn get(&self, key: &str) -> io::Result<Option<CacheEntry>> {
        let db_path = self.db_path.clone();
        let key = key.to_string();
        smol::unblock(move || {
            let connection = open_connection(&db_path)?;
            let now_ms = now_unix_ms();

            let mut statement = connection
                .prepare(
                    "SELECT payload, fetched_at_ms, stale_at_ms, expires_at_ms
                     FROM cache_entries
                     WHERE key = ?1",
                )
                .map_err(to_io_err)?;

            let row = statement
                .query_row(params![key], |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .optional()
                .map_err(to_io_err)?;

            let Some((payload, _fetched_at_ms, stale_at_ms, expires_at_ms)) = row else {
                return Ok(None);
            };

            if now_ms >= expires_at_ms {
                connection
                    .execute("DELETE FROM cache_entries WHERE key = ?1", params![key])
                    .map_err(to_io_err)?;
                return Ok(None);
            }

            Ok(Some(CacheEntry {
                payload,
                is_stale: now_ms >= stale_at_ms,
            }))
        })
        .await
    }

    pub async fn set(
        &self,
        key: &str,
        payload: &[u8],
        stale_after: Duration,
        ttl: Duration,
    ) -> io::Result<()> {
        let db_path = self.db_path.clone();
        let key = key.to_string();
        let payload = payload.to_vec();
        smol::unblock(move || {
            let connection = open_connection(&db_path)?;
            let fetched_at_ms = now_unix_ms();

            let stale_after_ms = duration_to_ms(stale_after).max(1);
            let ttl_ms = duration_to_ms(ttl).max(stale_after_ms);

            let stale_at_ms = fetched_at_ms.saturating_add(stale_after_ms);
            let expires_at_ms = fetched_at_ms.saturating_add(ttl_ms);

            connection
                .execute(
                    "INSERT INTO cache_entries (key, payload, fetched_at_ms, stale_at_ms, expires_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(key) DO UPDATE SET
                       payload = excluded.payload,
                       fetched_at_ms = excluded.fetched_at_ms,
                       stale_at_ms = excluded.stale_at_ms,
                       expires_at_ms = excluded.expires_at_ms",
                    params![key, payload, fetched_at_ms, stale_at_ms, expires_at_ms],
                )
                .map_err(to_io_err)?;

            Ok(())
        })
        .await
    }

    pub async fn invalidate_prefix(&self, prefix: &str) -> io::Result<usize> {
        let db_path = self.db_path.clone();
        let pattern = format!("{prefix}%");
        smol::unblock(move || {
            let connection = open_connection(&db_path)?;
            let deleted = connection
                .execute(
                    "DELETE FROM cache_entries WHERE key LIKE ?1",
                    params![pattern],
                )
                .map_err(to_io_err)?;
            Ok(deleted)
        })
        .await
    }

    pub async fn prune_expired(&self) -> io::Result<usize> {
        let db_path = self.db_path.clone();
        smol::unblock(move || {
            let connection = open_connection(&db_path)?;
            let deleted = connection
                .execute(
                    "DELETE FROM cache_entries WHERE expires_at_ms <= ?1",
                    params![now_unix_ms()],
                )
                .map_err(to_io_err)?;
            Ok(deleted)
        })
        .await
    }
}

pub fn default_app_home() -> io::Result<PathBuf> {
    if let Some(custom) = env::var_os("GH_PRS_HOME") {
        let path = PathBuf::from(custom);
        if path.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "GH_PRS_HOME is set but empty",
            ));
        }
        return Ok(path);
    }

    let home = env::var_os("HOME").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "HOME is not set; set GH_PRS_HOME explicitly",
        )
    })?;

    Ok(PathBuf::from(home).join(".config").join("gh-prs"))
}

pub fn default_cache_db_path() -> io::Result<PathBuf> {
    Ok(default_app_home()?.join("cache.db"))
}

pub fn cache_db_path_for_home(home: &Path) -> PathBuf {
    home.join("cache.db")
}

fn ensure_parent_dir(db_path: &Path) -> io::Result<()> {
    let parent = db_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "cache database path must include a parent directory",
        )
    })?;

    fs::create_dir_all(parent)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }

    Ok(())
}

fn initialize_schema(db_path: &Path) -> io::Result<()> {
    let connection = open_connection(db_path)?;
    let current_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(to_io_err)?;

    if current_version >= SCHEMA_VERSION {
        return Ok(());
    }

    connection
        .execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS cache_entries (
               key TEXT PRIMARY KEY,
               payload BLOB NOT NULL,
               fetched_at_ms INTEGER NOT NULL,
               stale_at_ms INTEGER NOT NULL,
               expires_at_ms INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_cache_entries_stale_at_ms ON cache_entries(stale_at_ms);
             CREATE INDEX IF NOT EXISTS idx_cache_entries_expires_at_ms ON cache_entries(expires_at_ms);
             COMMIT;",
        )
        .map_err(to_io_err)?;

    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(to_io_err)?;

    Ok(())
}

fn open_connection(path: &Path) -> io::Result<Connection> {
    let connection = Connection::open(path).map_err(to_io_err)?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(to_io_err)?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(to_io_err)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(to_io_err)?;
    Ok(connection)
}

fn duration_to_ms(duration: Duration) -> i64 {
    duration.as_millis().try_into().unwrap_or(i64::MAX)
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn to_io_err(error: rusqlite::Error) -> io::Error {
    io::Error::other(format!("sqlite cache error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::SqliteCacheStore;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_db_path() -> io::Result<PathBuf> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let base = std::env::temp_dir().join(format!("gh-prs-cache-tests-{nanos}"));
        fs::create_dir_all(&base)?;
        Ok(base.join("cache.db"))
    }

    #[test]
    fn round_trip_and_staleness() {
        smol::block_on(async {
            let db_path = temp_db_path().expect("db path");
            let store = SqliteCacheStore::open(db_path.clone()).expect("store");

            store
                .set(
                    "key|one",
                    b"payload",
                    Duration::from_millis(1),
                    Duration::from_secs(5),
                )
                .await
                .expect("set");

            smol::Timer::after(Duration::from_millis(3)).await;

            let cached = store.get("key|one").await.expect("get").expect("entry");
            assert_eq!(cached.payload, b"payload");
            assert!(cached.is_stale);

            fs::remove_dir_all(db_path.parent().expect("parent")).expect("cleanup");
        });
    }

    #[test]
    fn invalidate_prefix_deletes_only_matching_rows() {
        smol::block_on(async {
            let db_path = temp_db_path().expect("db path");
            let store = SqliteCacheStore::open(db_path.clone()).expect("store");

            store
                .set(
                    "pr|detail|acme/widgets|1",
                    b"a",
                    Duration::from_secs(1),
                    Duration::from_secs(30),
                )
                .await
                .expect("set a");
            store
                .set(
                    "preflight|diag",
                    b"b",
                    Duration::from_secs(1),
                    Duration::from_secs(30),
                )
                .await
                .expect("set b");

            store
                .invalidate_prefix("pr|")
                .await
                .expect("invalidate");

            assert!(store.get("pr|detail|acme/widgets|1").await.expect("get a").is_none());
            assert!(store.get("preflight|diag").await.expect("get b").is_some());

            fs::remove_dir_all(db_path.parent().expect("parent")).expect("cleanup");
        });
    }
}
