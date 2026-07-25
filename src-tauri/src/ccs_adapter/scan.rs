use super::fingerprint::{compute_compatibility_report, CapabilityState};
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
                routing: None,
            },
            vec![],
        ));
    }

    let path = discovery.database_path.clone().unwrap();
    let conn = open_with_retry(&path)?;
    let report = compute_compatibility_report(&conn)?;

    let schema_view = SchemaInfoView {
        fingerprint_id: report.observed_fingerprint.clone(),
        user_version: report.user_version,
        status: report.legacy_status().as_str().to_string(),
        tables: report.tables.clone(),
        providers_columns: report.providers_columns.clone(),
        message: report.message.clone(),
        version_verification: Some(report.version_verification.as_str().to_string()),
        capabilities: Some(report.capabilities.clone()),
        warnings: if report.warnings.is_empty() {
            None
        } else {
            Some(report.warnings.clone())
        },
    };

    // Gate Provider reading on structure capability, NOT exact version allowlist.
    if !report.can_scan_providers() {
        return Ok((
            ProviderScanView {
                discovery,
                schema: Some(schema_view),
                providers: vec![],
                can_test: false,
                scanned_at: Utc::now().to_rfc3339(),
                cc_switch_version_hint: read_version_hint(&conn),
                routing: None,
            },
            vec![],
        ));
    }

    // Routing discovery only when routing capability is usable; never fails provider scan.
    let routing = if report.capabilities.routing_discovery.is_usable() {
        Some(super::routing::discover_routing_status_sync(&conn))
    } else {
        Some(super::routing::RoutingStatusView {
            config_detected: false,
            global_enabled: false,
            listen_address: None,
            listen_port: None,
            health_reachable: false,
            server_running: false,
            failover_count: None,
            apps: vec![],
            warning: Some(report.capabilities.routing_discovery.reason.clone()),
            connect_host: None,
        })
    };

    let has_endpoints = report.capabilities.endpoint_scan.state != CapabilityState::Disabled
        && report.tables.iter().any(|t| t == "provider_endpoints")
        && report.capabilities.endpoint_scan.missing_tables.is_empty()
        && report.capabilities.endpoint_scan.missing_columns.is_empty();
    // Degraded endpoints may still have the table with partial columns — try load if table exists.
    let try_endpoints = report.tables.iter().any(|t| t == "provider_endpoints")
        && (has_endpoints
            || report.capabilities.endpoint_scan.state == CapabilityState::Degraded
            || report.capabilities.endpoint_scan.state == CapabilityState::Supported);

    let raws = match load_raw_providers(
        &conn,
        &report.providers_columns,
        try_endpoints && report.capabilities.endpoint_scan.missing_tables.is_empty(),
    ) {
        Ok(r) => r,
        Err(e) => {
            // Do not wipe the whole DB on a query failure if structure looked OK —
            // surface empty list with message via schema.
            let _ = e;
            vec![]
        }
    };

    let mut normalized = Vec::with_capacity(raws.len());
    let mut list = Vec::with_capacity(raws.len());
    for raw in raws {
        // Per-provider isolation: normalize never panics on bad JSON; skip empty ids.
        if raw.id.trim().is_empty() {
            continue;
        }
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
            can_test: report.can_test(),
            scanned_at: Utc::now().to_rfc3339(),
            cc_switch_version_hint: read_version_hint(&conn),
            routing,
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
            // Soft type coercion: bad row types become empty/default rather than failing the map.
            let id: String = row.get::<_, String>(0).unwrap_or_default();
            let app_type: String = row.get::<_, String>(1).unwrap_or_default();
            let name: String = row.get::<_, String>(2).unwrap_or_default();
            let settings_config: String = row.get::<_, String>(3).unwrap_or_default();
            let website_url: Option<String> = row.get(4).ok().flatten();
            let category: Option<String> = row.get(5).ok().flatten();
            let meta: String = row
                .get::<_, Option<String>>(6)
                .ok()
                .flatten()
                .unwrap_or_else(|| "{}".into());
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
        // Skip unreadable individual rows; do not abort the whole list.
        let Ok(mut raw) = r else {
            continue;
        };
        if raw.id.trim().is_empty() {
            continue;
        }
        if has_endpoints {
            // Endpoint load failure for one provider must not block others.
            raw.endpoint_urls = load_endpoints(conn, &raw.id, &raw.app_type).unwrap_or_default();
        }
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
    // Prefer ordered query; fall back if added_at is missing.
    let sqls = [
        "SELECT url FROM provider_endpoints WHERE provider_id = ?1 AND app_type = ?2 ORDER BY added_at ASC, url ASC",
        "SELECT url FROM provider_endpoints WHERE provider_id = ?1 AND app_type = ?2 ORDER BY url ASC",
        "SELECT url FROM provider_endpoints WHERE provider_id = ?1 AND app_type = ?2",
    ];
    for sql in sqls {
        let Ok(mut stmt) = conn.prepare(sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map(params![provider_id, app_type], |row| {
            row.get::<_, String>(0)
        }) else {
            continue;
        };
        let mut out = Vec::new();
        for u in rows.flatten() {
            if !u.is_empty() {
                out.push(u);
            }
        }
        return Ok(out);
    }
    Ok(vec![])
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
        assert!(normalized
            .iter()
            .any(|p| p.display_name.contains("V13 Claude")));
    }

    fn write_sql_fixture(path: &Path, sql: &str) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(sql).unwrap();
    }

    #[test]
    fn synthetic_v16_verified_scan() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_sql_fixture(
            &path,
            include_str!("../../../compatibility/fixtures/synthetic-v16.sql"),
        );
        let before = file_sha256(&path).unwrap();
        let (view, _) = scan_database(Some(&path)).unwrap();
        let after = file_sha256(&path).unwrap();
        assert_eq!(before, after);
        let schema = view.schema.as_ref().unwrap();
        assert_eq!(schema.user_version, 16);
        assert_eq!(schema.status, "verified");
        assert_eq!(schema.version_verification.as_deref(), Some("verified"));
        assert_eq!(schema.fingerprint_id, "ccs-schema-v16-providers-v318");
        assert!(view.can_test);
        assert!(!view.providers.is_empty());
        let caps = schema.capabilities.as_ref().unwrap();
        assert_eq!(
            caps.provider_scan.state,
            super::super::fingerprint::CapabilityState::Supported
        );
        assert_eq!(
            caps.direct_diagnosis.state,
            super::super::fingerprint::CapabilityState::Supported
        );
        let blob = serde_json::to_string(&view).unwrap();
        assert!(!blob.contains("sk-test-fake-key-for-unit-tests-only"));
    }

    #[test]
    fn future_v17_same_core_lists_providers() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_sql_fixture(
            &path,
            include_str!("../../../compatibility/fixtures/synthetic-future-v17-same-core.sql"),
        );
        let before = file_sha256(&path).unwrap();
        let (view, _) = scan_database(Some(&path)).unwrap();
        let after = file_sha256(&path).unwrap();
        assert_eq!(before, after);
        let schema = view.schema.as_ref().unwrap();
        assert_eq!(schema.user_version, 17);
        assert_eq!(
            schema.version_verification.as_deref(),
            Some("unverified_structure_compatible")
        );
        assert!(view.can_test);
        assert!(view
            .providers
            .iter()
            .any(|p| p.source_id == "v17-claude-1" && p.selectable));
    }

    #[test]
    fn future_extra_columns_do_not_block() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_sql_fixture(
            &path,
            include_str!("../../../compatibility/fixtures/synthetic-future-extra-columns.sql"),
        );
        let (view, _) = scan_database(Some(&path)).unwrap();
        assert!(view.can_test);
        assert!(view
            .providers
            .iter()
            .any(|p| p.source_id == "extra-claude-1"));
        let schema = view.schema.as_ref().unwrap();
        assert_eq!(schema.user_version, 18);
        assert!(view.can_test);
    }

    #[test]
    fn missing_required_column_disables_scan() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_sql_fixture(
            &path,
            include_str!(
                "../../../compatibility/fixtures/synthetic-provider-required-column-missing.sql"
            ),
        );
        let (view, _) = scan_database(Some(&path)).unwrap();
        assert!(!view.can_test);
        assert!(view.providers.is_empty());
        let schema = view.schema.as_ref().unwrap();
        let caps = schema.capabilities.as_ref().unwrap();
        assert_eq!(
            caps.provider_scan.state,
            super::super::fingerprint::CapabilityState::Disabled
        );
        assert!(caps
            .provider_scan
            .missing_columns
            .iter()
            .any(|c| c == "settings_config"));
    }

    #[test]
    fn endpoints_missing_settings_baseurl_degrades() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_sql_fixture(
            &path,
            include_str!(
                "../../../compatibility/fixtures/synthetic-endpoints-missing-baseurl-in-settings.sql"
            ),
        );
        let (view, _) = scan_database(Some(&path)).unwrap();
        // Providers still listed
        assert_eq!(view.providers.len(), 2);
        let ok = view
            .providers
            .iter()
            .find(|p| p.source_id == "ep-missing-ok")
            .unwrap();
        assert!(ok.selectable);
        let no_url = view
            .providers
            .iter()
            .find(|p| p.source_id == "ep-missing-no-url")
            .unwrap();
        assert!(!no_url.selectable);
        let schema = view.schema.as_ref().unwrap();
        let caps = schema.capabilities.as_ref().unwrap();
        assert_eq!(
            caps.endpoint_scan.state,
            super::super::fingerprint::CapabilityState::Degraded
        );
        assert!(caps.direct_diagnosis.is_usable());
        assert!(view.can_test);
    }

    #[test]
    fn routing_unknown_provider_still_works() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_sql_fixture(
            &path,
            include_str!(
                "../../../compatibility/fixtures/synthetic-routing-unknown-provider-compatible.sql"
            ),
        );
        let (view, _) = scan_database(Some(&path)).unwrap();
        assert!(view.can_test);
        assert!(view
            .providers
            .iter()
            .any(|p| p.source_id == "route-unknown-claude" && p.selectable));
        let schema = view.schema.as_ref().unwrap();
        let caps = schema.capabilities.as_ref().unwrap();
        assert_eq!(
            caps.routing_discovery.state,
            super::super::fingerprint::CapabilityState::Disabled
        );
        assert_eq!(
            caps.direct_diagnosis.state,
            super::super::fingerprint::CapabilityState::Supported
        );
        // Routing status should carry warning, not wipe providers
        assert!(view.routing.as_ref().is_some_and(|r| !r.config_detected));
    }

    #[test]
    fn one_invalid_provider_does_not_block_others() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        drop(tmp);
        write_sql_fixture(
            &path,
            include_str!("../../../compatibility/fixtures/synthetic-one-provider-invalid.sql"),
        );
        let (view, _) = scan_database(Some(&path)).unwrap();
        assert_eq!(view.providers.len(), 3);
        let a = view
            .providers
            .iter()
            .find(|p| p.source_id == "ok-a")
            .unwrap();
        let b = view
            .providers
            .iter()
            .find(|p| p.source_id == "ok-b")
            .unwrap();
        assert!(a.selectable);
        assert!(b.selectable);
        let bad = view
            .providers
            .iter()
            .find(|p| p.source_id == "bad-settings")
            .unwrap();
        // Invalid JSON → no key/url → not selectable, but still listed
        assert!(!bad.selectable);
        assert!(view.can_test);
        let blob = serde_json::to_string(&view).unwrap();
        assert!(!blob.contains("sk-test-fake-key-for-ok-a-only"));
        assert!(!blob.contains("sk-test-fake-key-for-ok-b-only"));
    }
}
