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

    let (status, message) = classify(
        user_version,
        has_providers,
        providers_ok,
        endpoints_ok,
        &providers_columns,
    );

    let id = fingerprint_id(
        user_version,
        &tables,
        &providers_columns,
        &provider_endpoints_columns,
    );

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

fn classify(
    user_version: i32,
    has_providers: bool,
    providers_ok: bool,
    endpoints_ok: bool,
    providers_columns: &[String],
) -> (CompatibilityStatus, String) {
    if !has_providers || !providers_ok {
        return (
            CompatibilityStatus::Unsupported,
            "缺少 providers 关键字段，无法安全解析。".into(),
        );
    }

    // Known verified: schema v15 with expected columns from CC Switch 3.17
    let verified_cols = [
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
    ];
    let is_v15_shape = user_version == 15
        && verified_cols
            .iter()
            .all(|c| providers_columns.iter().any(|x| x == *c))
        && endpoints_ok;

    if is_v15_shape {
        return (
            CompatibilityStatus::Verified,
            "Schema 与 CC Switch v3.17.0（user_version=15）已验证指纹匹配。".into(),
        );
    }

    if providers_ok && endpoints_ok && (12..=20).contains(&user_version) {
        return (
            CompatibilityStatus::Compatible,
            format!(
                "关键字段完整（user_version={user_version}），按兼容模式解析；该版本尚未正式标记为 verified。"
            ),
        );
    }

    if providers_ok && !endpoints_ok {
        return (
            CompatibilityStatus::Compatible,
            "providers 可解析；provider_endpoints 缺失或字段变化，将跳过额外端点候选。".into(),
        );
    }

    (
        CompatibilityStatus::Unknown,
        format!(
            "检测到未知 schema 结构（user_version={user_version}）。为安全起见已停止读取敏感字段与测试。请更新 CC Switch Doctor。"
        ),
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
    // table name from sqlite_master only — still quote carefully
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

    #[test]
    fn verified_v15_fingerprint() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA user_version=15;
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
            "#,
        )
        .unwrap();
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Verified);
        assert!(fp.status.can_test());
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
