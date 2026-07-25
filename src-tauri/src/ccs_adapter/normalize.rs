use super::managed_auth::detect_managed_auth;
use super::model_semantics::{ModelCandidate, ModelCandidateSource};
use super::models::{AppType, AuthKind, NormalizedProvider, ProtocolKind, ProviderKind};
use crate::security::redact::{
    mask_api_key, sanitize_url_for_display, sanitize_url_with_redactor, SecretRedactor,
};
use secrecy::SecretString;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RawProviderRow {
    pub id: String,
    pub app_type: String,
    pub name: String,
    pub settings_config: String,
    pub website_url: Option<String>,
    pub category: Option<String>,
    pub meta: String,
    pub is_current: bool,
    pub endpoint_urls: Vec<String>,
}

pub fn normalize_provider(raw: RawProviderRow) -> NormalizedProvider {
    let app_type = AppType::parse(&raw.app_type);
    let settings: Value = serde_json::from_str(&raw.settings_config).unwrap_or(Value::Null);
    let meta: Value = serde_json::from_str(&raw.meta).unwrap_or(Value::Object(Default::default()));

    let managed = detect_managed_auth(&raw.app_type, &meta, &settings, raw.category.as_deref());

    let (base_url, api_key, auth_kind_default, protocol_default, model) =
        extract_credentials(app_type, &settings, &meta);

    let (auth_kind, skip_reason, provider_kind) = if let Some((ak, reason)) = managed {
        (ak, Some(reason), ProviderKind::ManagedAccount)
    } else if api_key.trim().is_empty() {
        (
            AuthKind::Unknown,
            Some("安全跳过：数据库中无可用的静态第三方 API Key".into()),
            ProviderKind::Unknown,
        )
    } else if base_url.trim().is_empty() {
        (
            auth_kind_default,
            Some("安全跳过：缺少 Base URL".into()),
            ProviderKind::ThirdPartyApi,
        )
    } else {
        (auth_kind_default, None, ProviderKind::ThirdPartyApi)
    };

    let api_format_hint = meta
        .get("apiFormat")
        .or_else(|| meta.get("api_format"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let configured_protocol = resolve_protocol(
        app_type,
        api_format_hint.as_deref(),
        protocol_default,
        &settings,
    );

    let mut model_candidates: Vec<ModelCandidate> = Vec::new();
    let claude_family = matches!(app_type, AppType::Claude | AppType::ClaudeDesktop);

    if let Some(ref m) = model {
        if claude_family {
            model_candidates.push(ModelCandidate::from_configured_claude(m));
        } else {
            model_candidates.push(ModelCandidate::from_configured_plain(m));
        }
    }

    // Role / additional model fields — keep source so success is not "model variant".
    let role_fields: &[(&str, &str)] = if claude_family {
        &[
            ("/env/ANTHROPIC_MODEL", "primary"),
            ("/env/ANTHROPIC_DEFAULT_SONNET_MODEL", "sonnet"),
            ("/env/ANTHROPIC_DEFAULT_OPUS_MODEL", "opus"),
            ("/env/ANTHROPIC_DEFAULT_HAIKU_MODEL", "haiku"),
            ("/env/ANTHROPIC_DEFAULT_FABLE_MODEL", "fable"),
            ("/env/CLAUDE_CODE_SUBAGENT_MODEL", "subagent"),
        ]
    } else {
        &[
            ("/env/GEMINI_MODEL", "primary"),
            ("/model", "primary"),
            ("/options/model", "primary"),
        ]
    };

    for (path, role) in role_fields {
        let Some(s) = settings.pointer(path).and_then(|v| v.as_str()) else {
            continue;
        };
        if s.is_empty() {
            continue;
        }
        // Skip if already present as same wire model.
        let already = model_candidates.iter().any(|c| {
            c.display_model == s
                || c.wire_model == s
                || (claude_family
                    && c.wire_model == crate::ccs_adapter::strip_claude_one_m_marker(s).as_ref())
        });
        if already {
            continue;
        }
        // Primary field already covered by configured model.
        if *role == "primary" {
            continue;
        }
        let cand = if claude_family {
            let wire = crate::ccs_adapter::strip_claude_one_m_marker(s).into_owned();
            ModelCandidate::role_mapping(s, &wire)
        } else {
            ModelCandidate::role_mapping(s, s)
        };
        let _ = role; // role label reserved for future UI; source is ConfiguredRoleMapping
        let _ = ModelCandidateSource::ConfiguredRoleMapping;
        model_candidates.push(cand);
    }

    let mut endpoint_candidates = raw.endpoint_urls;
    if !base_url.is_empty() && !endpoint_candidates.iter().any(|u| u == &base_url) {
        endpoint_candidates.insert(0, base_url.clone());
    }

    let custom_user_agent = meta
        .get("customUserAgent")
        .or_else(|| meta.get("custom_user_agent"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let needs_local_routing = match app_type {
        AppType::Codex => {
            // If wire_api/chat or api format is chat while codex typically wants responses
            let cfg = settings
                .get("config")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cfg.contains("wire_api") && cfg.contains("chat") {
                Some(true)
            } else {
                match configured_protocol {
                    Some(ProtocolKind::OpenAiChat) => Some(true),
                    Some(ProtocolKind::OpenAiResponses) => Some(false),
                    _ => None,
                }
            }
        }
        _ => None,
    };

    let masked_key = mask_api_key(&api_key);
    // Register the real key so path/query/URL-encoded secrets are redacted, not
    // only generic sk- heuristics (v0.1.9 Provider card URL fix).
    let mut redactor = SecretRedactor::new();
    redactor.register_key(&api_key);
    let safe_base_url = if api_key.trim().is_empty() {
        sanitize_url_for_display(&base_url)
    } else {
        sanitize_url_with_redactor(&base_url, &redactor)
    };
    let (preferred_auth, credential_source) =
        resolve_preferred_auth(app_type, &settings, auth_kind);

    NormalizedProvider {
        opaque_id: Uuid::new_v4().to_string(),
        source_id: raw.id,
        app_type,
        display_name: raw.name,
        category: raw.category,
        auth_kind,
        provider_kind,
        base_url,
        api_key: SecretString::from(api_key),
        configured_protocol,
        configured_model: model,
        model_candidates,
        endpoint_candidates,
        custom_user_agent,
        needs_local_routing,
        is_current: raw.is_current,
        skip_reason,
        masked_key,
        safe_base_url,
        website_url: raw.website_url,
        api_format_hint,
        preferred_auth,
        credential_source,
    }
}

fn resolve_preferred_auth(
    app_type: AppType,
    settings: &Value,
    auth_kind: AuthKind,
) -> (Option<crate::protocols::types::AuthScheme>, Option<String>) {
    use crate::protocols::types::AuthScheme;
    let env = settings.get("env");
    match app_type {
        AppType::Claude | AppType::ClaudeDesktop => {
            if let Some(src) = first_present_key(
                env,
                &[
                    "ANTHROPIC_AUTH_TOKEN",
                    "ANTHROPIC_API_KEY",
                    "OPENROUTER_API_KEY",
                    "GOOGLE_API_KEY",
                ],
            ) {
                let scheme = match src.as_str() {
                    "ANTHROPIC_AUTH_TOKEN" | "OPENROUTER_API_KEY" => AuthScheme::Bearer,
                    "ANTHROPIC_API_KEY" => AuthScheme::XApiKey,
                    "GOOGLE_API_KEY" => AuthScheme::XGoogApiKey,
                    _ => AuthScheme::XApiKey,
                };
                return (Some(scheme), Some(src));
            }
            (Some(AuthScheme::XApiKey), None)
        }
        AppType::Codex => (Some(AuthScheme::Bearer), Some("OPENAI_API_KEY".into())),
        AppType::Gemini => (Some(AuthScheme::XGoogApiKey), Some("GEMINI_API_KEY".into())),
        _ => {
            let scheme = match auth_kind {
                AuthKind::BearerToken => AuthScheme::Bearer,
                AuthKind::AnthropicKey => AuthScheme::XApiKey,
                AuthKind::GeminiKey => AuthScheme::XGoogApiKey,
                AuthKind::ApiKey | AuthKind::AzureApiKey => AuthScheme::Bearer,
                _ => AuthScheme::Bearer,
            };
            (Some(scheme), None)
        }
    }
}

fn first_present_key(env: Option<&Value>, keys: &[&str]) -> Option<String> {
    let env = env?;
    for k in keys {
        if let Some(s) = env.get(*k).and_then(|v| v.as_str()) {
            if !s.trim().is_empty() {
                return Some((*k).to_string());
            }
        }
    }
    None
}

fn extract_credentials(
    app_type: AppType,
    settings: &Value,
    _meta: &Value,
) -> (
    String,
    String,
    AuthKind,
    Option<ProtocolKind>,
    Option<String>,
) {
    match app_type {
        AppType::Claude | AppType::ClaudeDesktop => {
            let env = settings.get("env");
            let base = str_at(env, "ANTHROPIC_BASE_URL");
            let key = first_non_empty(
                env,
                &[
                    "ANTHROPIC_AUTH_TOKEN",
                    "ANTHROPIC_API_KEY",
                    "OPENROUTER_API_KEY",
                    "GOOGLE_API_KEY",
                ],
            );
            let model =
                first_non_empty(env, &["ANTHROPIC_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL"]);
            (
                trim_slash(&base),
                key,
                AuthKind::AnthropicKey,
                Some(ProtocolKind::AnthropicMessages),
                empty_to_none(model),
            )
        }
        AppType::Codex => {
            let auth = settings.get("auth");
            let config_text = settings.get("config").and_then(|v| v.as_str());
            let key = extract_codex_api_key(auth, config_text);
            let base = config_text
                .and_then(extract_codex_base_url)
                .unwrap_or_default();
            let model = config_text.and_then(extract_codex_model);
            let protocol = config_text.and_then(extract_codex_protocol);
            (
                trim_slash(&base),
                key,
                AuthKind::BearerToken,
                protocol.or(Some(ProtocolKind::OpenAiResponses)),
                model,
            )
        }
        AppType::Gemini => {
            let env = settings.get("env");
            let base = str_at(env, "GOOGLE_GEMINI_BASE_URL");
            let key = first_non_empty(env, &["GEMINI_API_KEY", "GOOGLE_API_KEY"]);
            let model = first_non_empty(env, &["GEMINI_MODEL", "GOOGLE_GEMINI_MODEL"]);
            (
                trim_slash(&base),
                key,
                AuthKind::GeminiKey,
                Some(ProtocolKind::GeminiNative),
                empty_to_none(model),
            )
        }
        AppType::OpenCode => {
            let options = settings.get("options");
            let base = str_at(options, "baseURL");
            let key = str_at(options, "apiKey");
            let model = str_at(options, "model");
            let protocol = infer_opencode_protocol(settings);
            (
                trim_slash(&base),
                key,
                AuthKind::ApiKey,
                Some(protocol),
                empty_to_none(model),
            )
        }
        AppType::OpenClaw => {
            let base = settings
                .get("baseUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let key = settings
                .get("apiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let model = settings
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (
                trim_slash(&base),
                key,
                AuthKind::ApiKey,
                Some(ProtocolKind::OpenAiChat),
                model.filter(|s| !s.is_empty()),
            )
        }
        AppType::Hermes => {
            let base = settings
                .get("base_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let key = settings
                .get("api_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let model = settings
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (
                trim_slash(&base),
                key,
                AuthKind::ApiKey,
                Some(ProtocolKind::OpenAiChat),
                model.filter(|s| !s.is_empty()),
            )
        }
        AppType::GrokBuild => {
            // Grok build stores config text; best-effort JSON/env-like extraction
            let cfg = settings
                .get("config")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let base = extract_simple_kv(cfg, "base_url")
                .or_else(|| extract_simple_kv(cfg, "baseUrl"))
                .unwrap_or_default();
            let key = extract_simple_kv(cfg, "api_key")
                .or_else(|| extract_simple_kv(cfg, "apiKey"))
                .unwrap_or_default();
            let model = extract_simple_kv(cfg, "model");
            (
                trim_slash(&base),
                key,
                AuthKind::ApiKey,
                Some(ProtocolKind::OpenAiChat),
                model,
            )
        }
        AppType::Unknown => (String::new(), String::new(), AuthKind::Unknown, None, None),
    }
}

fn resolve_protocol(
    app_type: AppType,
    api_format: Option<&str>,
    default: Option<ProtocolKind>,
    settings: &Value,
) -> Option<ProtocolKind> {
    if let Some(fmt) = api_format {
        match fmt.to_ascii_lowercase().as_str() {
            "anthropic" => return Some(ProtocolKind::AnthropicMessages),
            "openai_chat" | "openai-chat" | "chat" | "chat_completions" => {
                return Some(ProtocolKind::OpenAiChat)
            }
            "openai_responses" | "openai-responses" | "responses" => {
                return Some(ProtocolKind::OpenAiResponses)
            }
            "gemini" | "gemini_native" => return Some(ProtocolKind::GeminiNative),
            _ => {}
        }
    }
    if app_type == AppType::OpenCode {
        return Some(infer_opencode_protocol(settings));
    }
    default
}

fn infer_opencode_protocol(settings: &Value) -> ProtocolKind {
    let npm = settings
        .get("npm")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if npm.contains("anthropic") {
        ProtocolKind::AnthropicMessages
    } else if npm.contains("google") || npm.contains("gemini") {
        ProtocolKind::GeminiNative
    } else {
        ProtocolKind::OpenAiChat
    }
}

fn extract_codex_api_key(auth: Option<&Value>, config_text: Option<&str>) -> String {
    if let Some(auth) = auth {
        if let Some(k) = auth
            .get("OPENAI_API_KEY")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return k.to_string();
        }
    }
    if let Some(cfg) = config_text {
        if let Some(k) = extract_simple_kv(cfg, "experimental_bearer_token") {
            return k;
        }
    }
    String::new()
}

/// Prefer active model_providers.<model_provider>.base_url, else top-level base_url.
pub fn extract_codex_base_url(config_text: &str) -> Option<String> {
    let doc = config_text.parse::<toml::Value>().ok()?;
    if let Some(active) = doc.get("model_provider").and_then(|v| v.as_str()) {
        if let Some(base) = doc
            .get("model_providers")
            .and_then(|p| p.get(active))
            .and_then(|p| p.get("base_url"))
            .and_then(|v| v.as_str())
        {
            return Some(base.to_string());
        }
    }
    doc.get("base_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_codex_model(config_text: &str) -> Option<String> {
    let doc = config_text.parse::<toml::Value>().ok()?;
    doc.get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_codex_protocol(config_text: &str) -> Option<ProtocolKind> {
    let doc = config_text.parse::<toml::Value>().ok()?;
    let active = doc.get("model_provider").and_then(|v| v.as_str())?;
    let wire = doc
        .get("model_providers")
        .and_then(|p| p.get(active))
        .and_then(|p| p.get("wire_api"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match wire {
        "chat" | "chat_completions" => Some(ProtocolKind::OpenAiChat),
        "responses" => Some(ProtocolKind::OpenAiResponses),
        _ => None,
    }
}

fn extract_simple_kv(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let v = rest.trim().trim_matches('"').trim_matches('\'').trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn str_at(obj: Option<&Value>, key: &str) -> String {
    obj.and_then(|e| e.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn first_non_empty(obj: Option<&Value>, keys: &[&str]) -> String {
    let Some(obj) = obj else {
        return String::new();
    };
    for k in keys {
        if let Some(s) = obj.get(*k).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    String::new()
}

fn trim_slash(s: &str) -> String {
    s.trim().trim_end_matches('/').to_string()
}

fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_claude_credentials() {
        let raw = RawProviderRow {
            id: "1".into(),
            app_type: "claude".into(),
            name: "GLM".into(),
            settings_config: r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.example.com/v1","ANTHROPIC_AUTH_TOKEN":"sk-abcdefghabcdefgh","ANTHROPIC_MODEL":"glm-4"}}"#.into(),
            website_url: None,
            category: Some("custom".into()),
            meta: r#"{"apiFormat":"anthropic"}"#.into(),
            is_current: true,
            endpoint_urls: vec![],
        };
        let n = normalize_provider(raw);
        assert!(n.is_selectable());
        assert_eq!(n.configured_protocol, Some(ProtocolKind::AnthropicMessages));
        assert!(n.masked_key.contains('…') || n.masked_key.contains("..."));
        assert!(!n.masked_key.contains("sk-abcdefgh"));
    }

    #[test]
    fn extract_codex_from_toml() {
        let cfg = r#"
model_provider = "minimax"
model = "MiniMax-M2"
[model_providers.minimax]
name = "MiniMax"
base_url = "https://api.minimax.test/v1"
wire_api = "chat"
"#;
        assert_eq!(
            extract_codex_base_url(cfg).as_deref(),
            Some("https://api.minimax.test/v1")
        );
        assert_eq!(extract_codex_protocol(cfg), Some(ProtocolKind::OpenAiChat));
    }

    #[test]
    fn managed_oauth_not_selectable() {
        let raw = RawProviderRow {
            id: "oauth".into(),
            app_type: "codex".into(),
            name: "Official".into(),
            settings_config: r#"{"auth":{},"config":""}"#.into(),
            website_url: None,
            category: Some("official".into()),
            meta: r#"{"providerType":"codex_oauth"}"#.into(),
            is_current: false,
            endpoint_urls: vec![],
        };
        let n = normalize_provider(raw);
        assert!(!n.is_selectable());
        assert!(n.skip_reason.unwrap().contains("OAuth"));
    }

    #[test]
    fn claude_one_m_becomes_local_marker_candidate() {
        let raw = RawProviderRow {
            id: "1".into(),
            app_type: "claude".into(),
            name: "GLM".into(),
            settings_config: r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.example.com/v1","ANTHROPIC_AUTH_TOKEN":"sk-abcdefghabcdefgh","ANTHROPIC_MODEL":"GLM-5.2[1M]","ANTHROPIC_DEFAULT_SONNET_MODEL":"sonnet-mapped"}}"#.into(),
            website_url: None,
            category: Some("custom".into()),
            meta: r#"{"apiFormat":"anthropic"}"#.into(),
            is_current: true,
            endpoint_urls: vec![],
        };
        let n = normalize_provider(raw);
        assert_eq!(n.configured_model.as_deref(), Some("GLM-5.2[1M]"));
        let current = n
            .model_candidates
            .iter()
            .find(|c| c.equivalent_to_current)
            .expect("current candidate");
        assert_eq!(current.wire_model, "GLM-5.2");
        assert_eq!(
            current.source,
            super::ModelCandidateSource::LocalMarkerNormalized
        );
        assert!(n.model_candidates.iter().any(|c| c.source
            == super::ModelCandidateSource::ConfiguredRoleMapping
            && c.wire_model == "sonnet-mapped"));
    }

    #[test]
    fn safe_base_url_redacts_key_in_path() {
        let key = "sk-secret-path-ABCDEFGH";
        let raw = RawProviderRow {
            id: "1".into(),
            app_type: "claude".into(),
            name: "Relay".into(),
            settings_config: format!(
                r#"{{"env":{{"ANTHROPIC_BASE_URL":"https://example.com/{key}/v1","ANTHROPIC_AUTH_TOKEN":"{key}","ANTHROPIC_MODEL":"m"}}}}"#
            ),
            website_url: None,
            category: Some("custom".into()),
            meta: r#"{"apiFormat":"anthropic"}"#.into(),
            is_current: true,
            endpoint_urls: vec![],
        };
        let n = normalize_provider(raw);
        assert!(
            !n.safe_base_url.contains(key),
            "full key leaked in safe_base_url: {}",
            n.safe_base_url
        );
    }
}
