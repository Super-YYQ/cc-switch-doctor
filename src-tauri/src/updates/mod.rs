use serde::{Deserialize, Serialize};

const CC_SWITCH_REPO: &str = "farion1231/cc-switch";
const DOCTOR_REPO: &str = "Super-YYQ/cc-switch-doctor";
const DOCTOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub doctor_version: String,
    pub doctor_latest: Option<String>,
    pub doctor_update_available: bool,
    pub doctor_release_url: Option<String>,
    pub cc_switch_latest: Option<String>,
    pub cc_switch_release_url: Option<String>,
    pub verified_cc_switch: String,
    pub message: String,
    pub checked: bool,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
}

pub async fn check_updates_now() -> UpdateStatus {
    let verified = load_verified_release();
    let observed = load_observed_release();
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent(format!("CC-Switch-Doctor/{DOCTOR_VERSION}"))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return UpdateStatus {
                doctor_version: DOCTOR_VERSION.into(),
                doctor_latest: None,
                doctor_update_available: false,
                doctor_release_url: None,
                cc_switch_latest: None,
                cc_switch_release_url: None,
                verified_cc_switch: verified,
                message: "更新检查客户端初始化失败".into(),
                checked: false,
                error: Some(e.to_string()),
            };
        }
    };

    let doctor = fetch_latest(&client, DOCTOR_REPO).await;
    let cc = fetch_latest(&client, CC_SWITCH_REPO).await;

    let doctor_latest = doctor.as_ref().ok().map(|(t, _)| strip_v(t));
    let doctor_url = doctor.as_ref().ok().map(|(_, u)| u.clone());
    let cc_latest = cc
        .as_ref()
        .ok()
        .map(|(t, _)| strip_v(t))
        .or(Some(observed.clone()));
    let cc_url = cc.as_ref().ok().map(|(_, u)| u.clone());

    let doctor_update = doctor_latest
        .as_ref()
        .map(|l| l != DOCTOR_VERSION && is_newer(l, DOCTOR_VERSION))
        .unwrap_or(false);

    let mut messages = Vec::new();
    if let Some(l) = &cc_latest {
        messages.push(format_cc_switch_status(l, &verified, &observed));
    }
    if doctor_update {
        messages.push(format!(
            "Doctor 当前 {DOCTOR_VERSION}，最新 {}，建议更新",
            doctor_latest.as_deref().unwrap_or("?")
        ));
    } else {
        messages.push(format!("Doctor 当前版本 {DOCTOR_VERSION}"));
    }

    let err = match (&doctor, &cc) {
        (Err(e), _) | (_, Err(e)) => Some(e.clone()),
        _ => None,
    };

    UpdateStatus {
        doctor_version: DOCTOR_VERSION.into(),
        doctor_latest,
        doctor_update_available: doctor_update,
        doctor_release_url: doctor_url,
        cc_switch_latest: cc_latest,
        cc_switch_release_url: cc_url,
        verified_cc_switch: verified,
        message: messages.join("；"),
        checked: doctor.is_ok() || cc.is_ok(),
        error: err,
    }
}

fn format_cc_switch_status(latest: &str, verified: &str, observed: &str) -> String {
    let _ = observed;
    if latest == verified {
        format!("CC Switch 最新：{latest}；Doctor 已验证：{verified}；状态：已验证")
    } else {
        format!(
            "CC Switch 最新：{latest}；Doctor 已验证：{verified}；状态：发现新版本，尚未完成兼容验证。检测到 CC Switch 新版本，但 Doctor 尚未完成该版本的兼容验证。当前不会自动升级兼容结论。"
        )
    }
}

async fn fetch_latest(client: &reqwest::Client, repo: &str) -> Result<(String, String), String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub HTTP {}", resp.status()));
    }
    let rel: GhRelease = resp.json().await.map_err(|e| e.to_string())?;
    Ok((rel.tag_name, rel.html_url))
}

fn strip_v(tag: &str) -> String {
    tag.trim().trim_start_matches('v').to_string()
}

fn is_newer(latest: &str, current: &str) -> bool {
    parse_semver(latest) > parse_semver(current)
}

fn parse_semver(s: &str) -> (u64, u64, u64) {
    let mut parts = s.split('.');
    let a = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let b = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let c = parts
        .next()
        .and_then(|x| x.split('-').next().unwrap_or(x).parse().ok())
        .unwrap_or(0);
    (a, b, c)
}

fn load_verified_release() -> String {
    let raw = include_str!("../../../compatibility/manifest.json");
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(s) = v
            .pointer("/ccSwitch/latestVerifiedRelease")
            .and_then(|x| x.as_str())
        {
            return s.to_string();
        }
        // Safe fallback: first verifiedReleases entry, never upgrade via observed.
        if let Some(s) = v
            .pointer("/ccSwitch/verifiedReleases/0")
            .and_then(|x| x.as_str())
        {
            return s.to_string();
        }
    }
    "3.17.0".into()
}

fn load_observed_release() -> String {
    let raw = include_str!("../../../compatibility/manifest.json");
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(s) = v
            .pointer("/ccSwitch/latestObservedRelease")
            .and_then(|x| x.as_str())
        {
            return s.to_string();
        }
    }
    load_verified_release()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(is_newer("3.18.0", "3.17.0"));
    }

    #[test]
    fn verified_reads_verified_field_not_observed() {
        let v = load_verified_release();
        assert_eq!(v, "3.17.0");
    }

    #[test]
    fn message_when_newer_not_verified() {
        let msg = format_cc_switch_status("3.18.0", "3.17.0", "3.18.0");
        assert!(msg.contains("尚未完成兼容验证"));
        assert!(!msg.contains("与已验证基线一致或兼容"));
    }

    #[test]
    fn message_when_same_verified() {
        let msg = format_cc_switch_status("3.17.0", "3.17.0", "3.17.0");
        assert!(msg.contains("已验证"));
        assert!(!msg.contains("尚未完成兼容验证"));
    }
}
