use crate::error::{PublicError, PublicResult};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Verified,
    Compatible,
    Unknown,
    Unsupported,
}

impl CompatibilityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Compatible => "compatible",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn can_test(self) -> bool {
        matches!(self, Self::Verified | Self::Compatible)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaFingerprint {
    pub id: String,
    pub user_version: i32,
    pub tables: Vec<String>,
    pub providers_columns: Vec<String>,
    pub provider_endpoints_columns: Vec<String>,
    pub settings_columns: Vec<String>,
    pub status: CompatibilityStatus,
    pub message: String,
}

#[derive(Debug, Clone)]
struct SchemaAllowEntry {
    id: &'static str,
    user_version: i32,
    status: CompatibilityStatus,
    required_tables: &'static [&'static str],
    providers_columns: &'static [&'static str],
    provider_endpoints_columns: &'static [&'static str],
    message: &'static str,
}

/// Exact fingerprints only — never wide ranges.
const SCHEMA_ALLOWLIST: &[SchemaAllowEntry] = &[
    SchemaAllowEntry {
        id: "ccs-schema-v15-providers-v317",
        user_version: 15,
        status: CompatibilityStatus::Verified,
        required_tables: &["providers", "provider_endpoints", "settings"],
        providers_columns: &[
            "id",
            "app_type",
            "name",
            "settings_config",
            "website_url",
            "category",
            "created_at",
            "sort_index",
            "notes",
            "icon",
            "icon_color",
            "meta",
            "is_current",
            "in_failover_queue",
        ],
        provider_endpoints_columns: &["id", "provider_id", "app_type", "url", "added_at"],
        message: "Schema 与 CC Switch v3.17.0（user_version=15）已验证指纹匹配。",
    },
    // Real-world DBs observed as user_version=13 with the same core provider shape
    // as the v3.17 lineage (providers + endpoints). Marked compatible, not verified.
    SchemaAllowEntry {
        id: "ccs-schema-v13-providers-core",
        user_version: 13,
        status: CompatibilityStatus::Compatible,
        required_tables: &["providers", "provider_endpoints"],
        providers_columns: &[
            "id",
            "app_type",
            "name",
            "settings_config",
            "website_url",
            "category",
            "created_at",
            "sort_index",
            "notes",
            "icon",
            "icon_color",
            "meta",
            "is_current",
            "in_failover_queue",
        ],
        provider_endpoints_columns: &["id", "provider_id", "app_type", "url", "added_at"],
        message: "Schema user_version=13 已按精确指纹标记为兼容；可只读扫描并测试第三方配置。",
    },
];

const REQUIRED_PROVIDER_COLS: &[&str] = &[
    "id",
    "app_type",
    "name",
    "settings_config",
    "meta",
    "is_current",
];

const REQUIRED_ENDPOINT_COLS: &[&str] = &["provider_id", "app_type", "url"];

pub fn compute_fingerprint(conn: &Connection) -> PublicResult<SchemaFingerprint> {
    let user_version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| PublicError::Database(format!("读取 user_version 失败：{e}")))?;

    let mut tables = list_tables(conn)?;
    tables.sort();

    let providers_columns = list_columns(conn, "providers").unwrap_or_default();
    let provider_endpoints_columns = list_columns(conn, "provider_endpoints").unwrap_or_default();
    let settings_columns = list_columns(conn, "settings").unwrap_or_default();

    let has_providers = tables.iter().any(|t| t == "providers");
    let has_endpoints = tables.iter().any(|t| t == "provider_endpoints");

    let providers_ok = has_providers
        && REQUIRED_PROVIDER_COLS
            .iter()
            .all(|c| providers_columns.iter().any(|x| x == c));
    let endpoints_ok = has_endpoints
        && REQUIRED_ENDPOINT_COLS
            .iter()
            .all(|c| provider_endpoints_columns.iter().any(|x| x == c));

    let (status, message, id_override) = classify(
        user_version,
        has_providers,
        providers_ok,
        endpoints_ok,
        &tables,
        &providers_columns,
        &provider_endpoints_columns,
    );

    let id = id_override.unwrap_or_else(|| {
        fingerprint_id(
            user_version,
            &tables,
            &providers_columns,
            &provider_endpoints_columns,
        )
    });

    Ok(SchemaFingerprint {
        id,
        user_version,
        tables,
        providers_columns,
        provider_endpoints_columns,
        settings_columns,
        status,
        message,
    })
}

fn columns_contain_all(have: &[String], need: &[&str]) -> bool {
    need.iter().all(|c| have.iter().any(|x| x == *c))
}

fn classify(
    user_version: i32,
    has_providers: bool,
    providers_ok: bool,
    endpoints_ok: bool,
    tables: &[String],
    providers_columns: &[String],
    provider_endpoints_columns: &[String],
) -> (CompatibilityStatus, String, Option<String>) {
    if !has_providers || !providers_ok {
        return (
            CompatibilityStatus::Unsupported,
            "缺少 providers 关键字段，无法安全解析。".into(),
            None,
        );
    }

    for entry in SCHEMA_ALLOWLIST {
        if entry.user_version != user_version {
            continue;
        }
        let tables_ok = entry
            .required_tables
            .iter()
            .all(|t| tables.iter().any(|x| x == *t));
        let prov_ok = columns_contain_all(providers_columns, entry.providers_columns);
        let ep_ok = if entry.required_tables.contains(&"provider_endpoints") {
            endpoints_ok
                && columns_contain_all(provider_endpoints_columns, entry.provider_endpoints_columns)
        } else {
            true
        };
        if tables_ok && prov_ok && ep_ok {
            return (
                entry.status,
                entry.message.into(),
                Some(entry.id.to_string()),
            );
        }
    }

    if providers_ok && !endpoints_ok {
        return (
            CompatibilityStatus::Unknown,
            "providers 可解析但 provider_endpoints 缺失或未验证；已停止敏感字段读取与测试。".into(),
            None,
        );
    }

    (
        CompatibilityStatus::Unknown,
        format!(
            "检测到未知 schema 结构（user_version={user_version}）。为安全起见已停止读取敏感字段与测试。请更新 CC Switch Doctor。"
        ),
        None,
    )
}

fn fingerprint_id(
    user_version: i32,
    tables: &[String],
    providers_columns: &[String],
    endpoints_columns: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_version.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(tables.join(",").as_bytes());
    hasher.update(b"|");
    hasher.update(providers_columns.join(",").as_bytes());
    hasher.update(b"|");
    hasher.update(endpoints_columns.join(",").as_bytes());
    let dig = hasher.finalize();
    format!("ccs-schema-{}", hex::encode(&dig[..8]))
}

fn list_tables(conn: &Connection) -> PublicResult<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| PublicError::Database(e.to_string()))?);
    }
    Ok(out)
}

fn list_columns(conn: &Connection, table: &str) -> PublicResult<Vec<String>> {
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(PublicError::Database("非法表名".into()));
    }
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| PublicError::Database(e.to_string()))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn seed_core_shape(conn: &Connection, user_version: i32) {
        conn.execute_batch(&format!(
            r#"
            PRAGMA user_version={user_version};
            CREATE TABLE providers (
                id TEXT, app_type TEXT, name TEXT, settings_config TEXT,
                website_url TEXT, category TEXT, created_at INTEGER, sort_index INTEGER,
                notes TEXT, icon TEXT, icon_color TEXT, meta TEXT, is_current INTEGER,
                in_failover_queue INTEGER
            );
            CREATE TABLE provider_endpoints (
                id INTEGER PRIMARY KEY, provider_id TEXT, app_type TEXT, url TEXT, added_at INTEGER
            );
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
            "#
        ))
        .unwrap();
    }

    #[test]
    fn verified_v15_fingerprint() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 15);
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Verified);
        assert!(fp.status.can_test());
    }

    #[test]
    fn compatible_v13_exact_fingerprint() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 13);
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Compatible);
        assert!(fp.status.can_test());
        assert_eq!(fp.id, "ccs-schema-v13-providers-core");
    }

    #[test]
    fn unknown_v16_not_allowed() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 16);
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Unknown);
        assert!(!fp.status.can_test());
    }

    #[test]
    fn v13_missing_required_col_unknown() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA user_version=13;
            CREATE TABLE providers (
                id TEXT, app_type TEXT, name TEXT, settings_config TEXT,
                meta TEXT, is_current INTEGER
            );
            CREATE TABLE provider_endpoints (
                id INTEGER PRIMARY KEY, provider_id TEXT, app_type TEXT, url TEXT, added_at INTEGER
            );
            "#,
        )
        .unwrap();
        let fp = compute_fingerprint(&conn).unwrap();
        // missing website_url etc → not exact allowlist; still has REQUIRED_PROVIDER_COLS so Unknown
        assert!(!fp.status.can_test() || fp.status == CompatibilityStatus::Unknown);
    }

    #[test]
    fn missing_providers_unsupported() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA user_version=1; CREATE TABLE foo(x);")
            .unwrap();
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Unsupported);
        assert!(!fp.status.can_test());
    }
}
