use super::models::AuthKind;
use serde_json::Value;

/// Detect managed / official auth that must be skipped (no bypass).
pub fn detect_managed_auth(
    app_type: &str,
    meta: &Value,
    settings: &Value,
    category: Option<&str>,
) -> Option<(AuthKind, String)> {
    let provider_type = meta
        .get("providerType")
        .or_else(|| meta.get("provider_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if provider_type.eq_ignore_ascii_case("codex_oauth") {
        return Some((
            AuthKind::CodexOAuth,
            "安全跳过：Codex OAuth / 官方登录（不读取登录缓存，不提供绕过）".into(),
        ));
    }
    if provider_type.eq_ignore_ascii_case("github_copilot") {
        return Some((
            AuthKind::GitHubCopilot,
            "安全跳过：GitHub Copilot 托管认证".into(),
        ));
    }

    let base_candidates = collect_base_url_hints(settings);
    for b in &base_candidates {
        let lower = b.to_ascii_lowercase();
        if lower.contains("chatgpt.com/backend-api/codex") {
            return Some((
                AuthKind::CodexOAuth,
                "安全跳过：ChatGPT Backend Codex 托管端点".into(),
            ));
        }
        if lower.contains("githubcopilot.com") {
            return Some((
                AuthKind::GitHubCopilot,
                "安全跳过：GitHub Copilot 端点".into(),
            ));
        }
        if lower.contains("api.openai.com") && app_type == "codex" && provider_type.is_empty() {
            // Not automatic skip — official API key configs are testable.
        }
        if lower.contains("api.anthropic.com")
            && category
                .map(|c| c.eq_ignore_ascii_case("official"))
                .unwrap_or(false)
        {
            return Some((
                AuthKind::OfficialSubscription,
                "安全跳过：官方订阅/官方端点配置".into(),
            ));
        }
    }

    // authBinding managed_account
    if let Some(binding) = meta.get("authBinding").or_else(|| meta.get("auth_binding")) {
        let kind = binding
            .get("kind")
            .or_else(|| binding.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if kind.contains("managed") || kind.contains("oauth") {
            return Some((AuthKind::ManagedOAuth, "安全跳过：托管账户认证绑定".into()));
        }
    }

    if category
        .map(|c| c.eq_ignore_ascii_case("official"))
        .unwrap_or(false)
        && provider_type.is_empty()
    {
        // Official category without static key will be caught later by empty key.
    }

    None
}

fn collect_base_url_hints(settings: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let pointers = [
        "/env/ANTHROPIC_BASE_URL",
        "/env/GOOGLE_GEMINI_BASE_URL",
        "/options/baseURL",
        "/baseUrl",
        "/base_url",
        "/auth/base_url",
    ];
    for p in pointers {
        if let Some(s) = settings.pointer(p).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                out.push(s.to_string());
            }
        }
    }
    if let Some(cfg) = settings.get("config").and_then(|v| v.as_str()) {
        out.push(cfg.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn skips_codex_oauth() {
        let meta = json!({"providerType": "codex_oauth"});
        let r = detect_managed_auth("codex", &meta, &json!({}), None).unwrap();
        assert_eq!(r.0, AuthKind::CodexOAuth);
    }

    #[test]
    fn skips_copilot_url() {
        let settings = json!({"env":{"ANTHROPIC_BASE_URL":"https://api.githubcopilot.com"}});
        let r = detect_managed_auth("claude", &json!({}), &settings, None).unwrap();
        assert_eq!(r.0, AuthKind::GitHubCopilot);
    }

    #[test]
    fn allows_third_party() {
        let settings = json!({"env":{"ANTHROPIC_BASE_URL":"https://api.relay.test","ANTHROPIC_AUTH_TOKEN":"sk-x"}});
        assert!(detect_managed_auth("claude", &json!({}), &settings, Some("custom")).is_none());
    }
}
