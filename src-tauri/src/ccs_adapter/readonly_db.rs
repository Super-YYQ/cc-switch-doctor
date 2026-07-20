use crate::error::{PublicError, PublicResult};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::time::Duration;

/// Open CC Switch SQLite database in true read-only mode.
///
/// - mode=ro URI
/// - query_only=ON
/// - busy_timeout for WAL readers
/// - only SELECT / PRAGMA allowed by intent (enforced by query_only + no write APIs in our code)
pub fn open_readonly(path: &Path) -> PublicResult<Connection> {
    if !path.exists() {
        return Err(PublicError::NotFound(format!(
            "数据库不存在：{}",
            path.display()
        )));
    }

    // Normalize to absolute for URI
    let abs = path
        .canonicalize()
        .map_err(|e| PublicError::Database(format!("无法解析数据库路径：{e}")))?;

    // SQLite URI: file:/path?mode=ro
    // On Windows canonicalize may produce \\?\ prefix — strip for URI.
    let mut path_str = abs.to_string_lossy().to_string();
    if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        path_str = stripped.to_string();
    }
    // URI needs forward slashes
    let uri_path = path_str.replace('\\', "/");
    // Ensure leading slash for drive letters: C:/...
    let uri = if uri_path.starts_with('/') {
        format!("file:{uri_path}?mode=ro")
    } else {
        format!("file:/{uri_path}?mode=ro")
    };

    let conn = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| PublicError::Database(format!("只读打开失败：{e}")))?;

    conn.busy_timeout(Duration::from_millis(1500))
        .map_err(|e| PublicError::Database(format!("busy_timeout 失败：{e}")))?;

    conn.pragma_update(None, "query_only", true)
        .map_err(|e| PublicError::Database(format!("query_only 失败：{e}")))?;

    // Verify write is impossible
    let qo: i32 = conn
        .pragma_query_value(None, "query_only", |row| row.get(0))
        .unwrap_or(0);
    if qo == 0 {
        return Err(PublicError::Database(
            "无法确认 query_only=ON，已中止".into(),
        ));
    }

    Ok(conn)
}

/// Open a temporary writable connection for fixture seeding in tests only.
#[cfg(test)]
pub fn open_memory() -> Connection {
    Connection::open_in_memory().expect("memory db")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn seed_file(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA user_version=15;
             CREATE TABLE providers (id TEXT, app_type TEXT, name TEXT, settings_config TEXT, meta TEXT, is_current INTEGER);
             CREATE TABLE provider_endpoints (id INTEGER PRIMARY KEY, provider_id TEXT, app_type TEXT, url TEXT, added_at INTEGER);
             CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
             INSERT INTO providers VALUES ('a','claude','t','{}','{}',0);",
        )
        .unwrap();
    }

    #[test]
    fn readonly_blocks_writes() {
        let tmp = NamedTempFile::new().unwrap();
        // NamedTempFile is empty; seed via separate writable open after close path
        let path = tmp.path().to_path_buf();
        drop(tmp); // close handle on Windows
                   // recreate
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "PRAGMA user_version=15;
                 CREATE TABLE t(x INTEGER);
                 INSERT INTO t VALUES (1);",
            )
            .unwrap();
        }
        let ro = open_readonly(&path).expect("open ro");
        let err = ro.execute("INSERT INTO t VALUES (2)", []).err();
        assert!(err.is_some(), "write must fail on readonly connection");
        // hash stability: reading doesn't change file
        let before = std::fs::metadata(&path).unwrap().len();
        let _: i32 = ro
            .query_row("SELECT x FROM t LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let after = std::fs::metadata(&path).unwrap().len();
        assert_eq!(before, after);
        let _ = path;
        let _ = seed_file;
    }
}
