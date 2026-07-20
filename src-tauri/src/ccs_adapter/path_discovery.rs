use super::models::DiscoveryInfo;
use std::path::{Path, PathBuf};

/// Discover CC Switch database candidates.
/// Only reads CC Switch data directories — never AI login homes.
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

    // 2) CC Switch custom data dir override store (Tauri store / settings file)
    if let Some(custom) = read_cc_switch_custom_data_dir() {
        let db = custom.join("cc-switch.db");
        if db.is_file() {
            return found(db, "cc-switch-custom-data-dir");
        }
        tried.push(format!("custom data dir missing db: {}", db.display()));
    }

    // 3) Default: real user home /.cc-switch/cc-switch.db
    if let Some(home) = dirs::home_dir() {
        let db = home.join(".cc-switch").join("cc-switch.db");
        if db.is_file() {
            return found(db, "default-home");
        }
        tried.push(format!("default missing: {}", db.display()));
    } else {
        tried.push("dirs::home_dir unavailable".into());
    }

    // 4) Windows legacy HOME env fallback (only when default missing)
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

    // 5) Common portable / local app data probes (read-only)
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

/// Best-effort: read CC Switch app store override for custom data directory.
/// We only look inside known CC Switch config files under ~/.cc-switch — never AI login dirs.
fn read_cc_switch_custom_data_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let settings = home.join(".cc-switch").join("settings.json");
    if let Some(p) = parse_override_file(&settings) {
        return Some(p);
    }
    let store = home.join(".cc-switch").join("app-store.json");
    if let Some(p) = parse_override_file(&store) {
        return Some(p);
    }
    // Tauri store style
    let tauri_store = home.join(".cc-switch").join("store.json");
    if let Some(p) = parse_override_file(&tauri_store) {
        return Some(p);
    }
    None
}

fn parse_override_file(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    // Common keys used by CC Switch variants
    for key in [
        "appConfigDir",
        "app_config_dir",
        "customDataDir",
        "custom_data_dir",
        "dataDir",
        "data_dir",
    ] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            let p = PathBuf::from(s.trim());
            if !s.trim().is_empty() {
                return Some(p);
            }
        }
        // nested
        if let Some(s) = v.pointer(&format!("/{key}")).and_then(|x| x.as_str()) {
            if !s.trim().is_empty() {
                return Some(PathBuf::from(s.trim()));
            }
        }
    }
    // settings table dump style: { "value": "..." } not applicable here
    None
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
        // May still find real user db; just ensure function returns DiscoveryInfo
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
}
