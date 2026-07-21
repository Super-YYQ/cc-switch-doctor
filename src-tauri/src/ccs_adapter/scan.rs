use super::fingerprint::{compute_fingerprint, CompatibilityStatus};
use super::models::{DiscoveryInfo, ProviderListItem, ProviderScanView, SchemaInfoView};
use super::normalize::{normalize_provider, RawProviderRow};
use super::path_discovery::discover_database_paths;
use super::readonly_db::open_readonly;
use crate::error::{PublicError, PublicResult};
use crate::security::redact::SecretRedactor;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

pub fn scan_database(
    db_path: Option<&Path>,
) -> PublicResult<(ProviderScanView, Vec<super::NormalizedProvider>)> {
    let discovery = if let Some(p) = db_path {
        if p.is_file() {
            DiscoveryInfo {
                found: true,
                database_path: Some(p.to_path_buf()),
                data_dir: p.parent().map(|x| x.to_path_buf()),
                source: Some("manual-or-state".into()),
                message: format!("使用数据库：{}", p.display()),
            }
        } else {
            DiscoveryInfo {
                found: false,
                database_path: None,
                data_dir: None,
                source: None,
                message: format!("指定数据库不存在：{}", p.display()),
            }
        }
    } else {
        discover_database_paths()
    };

    if !discovery.found {
        return Ok((
            ProviderScanView {
                discovery,
                schema: None,
                providers: vec![],
                can_test: false,
                scanned_at: Utc::now().to_rfc3339(),
                cc_switch_version_hint: None,
            },
            vec![],
        ));
    }

    let path = discovery.database_path.clone().unwrap();
    let conn = open_with_retry(&path)?;
    let fp = compute_fingerprint(&conn)?;

    let schema_view = SchemaInfoView {
        fingerprint_id: fp.id.clone(),
        user_version: fp.user_version,
        status: fp.status.as_str().to_string(),
        tables: fp.tables.clone(),
        providers_columns: fp.providers_columns.clone(),
        message: fp.message.clone(),
    };

    if matches!(
        fp.status,
        CompatibilityStatus::Unknown | CompatibilityStatus::Unsupported
    ) {
        return Ok((
            ProviderScanView {
                discovery,
                schema: Some(schema_view),
                providers: vec![],
                can_test: false,
                scanned_at: Utc::now().to_rfc3339(),
                cc_switch_version_hint: read_version_hint(&conn),
            },
            vec![],
        ));
    }

    let raws = load_raw_providers(
        &conn,
        &fp.providers_columns,
        fp.tables.iter().any(|t| t == "provider_endpoints"),
    )?;
    let mut normalized = Vec::with_capacity(raws.len());
    let mut list = Vec::with_capacity(raws.len());
    for raw in raws {
        let n = normalize_provider(raw);
        list.push(ProviderListItem::from(&n));
        normalized.push(n);
    }

    // Sort: current first, then app, then name
    list.sort_by(|a, b| {
        b.is_current
            .cmp(&a.is_current)
            .then(a.app_label.cmp(&b.app_label))
            .then(a.display_name.cmp(&b.display_name))
    });

    Ok((
        ProviderScanView {
            discovery,
            schema: Some(schema_view),
            providers: list,
            can_test: fp.status.can_test(),
            scanned_at: Utc::now().to_rfc3339(),
            cc_switch_version_hint: read_version_hint(&conn),
        },
        normalized,
    ))
}

fn open_with_retry(path: &Path) -> PublicResult<Connection> {
    let mut last = None;
    for _ in 0..3 {
        match open_readonly(path) {
            Ok(c) => return Ok(c),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }
    Err(last.unwrap_or_else(|| PublicError::Database("打开数据库失败".into())))
}

fn load_raw_providers(
    conn: &Connection,
    provider_cols: &[String],
    has_endpoints: bool,
) -> PublicResult<Vec<RawProviderRow>> {
    let has = |c: &str| provider_cols.iter().any(|x| x == c);
    if !has("id") || !has("app_type") || !has("name") || !has("settings_config") {
        return Err(PublicError::UnsupportedSchema(
            "providers 缺少关键列".into(),
        ));
    }

    let meta_expr = if has("meta") { "meta" } else { "'{}'" };
    let website_expr = if has("website_url") {
        "website_url"
    } else {
        "NULL"
    };
    let category_expr = if has("category") { "category" } else { "NULL" };
    let current_expr = if has("is_current") { "is_current" } else { "0" };

    let sql = format!(
        "SELECT id, app_type, name, settings_config, {website_expr}, {category_expr}, {meta_expr}, {current_expr}
         FROM providers
         ORDER BY COALESCE(sort_index, 999999), created_at ASC, id ASC"
    );

    // sort_index/created_at may be missing — fallback query
    let sql = if has("sort_index") && has("created_at") {
        sql
    } else {
        format!(
            "SELECT id, app_type, name, settings_config, {website_expr}, {category_expr}, {meta_expr}, {current_expr}
             FROM providers
             ORDER BY id ASC"
        )
    };

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| PublicError::Database(format!("查询 providers 失败：{e}")))?;

    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let app_type: String = row.get(1)?;
            let name: String = row.get(2)?;
            let settings_config: String = row.get(3)?;
            let website_url: Option<String> = row.get(4)?;
            let category: Option<String> = row.get(5)?;
            let meta: String = row.get(6)?;
            let is_current: i64 = row.get::<_, i64>(7).unwrap_or(0);
            Ok(RawProviderRow {
                id,
                app_type,
                name,
                settings_config,
                website_url,
                category,
                meta,
                is_current: is_current != 0,
                endpoint_urls: vec![],
            })
        })
        .map_err(|e| PublicError::Database(e.to_string()))?;

    let mut out = Vec::new();
    for r in rows {
        let mut raw = r.map_err(|e| PublicError::Database(e.to_string()))?;
        if has_endpoints {
            raw.endpoint_urls = load_endpoints(conn, &raw.id, &raw.app_type)?;
        }
        // Redact accidental secrets from name? no
        let _ = SecretRedactor::default();
        out.push(raw);
    }
    Ok(out)
}

fn load_endpoints(
    conn: &Connection,
    provider_id: &str,
    app_type: &str,
) -> PublicResult<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT url FROM provider_endpoints WHERE provider_id = ?1 AND app_type = ?2 ORDER BY added_at ASC, url ASC",
        )
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let rows = stmt
        .query_map(params![provider_id, app_type], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let mut out = Vec::new();
    for u in rows.flatten() {
        if !u.is_empty() {
            out.push(u);
        }
    }
    Ok(out)
}

fn read_version_hint(conn: &Connection) -> Option<String> {
    // settings table may store app version
    let val: Result<String, _> = conn.query_row(
        "SELECT value FROM settings WHERE key = 'app_version' LIMIT 1",
        [],
        |row| row.get(0),
    );
    match val {
        Ok(v) => {
            let cleaned = v.trim().trim_matches('"').to_string();
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        }
        Err(_) => None,
    }
}

/// Hash file bytes for immutability checks in tests.
pub fn file_sha256(path: &PathBuf) -> PublicResult<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| PublicError::Database(e.to_string()))?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(hex::encode(h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    fn write_fixture_db(path: &Path) {
        let sql = include_str!("../../../compatibility/fixtures/sanitized-v317.sql");
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(sql).unwrap();
    }

    fn write_v13_fixture_db(path: &Path) {
        let sql = include_str!("../../../compatibility/fixtures/synthetic-v13.sql");
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(sql).unwrap();
    }

    #[test]
    fn scan_fixture_lists_and_skips_managed() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_fixture_db(&path);
        let before = file_sha256(&path).unwrap();
        let (view, normalized) = scan_database(Some(&path)).unwrap();
        let after = file_sha256(&path).unwrap();
        assert_eq!(before, after, "DB must remain unchanged");
        assert!(view.can_test);
        assert!(view.providers.len() >= 4);
        let oauth = view
            .providers
            .iter()
            .find(|p| p.source_id == "codex-official-oauth")
            .unwrap();
        assert!(!oauth.selectable);
        let copilot = view
            .providers
            .iter()
            .find(|p| p.source_id == "gh-copilot-1")
            .unwrap();
        assert!(!copilot.selectable);
        let glm = view
            .providers
            .iter()
            .find(|p| p.source_id == "glm-claude-1")
            .unwrap();
        assert!(glm.selectable);
        // Full key must never appear in list items
        let blob = serde_json::to_string(&view.providers).unwrap();
        assert!(!blob.contains("sk-test-fake-key-for-unit-tests-only"));
        assert!(!normalized.is_empty());
    }

    #[test]
    fn synthetic_v13_end_to_end_scan() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_v13_fixture_db(&path);
        let before = file_sha256(&path).unwrap();
        let (view, normalized) = scan_database(Some(&path)).unwrap();
        let after = file_sha256(&path).unwrap();
        assert_eq!(before, after, "DB SHA256 must remain unchanged");

        let schema = view.schema.as_ref().expect("schema");
        assert_eq!(schema.user_version, 13);
        assert_eq!(schema.status, "compatible");
        assert_eq!(schema.fingerprint_id, "ccs-schema-v13-providers-core");
        assert!(view.can_test);
        assert!(!view.providers.is_empty());

        let claude = view
            .providers
            .iter()
            .find(|p| p.source_id == "v13-claude-1")
            .expect("claude provider visible");
        assert_eq!(claude.app_type.as_str(), "claude");
        assert!(claude.selectable);

        let oauth = view
            .providers
            .iter()
            .find(|p| p.source_id == "v13-codex-oauth")
            .expect("oauth listed");
        assert!(!oauth.selectable);

        let blob = serde_json::to_string(&view).unwrap();
        assert!(
            !blob.contains("sk-test-fake-key-for-v13"),
            "full key leaked into frontend view"
        );
        assert!(!normalized.is_empty());
        // normalized still holds secrets in memory only — not serialized to view
        assert!(normalized.iter().any(|p| p.display_name.contains("V13 Claude")));
    }
}
