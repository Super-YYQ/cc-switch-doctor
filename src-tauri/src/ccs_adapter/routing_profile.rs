//! CCS routing client protocol profile — loaded from compatibility/manifest.json.
//!
//! Source: docs/research/v0.1.7-source-review.md
//! (farion1231/cc-switch DEFAULT_PROXY_ROUTES + universal presets).
//!
//! Business code must not scatter hard-coded dated Claude model IDs for route probes.

use once_cell::sync::Lazy;
use serde::Deserialize;

/// Embedded verified routing profiles from the compatibility manifest.
static ROUTING_PROFILES: Lazy<Vec<RoutingProfile>> = Lazy::new(|| {
    let raw = include_str!("../../../compatibility/manifest.json");
    let v: serde_json::Value = serde_json::from_str(raw).unwrap_or_else(|e| {
        panic!("compatibility/manifest.json invalid JSON: {e}");
    });
    let arr = v
        .get("routingProfiles")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    arr.into_iter()
        .filter_map(|item| serde_json::from_value::<RoutingProfile>(item).ok())
        .collect()
});

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeRoleModels {
    pub haiku: String,
    pub sonnet: String,
    pub opus: String,
    #[serde(default)]
    pub fable: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingProfile {
    pub id: String,
    pub cc_switch_commit: String,
    pub release_range: String,
    pub placeholder_token: String,
    pub claude_client_models: ClaudeRoleModels,
    #[serde(default)]
    pub default_route_model_by_app: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub routes: std::collections::HashMap<String, Vec<String>>,
    #[serde(default)]
    pub sources: Vec<String>,
}

/// Active verified profile used for route probes in this Doctor build.
///
/// Returns None only if the manifest is missing routingProfiles (should not
/// happen in release). Callers must not invent aliases when None.
pub fn active_routing_profile() -> Option<&'static RoutingProfile> {
    ROUTING_PROFILES.first()
}

/// Whether Doctor may send real CCS route business requests.
pub fn route_profile_verified() -> bool {
    active_routing_profile().is_some()
}

/// Placeholder token from profile (falls back to well-known PROXY_MANAGED).
pub fn placeholder_token() -> &'static str {
    active_routing_profile()
        .map(|p| p.placeholder_token.as_str())
        .unwrap_or("PROXY_MANAGED")
}

/// Client-facing model for a CCS route probe, from the verified profile.
pub fn client_route_model(app: crate::ccs_adapter::AppType) -> Option<String> {
    let profile = active_routing_profile()?;
    let key = app.as_str();
    if let Some(m) = profile.default_route_model_by_app.get(key) {
        return Some(m.clone());
    }
    match app {
        crate::ccs_adapter::AppType::Claude | crate::ccs_adapter::AppType::ClaudeDesktop => {
            Some(profile.claude_client_models.sonnet.clone())
        }
        crate::ccs_adapter::AppType::Codex => profile
            .default_route_model_by_app
            .get("codex")
            .cloned()
            .or_else(|| Some("gpt-5.5".into())),
        crate::ccs_adapter::AppType::Gemini => profile
            .default_route_model_by_app
            .get("gemini")
            .cloned()
            .or_else(|| Some("gemini-3.5-flash".into())),
        _ => None,
    }
}

/// Default model guess for direct-upstream planning when the DB has no model.
/// Prefer profile client defaults so Doctor does not invent dated Anthropic IDs.
pub fn default_direct_model_guess(app: crate::ccs_adapter::AppType) -> String {
    client_route_model(app).unwrap_or_else(|| match app {
        crate::ccs_adapter::AppType::Claude | crate::ccs_adapter::AppType::ClaudeDesktop => {
            "claude-sonnet-5".into()
        }
        crate::ccs_adapter::AppType::Codex => "gpt-5.5".into(),
        crate::ccs_adapter::AppType::Gemini => "gemini-3.5-flash".into(),
        _ => "gpt-4o-mini".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccs_adapter::AppType;

    #[test]
    fn manifest_has_verified_routing_profile() {
        let p = active_routing_profile().expect("routingProfiles required in manifest");
        assert!(!p.cc_switch_commit.is_empty());
        assert_eq!(p.placeholder_token, "PROXY_MANAGED");
        assert_eq!(p.claude_client_models.sonnet, "claude-sonnet-5");
        assert_eq!(p.claude_client_models.opus, "claude-opus-4-8");
        assert_eq!(p.claude_client_models.haiku, "claude-haiku-4-5");
        assert_eq!(
            p.claude_client_models.fable.as_deref(),
            Some("claude-fable-5")
        );
    }

    #[test]
    fn claude_route_model_is_role_alias_not_dated() {
        let m = client_route_model(AppType::Claude).unwrap();
        assert_eq!(m, "claude-sonnet-5");
        assert!(!m.contains("20250514"));
        assert!(!m.contains("20250929"));
    }

    #[test]
    fn no_scattered_legacy_date_alias_in_profile() {
        let p = active_routing_profile().unwrap();
        let blob = format!("{:?}", p.claude_client_models);
        assert!(!blob.contains("claude-sonnet-4-20250514"));
    }

    #[test]
    fn codex_and_gemini_defaults_from_profile() {
        assert_eq!(
            client_route_model(AppType::Codex).as_deref(),
            Some("gpt-5.5")
        );
        assert_eq!(
            client_route_model(AppType::Gemini).as_deref(),
            Some("gemini-3.5-flash")
        );
    }
}
