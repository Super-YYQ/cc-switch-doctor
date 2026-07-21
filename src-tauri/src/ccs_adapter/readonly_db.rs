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

    let uri = sqlite_readonly_uri(&abs);

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

/// Build a SQLite URI with `mode=ro` for local drive letters and Windows UNC paths.
///
/// Examples:
/// - `C:\data\cc-switch.db` → `file:/C:/data/cc-switch.db?mode=ro`
/// - `\\?\C:\data\cc-switch.db` → `file:/C:/data/cc-switch.db?mode=ro`
/// - `\\?\UNC\server\share\cc-switch.db` → `file://server/share/cc-switch.db?mode=ro`
/// - `\\server\share\cc-switch.db` → `file://server/share/cc-switch.db?mode=ro`
pub fn sqlite_readonly_uri(path: &Path) -> String {
    let mut path_str = path.to_string_lossy().to_string();

    // Strip Windows extended-length prefix
    if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        path_str = stripped.to_string();
    }

    // UNC after \\?\ strip becomes UNC\server\share\...
    if let Some(rest) = path_str.strip_prefix(r"UNC\") {
        let with_slashes = rest.replace('\\', "/");
        let encoded = encode_uri_path(&with_slashes);
        return format!("file://{encoded}?mode=ro");
    }

    // Raw UNC \\server\share\...
    if path_str.starts_with(r"\\") {
        let trimmed = path_str.trim_start_matches('\\');
        let with_slashes = trimmed.replace('\\', "/");
        let encoded = encode_uri_path(&with_slashes);
        return format!("file://{encoded}?mode=ro");
    }

    // Local path (drive letter or POSIX)
    let uri_path = path_str.replace('\\', "/");
    let encoded = encode_uri_path(&uri_path);
    if encoded.starts_with('/') {
        format!("file:{encoded}?mode=ro")
    } else {
        format!("file:/{encoded}?mode=ro")
    }
}

/// Percent-encode path segments that need it, keeping `/` and `:` (drive) intact.
fn encode_uri_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Open a temporary writable connection for fixture seeding in tests only.
#[cfg(test)]
pub fn open_memory() -> Connection {
    Connection::open_in_memory().expect("memory db")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn file_sha256(path: &Path) -> String {
        let bytes = std::fs::read(path).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }

    #[test]
    fn readonly_blocks_writes_and_sha256_stable() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp); // close handle on Windows
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "PRAGMA user_version=15;
                 CREATE TABLE t(x INTEGER);
                 INSERT INTO t VALUES (1);",
            )
            .unwrap();
        }
        let before_hash = file_sha256(&path);
        let ro = open_readonly(&path).expect("open ro");
        let err = ro.execute("INSERT INTO t VALUES (2)", []).err();
        assert!(err.is_some(), "write must fail on readonly connection");
        let _: i32 = ro
            .query_row("SELECT x FROM t LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let after_hash = file_sha256(&path);
        assert_eq!(
            before_hash, after_hash,
            "SHA-256 must be stable after readonly reads"
        );
    }

    #[test]
    fn uri_local_drive() {
        let p = PathBuf::from(r"C:\Users\me\cc-switch.db");
        let uri = sqlite_readonly_uri(&p);
        assert!(uri.starts_with("file:/"), "{uri}");
        assert!(uri.contains("C:/Users/me/cc-switch.db"), "{uri}");
        assert!(uri.ends_with("?mode=ro"), "{uri}");
    }

    #[test]
    fn uri_extended_prefix_local() {
        let p = PathBuf::from(r"\\?\C:\data\cc-switch.db");
        let uri = sqlite_readonly_uri(&p);
        assert!(uri.contains("C:/data/cc-switch.db"), "{uri}");
        assert!(!uri.contains("?\\"), "{uri}");
        assert!(uri.ends_with("?mode=ro"), "{uri}");
    }

    #[test]
    fn uri_unc_extended() {
        let p = PathBuf::from(r"\\?\UNC\server\share\cc-switch.db");
        let uri = sqlite_readonly_uri(&p);
        assert_eq!(uri, "file://server/share/cc-switch.db?mode=ro");
    }

    #[test]
    fn uri_unc_raw() {
        let p = PathBuf::from(r"\\server\share\cc-switch.db");
        let uri = sqlite_readonly_uri(&p);
        assert_eq!(uri, "file://server/share/cc-switch.db?mode=ro");
    }

    #[test]
    fn uri_encodes_spaces() {
        let p = PathBuf::from(r"C:\My Docs\cc switch.db");
        let uri = sqlite_readonly_uri(&p);
        assert!(uri.contains("My%20Docs"), "{uri}");
        assert!(uri.contains("cc%20switch.db"), "{uri}");
        assert!(uri.ends_with("?mode=ro"), "{uri}");
    }
}
