use super::models::DiscoveryInfo;
use std::path::{Path, PathBuf};

/// Discover CC Switch database candidates.
/// Only reads CC Switch data dirs — never AI login homes.
pub fn discover_database_paths() -> DiscoveryInfo {
    let mut tried: Vec<String> = Vec::new();

    // 1) Explicit test/debug override for Doctor (not persisted)
    if let Ok(p) = std::env::var("CC_SWITCH_DOCTOR_DB") {
        let path = PathBuf::from(p.trim());
        if path.is_file() {
            return found(path, "env:CC_SWITCH_DOCTOR_DB");
        }
        tried.push(format!(
            "env CC_SWITCH_DOCTOR_DB missing: {}",
            path.display()
        ));
    }

    // 2) CC Switch app_paths.json / app_config_dir_override (Tauri id com.ccswitch.desktop)
    if let Some(custom) = read_cc_switch_app_config_dir_override() {
        let db = custom.join("cc-switch.db");
        if db.is_file() {
            return found(db, "app_paths.json:app_config_dir_override");
        }
        tried.push(format!(
            "app_config_dir_override missing db: {}",
            db.display()
        ));
    }

    // 3) Legacy custom keys under ~/.cc-switch (settings.json etc.)
    if let Some(custom) = read_legacy_custom_data_dir() {
        let db = custom.join("cc-switch.db");
        if db.is_file() {
            return found(db, "cc-switch-custom-data-dir");
        }
        tried.push(format!("custom data dir missing db: {}", db.display()));
    }

    // 4) Default: real user home /.cc-switch/cc-switch.db
    if let Some(home) = dirs::home_dir() {
        let db = home.join(".cc-switch").join("cc-switch.db");
        if db.is_file() {
            return found(db, "default-home");
        }
        tried.push(format!("default missing: {}", db.display()));
    } else {
        tried.push("dirs::home_dir unavailable".into());
    }

    // 5) Windows legacy HOME env fallback (only when default missing)
    #[cfg(windows)]
    {
        if let Ok(home_env) = std::env::var("HOME") {
            let trimmed = home_env.trim();
            if !trimmed.is_empty() {
                let db = PathBuf::from(trimmed)
                    .join(".cc-switch")
                    .join("cc-switch.db");
                if db.is_file() {
                    return found(db, "windows-legacy-HOME");
                }
                tried.push(format!("legacy HOME missing: {}", db.display()));
            }
        }
    }

    // 6) Common portable / local app data probes (read-only)
    for candidate in portable_candidates() {
        if candidate.is_file() {
            return found(candidate, "portable-probe");
        }
        tried.push(format!("portable missing: {}", candidate.display()));
    }

    DiscoveryInfo {
        found: false,
        database_path: None,
        data_dir: None,
        source: None,
        message: format!(
            "未找到 CC Switch 数据库。请确认已安装并至少启动过一次 CC Switch，或手动选择 cc-switch.db。已尝试：{}",
            tried.join(" | ")
        ),
    }
}

fn found(db: PathBuf, source: &str) -> DiscoveryInfo {
    let data_dir = db.parent().map(|p| p.to_path_buf());
    DiscoveryInfo {
        found: true,
        database_path: Some(db),
        data_dir,
        source: Some(source.to_string()),
        message: format!("已定位 CC Switch 数据库（来源：{source}）"),
    }
}

fn portable_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = dirs::data_local_dir() {
        out.push(local.join("cc-switch").join("cc-switch.db"));
        out.push(local.join("CC-Switch").join("cc-switch.db"));
    }
    if let Some(roaming) = dirs::config_dir() {
        out.push(roaming.join("cc-switch").join("cc-switch.db"));
        out.push(roaming.join("CC-Switch").join("cc-switch.db"));
    }
    out
}

/// Resolve Tauri store dirs for identifier `com.ccswitch.desktop`.
fn ccswitch_store_dirs() -> Vec<PathBuf> {
    let mut dirs_out = Vec::new();
    // Windows: %APPDATA%\com.ccswitch.desktop and nested forms
    if let Some(config) = dirs::config_dir() {
        dirs_out.push(config.join("com.ccswitch.desktop"));
        dirs_out.push(config.join("ccswitch").join("com.ccswitch.desktop"));
    }
    if let Some(local) = dirs::data_local_dir() {
        dirs_out.push(local.join("com.ccswitch.desktop"));
        // Some Tauri builds use WebView2 package family style under Local
        dirs_out.push(local.join("com.ccswitch.desktop").join("EBWebView"));
    }
    if let Some(home) = dirs::home_dir() {
        dirs_out.push(home.join(".cc-switch"));
        dirs_out.push(
            home.join("AppData")
                .join("Roaming")
                .join("com.ccswitch.desktop"),
        );
        dirs_out.push(
            home.join("AppData")
                .join("Local")
                .join("com.ccswitch.desktop"),
        );
    }
    dirs_out
}

/// Read `app_paths.json` → `app_config_dir_override` (primary CC Switch mechanism).
fn read_cc_switch_app_config_dir_override() -> Option<PathBuf> {
    for dir in ccswitch_store_dirs() {
        let path = dir.join("app_paths.json");
        if let Some(p) = parse_app_paths_override(&path) {
            return Some(p);
        }
    }
    None
}

fn parse_app_paths_override(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    // tauri-plugin-store may nest as plain object or { "app_config_dir_override": "..." }
    let raw = v
        .get("app_config_dir_override")
        .or_else(|| v.pointer("/app_config_dir_override"))
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    Some(expand_user_path(&raw))
}

fn read_legacy_custom_data_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    for name in [
        "settings.json",
        "app-store.json",
        "store.json",
        "app_paths.json",
    ] {
        let settings = home.join(".cc-switch").join(name);
        if let Some(p) = parse_override_file(&settings) {
            return Some(p);
        }
    }
    None
}

fn parse_override_file(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    for key in [
        "app_config_dir_override",
        "appConfigDir",
        "app_config_dir",
        "customDataDir",
        "custom_data_dir",
        "dataDir",
        "data_dir",
    ] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(expand_user_path(s));
            }
        }
    }
    None
}

/// Expand `~`, `~/…`, `~\…`, keep drive letters and UNC.
pub fn expand_user_path(raw: &str) -> PathBuf {
    let s = raw.trim();
    if s == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(s));
    }
    if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn discovers_env_override() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("cc-switch.db");
        std::fs::write(&db, b"not-a-real-db").unwrap();
        std::env::set_var("CC_SWITCH_DOCTOR_DB", db.to_string_lossy().as_ref());
        let info = discover_database_paths();
        std::env::remove_var("CC_SWITCH_DOCTOR_DB");
        assert!(info.found);
        assert_eq!(info.source.as_deref(), Some("env:CC_SWITCH_DOCTOR_DB"));
    }

    #[test]
    fn missing_db_reports_message() {
        std::env::set_var("CC_SWITCH_DOCTOR_DB", "Z:/definitely/missing/cc-switch.db");
        let info = discover_database_paths();
        std::env::remove_var("CC_SWITCH_DOCTOR_DB");
        let _ = info.message;
    }

    #[test]
    fn parse_override_reads_custom_dir() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("settings.json");
        let mut file = std::fs::File::create(&f).unwrap();
        write!(file, r#"{{"appConfigDir":"D:/custom-cc"}}"#).unwrap();
        let p = parse_override_file(&f).unwrap();
        assert_eq!(p, PathBuf::from("D:/custom-cc"));
    }

    #[test]
    fn parse_app_paths_override_key() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("app_paths.json");
        std::fs::write(&f, r#"{"app_config_dir_override":"E:\\data\\ccs"}"#).unwrap();
        let p = parse_app_paths_override(&f).unwrap();
        assert_eq!(p, PathBuf::from(r"E:\data\ccs"));
    }

    #[test]
    fn expand_tilde() {
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_user_path("~"), home);
            assert_eq!(expand_user_path("~/foo/bar"), home.join("foo/bar"));
        }
    }

    #[test]
    fn expand_unc_and_drive() {
        assert_eq!(
            expand_user_path(r"\\server\share\data"),
            PathBuf::from(r"\\server\share\data")
        );
        assert_eq!(expand_user_path(r"D:\custom"), PathBuf::from(r"D:\custom"));
    }

    #[test]
    fn corrupt_store_returns_none() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("app_paths.json");
        std::fs::write(&f, "{not-json").unwrap();
        assert!(parse_app_paths_override(&f).is_none());
    }
}
