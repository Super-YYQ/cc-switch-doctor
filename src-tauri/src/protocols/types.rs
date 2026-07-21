use crate::ccs_adapter::ProtocolKind;
use crate::security::redact::SecretRedactor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub const PROMPT_ZH: &str = "只输出字符串 CCS_DOCTOR_OK，不要输出其他内容。";
pub const PROMPT_EN: &str = "Reply with exactly CCS_DOCTOR_OK and nothing else.";
pub const SUCCESS_MARKER: &str = "CCS_DOCTOR_OK";
pub const MAX_TOKENS: u32 = 16;
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ERROR_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    Bearer,
    XApiKey,
    XGoogApiKey,
    QueryKey,
}

#[derive(Debug, Clone)]
pub struct BuiltRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Value>,
    pub stream: bool,
    pub protocol: ProtocolKind,
    pub model: String,
    pub purpose: RequestPurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RequestPurpose {
    Generate,
    StreamGenerate,
    ToolCall,
    ListModels,
}

/// Which OpenAI Chat token-limit field to put on the request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokenLimitField {
    #[default]
    MaxCompletionTokens,
    MaxTokens,
}

impl TokenLimitField {
    pub fn as_json_key(self) -> &'static str {
        match self {
            Self::MaxCompletionTokens => "max_completion_tokens",
            Self::MaxTokens => "max_tokens",
        }
    }

    pub fn label(self) -> &'static str {
        self.as_json_key()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptResult {
    pub ok: bool,
    pub partial: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
    pub ttft_ms: Option<u64>,
    pub protocol: ProtocolKind,
    pub model: String,
    pub url: String,
    pub stream: bool,
    pub purpose: RequestPurpose,
    pub extracted_text: Option<String>,
    pub tool_call_ok: Option<bool>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub response_excerpt: Option<String>,
    pub classification: String,
    /// True only when `reqwest` actually started sending the request.
    #[serde(default)]
    pub http_sent: bool,
    /// True when this result was served from the in-run memory cache.
    #[serde(default)]
    pub reused_from_cache: bool,
    /// Optional UI note (e.g. cache reuse / token field fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion_note: Option<String>,
    /// Token limit field used for OpenAI Chat (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_limit_field: Option<TokenLimitField>,
}

impl AttemptResult {
    pub fn network_error(
        protocol: ProtocolKind,
        model: &str,
        url: &str,
        msg: &str,
        latency_ms: u64,
    ) -> Self {
        Self {
            ok: false,
            partial: false,
            status_code: None,
            latency_ms,
            ttft_ms: None,
            protocol,
            model: model.to_string(),
            url: url.to_string(),
            stream: false,
            purpose: RequestPurpose::Generate,
            extracted_text: None,
            tool_call_ok: None,
            error_kind: Some("network".into()),
            error_message: Some(msg.to_string()),
            response_excerpt: None,
            classification: "NETWORK_UNREACHABLE".into(),
            http_sent: false,
            reused_from_cache: false,
            suggestion_note: None,
            token_limit_field: None,
        }
    }

    pub fn budget_stopped(
        protocol: ProtocolKind,
        model: &str,
        url: &str,
        reason: &str,
        classification: &str,
    ) -> Self {
        Self {
            ok: false,
            partial: false,
            status_code: None,
            latency_ms: 0,
            ttft_ms: None,
            protocol,
            model: model.to_string(),
            url: url.to_string(),
            stream: false,
            purpose: RequestPurpose::Generate,
            extracted_text: None,
            tool_call_ok: None,
            error_kind: Some("budget".into()),
            error_message: Some(reason.to_string()),
            response_excerpt: None,
            classification: classification.into(),
            http_sent: false,
            reused_from_cache: false,
            suggestion_note: None,
            token_limit_field: None,
        }
    }
}

/// True only when the error body clearly rejects `max_completion_tokens` as a field.
/// Never true for auth, quota, rate-limit, network, model-missing, or bare 404.
pub fn is_max_completion_tokens_unsupported(result: &AttemptResult) -> bool {
    // Never fall back on these hard failures
    match result.classification.as_str() {
        "KEY_INVALID"
        | "PERMISSION_DENIED"
        | "QUOTA_EXHAUSTED"
        | "RATE_LIMITED"
        | "NETWORK_UNREACHABLE"
        | "TLS_ERROR"
        | "TIMEOUT"
        | "MODEL_NOT_FOUND"
        | "ENDPOINT_NOT_FOUND"
        | "CANCELLED"
        | "CROSS_ORIGIN_REDIRECT_BLOCKED"
        | "HOST_BUDGET_EXHAUSTED"
        | "HOST_RATE_LIMIT_STOPPED" => return false,
        _ => {}
    }

    let body = format!(
        "{} {}",
        result.error_message.as_deref().unwrap_or(""),
        result.response_excerpt.as_deref().unwrap_or("")
    )
    .to_ascii_lowercase();

    if !body.contains("max_completion_tokens") {
        return false;
    }

    body.contains("unknown parameter")
        || body.contains("unsupported parameter")
        || body.contains("unrecognized")
        || body.contains("invalid parameter")
        || body.contains("invalid field")
        || body.contains("not supported")
        || body.contains("unexpected argument")
        || (body.contains("unknown") && body.contains("parameter"))
}

pub fn evaluate_text(text: &str) -> (bool, bool) {
    let t = text.trim();
    if t.is_empty() {
        return (false, false);
    }
    if t.contains(SUCCESS_MARKER) {
        return (true, false);
    }
    // Partial: got non-empty model text without marker
    (false, true)
}

pub fn apply_auth(headers: &mut HashMap<String, String>, scheme: AuthScheme, key: &str) {
    match scheme {
        AuthScheme::Bearer => {
            headers.insert("Authorization".into(), format!("Bearer {key}"));
        }
        AuthScheme::XApiKey => {
            headers.insert("x-api-key".into(), key.to_string());
            headers
                .entry("anthropic-version".into())
                .or_insert_with(|| "2023-06-01".into());
        }
        AuthScheme::XGoogApiKey => {
            headers.insert("x-goog-api-key".into(), key.to_string());
        }
        AuthScheme::QueryKey => {
            // handled at URL level
        }
    }
}

pub fn default_timeout(deep: bool) -> Duration {
    if deep {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(15)
    }
}

pub fn redact_result(mut r: AttemptResult, redactor: &SecretRedactor) -> AttemptResult {
    r.url = crate::security::sanitize_url_for_display(&r.url);
    if let Some(m) = r.error_message.as_mut() {
        *m = redactor.redact(m);
    }
    if let Some(ex) = r.response_excerpt.as_mut() {
        *ex = redactor.redact(ex);
        if ex.len() > 512 {
            ex.truncate(512);
            ex.push('…');
        }
    }
    r
}
