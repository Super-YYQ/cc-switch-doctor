use crate::ccs_adapter::ProtocolKind;
use crate::security::redact::{truncate_utf8, SecretRedactor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Fixed, product-neutral generate prompt. Not randomized; not provider-specific.
pub const BASIC_GENERATE_PROMPT: &str = "Reply briefly.";
pub const MAX_TOKENS: u32 = 16;
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ERROR_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// How the response text was recovered from the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResponseCompatibility {
    #[default]
    Native,
    CrossProtocol,
    LooseField,
}

/// Which diagnostic channel produced an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisChannel {
    #[default]
    DirectUpstream,
    CcsLocalRoute,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvidence {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_keyword: Option<String>,
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
    #[serde(default)]
    pub error_evidence: Vec<ErrorEvidence>,
    /// Direct upstream vs CCS local route.
    #[serde(default)]
    pub channel: DiagnosisChannel,
    /// How the body text was extracted when ok/partial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_compatibility: Option<ResponseCompatibility>,
    /// Protocol we requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_protocol: Option<ProtocolKind>,
    /// Protocol shape that actually matched (may differ).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_protocol: Option<ProtocolKind>,
    /// Configured / display model (may keep local `[1M]` marker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_model_display: Option<String>,
    /// Model value actually sent upstream (wire model).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbound_model: Option<String>,
    /// Human-readable transform note (e.g. local [1M] strip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_transform: Option<String>,
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
            error_evidence: vec![],
            channel: DiagnosisChannel::DirectUpstream,
            response_compatibility: None,
            requested_protocol: Some(protocol),
            matched_protocol: None,
            configured_model_display: None,
            outbound_model: None,
            model_transform: None,
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
            error_evidence: vec![],
            channel: DiagnosisChannel::DirectUpstream,
            response_compatibility: None,
            requested_protocol: Some(protocol),
            matched_protocol: None,
            configured_model_display: None,
            outbound_model: None,
            model_transform: None,
        }
    }

    /// Native success on the direct channel can count as current-config success.
    pub fn is_native_success(&self) -> bool {
        self.ok
            && matches!(
                self.response_compatibility,
                Some(ResponseCompatibility::Native) | None
            )
            && !matches!(
                self.classification.as_str(),
                "RESPONSE_PROTOCOL_VARIANT_OK"
                    | "DIRECT_PROTOCOL_VARIANT_OK"
                    | "DIRECT_LOOSE_TEXT_OK"
                    | "LOOSE_RESPONSE_TEXT_OK"
                    | "STREAM_PROTOCOL_VARIANT_OK"
                    | "PARTIAL_TEXT"
            )
            && self.channel == DiagnosisChannel::DirectUpstream
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

/// True when native / cross-protocol extractors recovered consumable model text.
pub fn evaluate_native_text(text: &str) -> bool {
    !text.trim().is_empty()
}

/// Generate / stream success for non-empty recovered text.
/// Returns `(ok, partial)` where partial is reserved for loose-field paths only
/// (callers must not map native non-empty text to partial).
pub fn evaluate_text(text: &str) -> (bool, bool) {
    if evaluate_native_text(text) {
        (true, false)
    } else {
        (false, false)
    }
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
    r.url = crate::security::redact::sanitize_url_with_redactor(&r.url, redactor);
    if let Some(m) = r.error_message.as_mut() {
        *m = redactor.redact(m);
    }
    if let Some(ex) = r.response_excerpt.as_mut() {
        *ex = truncate_utf8(&redactor.redact(ex), 512);
    }
    for e in &mut r.error_evidence {
        if let Some(m) = e.message.as_mut() {
            *m = redactor.redact(m);
        }
    }
    r
}

#[cfg(test)]
mod redact_tests {
    use super::*;
    use crate::ccs_adapter::ProtocolKind;

    #[test]
    fn redact_result_scrubs_error_evidence_messages() {
        let key = "sk-secret-evidence-ABCDEFGH";
        let mut redactor = SecretRedactor::new();
        redactor.register_key(key);
        let r = AttemptResult {
            ok: false,
            partial: false,
            status_code: Some(200),
            latency_ms: 1,
            ttft_ms: None,
            protocol: ProtocolKind::AnthropicMessages,
            model: "m".into(),
            url: "https://api.example.com/v1".into(),
            stream: false,
            purpose: RequestPurpose::Generate,
            extracted_text: None,
            tool_call_ok: None,
            error_kind: Some("structured_error".into()),
            error_message: Some(format!("auth failed for {key}")),
            response_excerpt: Some(format!("body has {key}")),
            classification: "AUTH_INVALID".into(),
            http_sent: true,
            reused_from_cache: false,
            suggestion_note: None,
            token_limit_field: None,
            error_evidence: vec![ErrorEvidence {
                source: "error_envelope".into(),
                code: Some("invalid_api_key".into()),
                message: Some(format!("invalid token {key}")),
                matched_keyword: None,
            }],
            channel: DiagnosisChannel::DirectUpstream,
            response_compatibility: None,
            requested_protocol: None,
            matched_protocol: None,
            configured_model_display: None,
            outbound_model: None,
            model_transform: None,
        };
        let out = redact_result(r, &redactor);
        assert!(
            !out.error_message.as_deref().unwrap_or("").contains(key),
            "error_message leaked key"
        );
        assert!(
            !out.response_excerpt.as_deref().unwrap_or("").contains(key),
            "response_excerpt leaked key"
        );
        let ev_msg = out.error_evidence[0].message.as_deref().unwrap_or("");
        assert!(!ev_msg.contains(key), "error_evidence message leaked key: {ev_msg}");
    }
}
