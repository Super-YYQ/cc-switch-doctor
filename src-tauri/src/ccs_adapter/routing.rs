//! Read-only CC Switch local-route discovery and loopback health probes.
//!
//! Hard rules:
//! - SELECT only against `proxy_config` (never INSERT/UPDATE/DELETE).
//! - Health/status probes only to loopback hosts.
//! - No provider real keys are sent on health/status probes.
//! - Never start/stop/reconfigure the CCS proxy.

use super::models::AppType;
use crate::error::{PublicError, PublicResult};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Placeholder credential used by CCS-managed client takeover (baseline v3.17).
/// Re-check upstream before changing; never send provider real keys on route tests.
pub const CCS_PROXY_PLACEHOLDER_TOKEN: &str = "PROXY_MANAGED";

pub const DEFAULT_LISTEN_PORT: u16 = 15721;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRoutingStatusView {
    pub app_type: String,
    pub app_label: String,
    pub enabled: bool,
    pub auto_failover_enabled: bool,
    pub max_retries: Option<u32>,
    pub streaming_first_byte_timeout: Option<u32>,
    pub streaming_idle_timeout: Option<u32>,
    pub non_streaming_timeout: Option<u32>,
    pub active_provider_id: Option<String>,
    pub active_provider_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingStatusView {
    pub config_detected: bool,
    pub global_enabled: bool,
    pub listen_address: Option<String>,
    pub listen_port: Option<u16>,
    pub health_reachable: bool,
    pub server_running: bool,
    pub failover_count: Option<u64>,
    pub apps: Vec<AppRoutingStatusView>,
    pub warning: Option<String>,
    /// Connect host used for probes (loopback only).
    pub connect_host: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProxyConfigRow {
    pub app_type: String,
    pub proxy_enabled: bool,
    pub listen_address: String,
    pub listen_port: u16,
    pub enable_logging: bool,
    pub enabled: bool,
    pub auto_failover_enabled: bool,
    pub max_retries: u32,
    pub streaming_first_byte_timeout: u32,
    pub streaming_idle_timeout: u32,
    pub non_streaming_timeout: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTarget {
    pub app_type: String,
    pub provider_id: String,
    pub provider_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatusSnapshot {
    pub running: bool,
    pub failover_count: u64,
    pub active_targets: Vec<ActiveTarget>,
    pub current_provider_id: Option<String>,
    pub current_provider: Option<String>,
}

/// Map CCS listen_address to a connectable loopback host, or None if non-loopback.
pub fn loopback_connect_host(listen_address: &str) -> Option<String> {
    let a = listen_address.trim();
    if a.is_empty() {
        return Some("127.0.0.1".into());
    }
    let lower = a.to_ascii_lowercase();
    match lower.as_str() {
        "127.0.0.1" | "localhost" => Some("127.0.0.1".into()),
        "0.0.0.0" => Some("127.0.0.1".into()),
        "::1" => Some("::1".into()),
        "::" | "[::]" => Some("::1".into()),
        _ if lower.starts_with("127.") => Some(a.to_string()),
        _ => None,
    }
}

pub fn proxy_config_table_exists(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='proxy_config' LIMIT 1",
        [],
        |_| Ok(()),
    )
    .is_ok()
}

fn has_column(conn: &Connection, table: &str, col: &str) -> bool {
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    let sql = format!("PRAGMA table_info({table})");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return false;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return false;
    };
    let names: Vec<String> = rows.flatten().collect();
    names.iter().any(|c| c == col)
}

/// Read proxy_config rows if the table/columns are compatible. Never fails the scan.
pub fn read_proxy_config_rows(conn: &Connection) -> Result<Vec<ProxyConfigRow>, String> {
    if !proxy_config_table_exists(conn) {
        return Err("proxy_config 表不存在".into());
    }
    if !has_column(conn, "proxy_config", "app_type") {
        return Err("proxy_config 缺少 app_type（旧单例结构，无法安全读取）".into());
    }
    let needed = [
        "proxy_enabled",
        "listen_address",
        "listen_port",
        "enabled",
        "auto_failover_enabled",
    ];
    for c in needed {
        if !has_column(conn, "proxy_config", c) {
            return Err(format!("proxy_config 缺少列 {c}"));
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT app_type, proxy_enabled, listen_address, listen_port, enabled, auto_failover_enabled
             FROM proxy_config ORDER BY app_type",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ProxyConfigRow {
                app_type: row.get(0)?,
                proxy_enabled: row.get::<_, i64>(1)? != 0,
                listen_address: row.get(2)?,
                listen_port: row.get::<_, i64>(3)? as u16,
                enabled: row.get::<_, i64>(4)? != 0,
                auto_failover_enabled: row.get::<_, i64>(5)? != 0,
                enable_logging: true,
                max_retries: 3,
                streaming_first_byte_timeout: 60,
                streaming_idle_timeout: 120,
                non_streaming_timeout: 600,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        let mut row = r.map_err(|e| e.to_string())?;
        // Optional richer columns (best-effort)
        if has_column(conn, "proxy_config", "max_retries") {
            let v: Result<i64, _> = conn.query_row(
                "SELECT max_retries FROM proxy_config WHERE app_type = ?1",
                params![row.app_type],
                |r| r.get(0),
            );
            if let Ok(n) = v {
                row.max_retries = n as u32;
            }
        }
        if has_column(conn, "proxy_config", "auto_failover_enabled") {
            // already loaded
        }
        out.push(row);
    }
    Ok(out)
}

/// Build a safe RoutingStatusView from DB rows (no live probe yet).
pub fn routing_view_from_rows(rows: &[ProxyConfigRow]) -> RoutingStatusView {
    if rows.is_empty() {
        return RoutingStatusView {
            config_detected: false,
            global_enabled: false,
            listen_address: None,
            listen_port: None,
            health_reachable: false,
            server_running: false,
            failover_count: None,
            apps: vec![],
            warning: Some("未检测到 proxy_config 路由配置".into()),
            connect_host: None,
        };
    }

    // Global fields are mirrored across app rows; take first non-empty.
    let listen_address = rows
        .first()
        .map(|r| r.listen_address.clone())
        .unwrap_or_else(|| "127.0.0.1".into());
    let listen_port = rows
        .first()
        .map(|r| r.listen_port)
        .unwrap_or(DEFAULT_LISTEN_PORT);
    let global_enabled = rows.iter().any(|r| r.proxy_enabled || r.enabled);
    let connect_host = loopback_connect_host(&listen_address);

    let mut warning = None;
    if connect_host.is_none() {
        warning = Some(format!(
            "监听地址 {listen_address} 非 loopback，已禁止自动路由探测"
        ));
    }

    let apps = rows
        .iter()
        .map(|r| {
            let app = AppType::parse(&r.app_type);
            AppRoutingStatusView {
                app_type: app.as_str().to_string(),
                app_label: app.label_zh().to_string(),
                enabled: r.enabled,
                auto_failover_enabled: r.auto_failover_enabled,
                max_retries: Some(r.max_retries),
                streaming_first_byte_timeout: Some(r.streaming_first_byte_timeout),
                streaming_idle_timeout: Some(r.streaming_idle_timeout),
                non_streaming_timeout: Some(r.non_streaming_timeout),
                active_provider_id: None,
                active_provider_name: None,
            }
        })
        .collect();

    RoutingStatusView {
        config_detected: true,
        global_enabled,
        listen_address: Some(listen_address),
        listen_port: Some(listen_port),
        health_reachable: false,
        server_running: false,
        failover_count: None,
        apps,
        warning,
        connect_host,
    }
}

/// Probe GET /health and GET /status on loopback only. Timeout ~1.5s.
pub async fn probe_local_route(
    connect_host: &str,
    port: u16,
) -> (bool, bool, Option<ProxyStatusSnapshot>) {
    let base = route_base_url(connect_host, port);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(1500))
        .connect_timeout(Duration::from_millis(800))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
    {
        Ok(c) => c,
        Err(_) => return (false, false, None),
    };

    let health_url = format!("{base}/health");
    let health_ok = match client.get(&health_url).send().await {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    };

    let status_snap = probe_status_only(connect_host, port).await;
    let running = status_snap.as_ref().map(|s| s.running).unwrap_or(health_ok);

    (health_ok, running, status_snap)
}

/// Lightweight GET /status only (loopback). Used for before/after route target checks.
/// Does not send provider keys. Failure returns None without panicking.
pub async fn probe_status_only(connect_host: &str, port: u16) -> Option<ProxyStatusSnapshot> {
    let base = route_base_url(connect_host, port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(1500))
        .connect_timeout(Duration::from_millis(800))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .ok()?;
    let status_url = format!("{base}/status");
    match client.get(&status_url).send().await {
        Ok(r) if r.status().is_success() => {
            let text = r.text().await.ok()?;
            parse_status_json(&text)
        }
        _ => None,
    }
}

/// Active provider id for a given app in a status snapshot, if present.
pub fn active_provider_for_app<'a>(
    snap: &'a ProxyStatusSnapshot,
    app_type: &str,
) -> Option<(&'a str, Option<&'a str>)> {
    if let Some(t) = snap.active_targets.iter().find(|t| t.app_type == app_type) {
        return Some((t.provider_id.as_str(), Some(t.provider_name.as_str())));
    }
    snap.current_provider_id
        .as_deref()
        .map(|id| (id, snap.current_provider.as_deref()))
}

fn parse_status_json(text: &str) -> Option<ProxyStatusSnapshot> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let running = v.get("running").and_then(|x| x.as_bool()).unwrap_or(true);
    let failover_count = v
        .get("failover_count")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let current_provider_id = v
        .get("current_provider_id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let current_provider = v
        .get("current_provider")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());

    let mut active_targets = Vec::new();
    if let Some(arr) = v.get("active_targets").and_then(|x| x.as_array()) {
        for item in arr {
            let app_type = item
                .get("app_type")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let provider_id = item
                .get("provider_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let provider_name = item
                .get("provider_name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if !app_type.is_empty() && !provider_id.is_empty() {
                active_targets.push(ActiveTarget {
                    app_type,
                    provider_id,
                    provider_name,
                });
            }
        }
    }

    Some(ProxyStatusSnapshot {
        running,
        failover_count,
        active_targets,
        current_provider_id,
        current_provider,
    })
}

/// Merge live status into routing view (mutates apps' active targets).
pub fn apply_status_snapshot(view: &mut RoutingStatusView, snap: &ProxyStatusSnapshot) {
    view.server_running = snap.running;
    view.failover_count = Some(snap.failover_count);
    for app in &mut view.apps {
        if let Some(t) = snap
            .active_targets
            .iter()
            .find(|t| t.app_type == app.app_type)
        {
            app.active_provider_id = Some(t.provider_id.clone());
            app.active_provider_name = Some(t.provider_name.clone());
        } else if let Some(id) = &snap.current_provider_id {
            // Fallback single current target when active_targets missing
            if app.enabled {
                app.active_provider_id = Some(id.clone());
                app.active_provider_name = snap.current_provider.clone();
            }
        }
    }
}

/// Full read-only discovery: DB rows + optional loopback probe.
pub async fn discover_routing_status(conn: &Connection) -> RoutingStatusView {
    let rows = match read_proxy_config_rows(conn) {
        Ok(r) => r,
        Err(msg) => {
            return RoutingStatusView {
                config_detected: false,
                global_enabled: false,
                listen_address: None,
                listen_port: None,
                health_reachable: false,
                server_running: false,
                failover_count: None,
                apps: vec![],
                warning: Some(format!("路由状态不可用：{msg}")),
                connect_host: None,
            };
        }
    };

    let mut view = routing_view_from_rows(&rows);
    if let (Some(host), Some(port)) = (view.connect_host.clone(), view.listen_port) {
        let (health, running, snap) = probe_local_route(&host, port).await;
        view.health_reachable = health;
        view.server_running = running || health;
        if let Some(s) = snap {
            apply_status_snapshot(&mut view, &s);
        } else if !health && view.global_enabled {
            view.warning = Some(
                view.warning
                    .clone()
                    .unwrap_or_else(|| "CCS 路由已配置但本地服务未响应 /health".into()),
            );
        }
    }
    view
}

/// Synchronous fallback used from non-async scan paths (probe skipped).
pub fn discover_routing_status_sync(conn: &Connection) -> RoutingStatusView {
    match read_proxy_config_rows(conn) {
        Ok(rows) => {
            let mut view = routing_view_from_rows(&rows);
            if view.global_enabled && view.connect_host.is_some() {
                view.warning = Some("已读取路由配置；健康探测请在异步扫描路径完成".into());
            }
            view
        }
        Err(msg) => RoutingStatusView {
            config_detected: false,
            global_enabled: false,
            listen_address: None,
            listen_port: None,
            health_reachable: false,
            server_running: false,
            failover_count: None,
            apps: vec![],
            warning: Some(format!("路由状态不可用：{msg}")),
            connect_host: None,
        },
    }
}

/// Assert no write SQL touches proxy_config (used by security tests).
pub fn assert_no_proxy_writes(sql: &str) -> PublicResult<()> {
    let lower = sql.to_ascii_lowercase();
    if lower.contains("proxy_config")
        && (lower.contains("insert ")
            || lower.contains("update ")
            || lower.contains("delete ")
            || lower.contains("drop ")
            || lower.contains("alter "))
    {
        return Err(PublicError::Database(
            "禁止对 proxy_config 执行写操作".into(),
        ));
    }
    Ok(())
}

/// Build client-facing route base URL for an app (loopback only).
pub fn route_base_url(connect_host: &str, port: u16) -> String {
    if connect_host.contains(':') && !connect_host.starts_with('[') {
        format!("http://[{connect_host}]:{port}")
    } else {
        format!("http://{connect_host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn seed_proxy(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE proxy_config (
                app_type TEXT PRIMARY KEY,
                proxy_enabled INTEGER NOT NULL DEFAULT 0,
                listen_address TEXT NOT NULL DEFAULT '127.0.0.1',
                listen_port INTEGER NOT NULL DEFAULT 15721,
                enable_logging INTEGER NOT NULL DEFAULT 1,
                enabled INTEGER NOT NULL DEFAULT 0,
                auto_failover_enabled INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 3,
                streaming_first_byte_timeout INTEGER NOT NULL DEFAULT 60,
                streaming_idle_timeout INTEGER NOT NULL DEFAULT 120,
                non_streaming_timeout INTEGER NOT NULL DEFAULT 600
            );
            INSERT INTO proxy_config (app_type, proxy_enabled, enabled, auto_failover_enabled, listen_port)
            VALUES ('claude', 1, 1, 0, 15721);
            INSERT INTO proxy_config (app_type, proxy_enabled, enabled, listen_port)
            VALUES ('codex', 1, 0, 15721);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn loopback_mapping() {
        assert_eq!(
            loopback_connect_host("0.0.0.0").as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(loopback_connect_host("::").as_deref(), Some("::1"));
        assert_eq!(
            loopback_connect_host("127.0.0.1").as_deref(),
            Some("127.0.0.1")
        );
        assert!(loopback_connect_host("192.168.1.1").is_none());
        assert!(loopback_connect_host("10.0.0.5").is_none());
    }

    #[test]
    fn reads_proxy_config_readonly() {
        let conn = Connection::open_in_memory().unwrap();
        seed_proxy(&conn);
        let rows = read_proxy_config_rows(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        let claude = rows.iter().find(|r| r.app_type == "claude").unwrap();
        assert!(claude.enabled);
        assert!(claude.proxy_enabled);
        assert_eq!(claude.listen_port, 15721);
        let view = routing_view_from_rows(&rows);
        assert!(view.config_detected);
        assert!(view.global_enabled);
        assert_eq!(view.connect_host.as_deref(), Some("127.0.0.1"));
    }

    #[test]
    fn missing_table_is_soft_failure() {
        let conn = Connection::open_in_memory().unwrap();
        let err = read_proxy_config_rows(&conn).unwrap_err();
        assert!(err.contains("不存在"));
        let view = discover_routing_status_sync(&conn);
        assert!(!view.config_detected);
        assert!(view.warning.as_ref().unwrap().contains("不可用"));
    }

    #[test]
    fn non_loopback_blocks_auto_probe() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE proxy_config (
                app_type TEXT PRIMARY KEY,
                proxy_enabled INTEGER, listen_address TEXT, listen_port INTEGER,
                enable_logging INTEGER, enabled INTEGER, auto_failover_enabled INTEGER,
                max_retries INTEGER, streaming_first_byte_timeout INTEGER,
                streaming_idle_timeout INTEGER, non_streaming_timeout INTEGER
            );
            INSERT INTO proxy_config VALUES ('claude',1,'192.168.0.10',15721,1,1,0,3,60,120,600);
            "#,
        )
        .unwrap();
        let rows = read_proxy_config_rows(&conn).unwrap();
        let view = routing_view_from_rows(&rows);
        assert!(view.connect_host.is_none());
        assert!(view.warning.as_ref().unwrap().contains("loopback"));
    }

    #[test]
    fn parse_status_active_targets() {
        let raw = r#"{
            "running": true,
            "failover_count": 2,
            "active_targets": [
                {"app_type":"claude","provider_id":"p1","provider_name":"Relay"}
            ],
            "current_provider_id": "p1",
            "current_provider": "Relay"
        }"#;
        let snap = parse_status_json(raw).unwrap();
        assert!(snap.running);
        assert_eq!(snap.failover_count, 2);
        assert_eq!(snap.active_targets.len(), 1);
        assert_eq!(snap.active_targets[0].provider_id, "p1");
    }

    #[test]
    fn placeholder_token_constant() {
        assert_eq!(CCS_PROXY_PLACEHOLDER_TOKEN, "PROXY_MANAGED");
        assert!(!CCS_PROXY_PLACEHOLDER_TOKEN.starts_with("sk-"));
    }

    #[test]
    fn rejects_proxy_write_sql() {
        assert!(assert_no_proxy_writes("SELECT * FROM proxy_config").is_ok());
        assert!(assert_no_proxy_writes("UPDATE proxy_config SET enabled=1").is_err());
        assert!(assert_no_proxy_writes("INSERT INTO proxy_config VALUES (1)").is_err());
    }

    #[test]
    fn apply_status_fills_active() {
        let conn = Connection::open_in_memory().unwrap();
        seed_proxy(&conn);
        let rows = read_proxy_config_rows(&conn).unwrap();
        let mut view = routing_view_from_rows(&rows);
        let snap = ProxyStatusSnapshot {
            running: true,
            failover_count: 1,
            active_targets: vec![ActiveTarget {
                app_type: "claude".into(),
                provider_id: "glm-1".into(),
                provider_name: "GLM".into(),
            }],
            current_provider_id: Some("glm-1".into()),
            current_provider: Some("GLM".into()),
        };
        apply_status_snapshot(&mut view, &snap);
        assert!(view.server_running);
        let claude = view.apps.iter().find(|a| a.app_type == "claude").unwrap();
        assert_eq!(claude.active_provider_id.as_deref(), Some("glm-1"));
    }
}
