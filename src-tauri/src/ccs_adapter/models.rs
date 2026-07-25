use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppType {
    Claude,
    #[serde(rename = "claude-desktop", alias = "claude_desktop")]
    ClaudeDesktop,
    Codex,
    Gemini,
    GrokBuild,
    OpenCode,
    OpenClaw,
    Hermes,
    Unknown,
}

impl AppType {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "claude" => Self::Claude,
            "claude-desktop" | "claude_desktop" => Self::ClaudeDesktop,
            "codex" => Self::Codex,
            "gemini" => Self::Gemini,
            "grokbuild" | "grok-build" | "grok_build" => Self::GrokBuild,
            "opencode" | "open-code" => Self::OpenCode,
            "openclaw" | "open-claw" => Self::OpenClaw,
            "hermes" => Self::Hermes,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::GrokBuild => "grokbuild",
            Self::OpenCode => "opencode",
            Self::OpenClaw => "openclaw",
            Self::Hermes => "hermes",
            Self::Unknown => "unknown",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini CLI",
            Self::GrokBuild => "Grok Build",
            Self::OpenCode => "OpenCode",
            Self::OpenClaw => "OpenClaw",
            Self::Hermes => "Hermes",
            Self::Unknown => "未知",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    ApiKey,
    BearerToken,
    AnthropicKey,
    GeminiKey,
    AzureApiKey,
    ManagedOAuth,
    GitHubCopilot,
    CodexOAuth,
    OfficialSubscription,
    Unknown,
}

impl AuthKind {
    pub fn is_testable(self) -> bool {
        matches!(
            self,
            Self::ApiKey
                | Self::BearerToken
                | Self::AnthropicKey
                | Self::GeminiKey
                | Self::AzureApiKey
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolKind {
    OpenAiChat,
    OpenAiResponses,
    AnthropicMessages,
    GeminiNative,
    Unknown,
}

impl ProtocolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiChat => "openai_chat",
            Self::OpenAiResponses => "openai_responses",
            Self::AnthropicMessages => "anthropic_messages",
            Self::GeminiNative => "gemini_native",
            Self::Unknown => "unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAiChat => "OpenAI Chat Completions",
            Self::OpenAiResponses => "OpenAI Responses",
            Self::AnthropicMessages => "Anthropic Messages",
            Self::GeminiNative => "Gemini Native",
            Self::Unknown => "未知协议",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    ThirdPartyApi,
    ManagedAccount,
    OfficialSubscription,
    Unknown,
}

/// In-memory provider with secret key. Never serialize the raw key to the frontend.
pub struct NormalizedProvider {
    pub opaque_id: String,
    pub source_id: String,
    pub app_type: AppType,
    pub display_name: String,
    pub category: Option<String>,
    pub auth_kind: AuthKind,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub api_key: secrecy::SecretString,
    pub configured_protocol: Option<ProtocolKind>,
    pub configured_model: Option<String>,
    pub model_candidates: Vec<String>,
    pub endpoint_candidates: Vec<String>,
    pub custom_user_agent: Option<String>,
    pub needs_local_routing: Option<bool>,
    pub is_current: bool,
    pub skip_reason: Option<String>,
    pub masked_key: String,
    pub safe_base_url: String,
    pub website_url: Option<String>,
    pub api_format_hint: Option<String>,
    /// Preferred HTTP auth scheme for the current config test (never a secret).
    pub preferred_auth: Option<crate::protocols::types::AuthScheme>,
    /// Credential source field name for UI (e.g. ANTHROPIC_AUTH_TOKEN).
    pub credential_source: Option<String>,
}

impl NormalizedProvider {
    pub fn is_selectable(&self) -> bool {
        self.skip_reason.is_none()
            && self.auth_kind.is_testable()
            && !self.api_key.expose().is_empty()
            && !self.base_url.is_empty()
    }
}

trait SecretExpose {
    fn expose(&self) -> &str;
}

impl SecretExpose for secrecy::SecretString {
    fn expose(&self) -> &str {
        use secrecy::ExposeSecret;
        self.expose_secret()
    }
}

/// Frontend-safe provider row (no full key / no settings_config).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderListItem {
    pub opaque_id: String,
    pub source_id: String,
    pub app_type: AppType,
    pub app_label: String,
    pub display_name: String,
    pub category: Option<String>,
    pub auth_kind: AuthKind,
    pub provider_kind: ProviderKind,
    pub safe_base_url: String,
    pub masked_key: String,
    pub configured_protocol: Option<ProtocolKind>,
    pub protocol_label: Option<String>,
    pub configured_model: Option<String>,
    pub is_current: bool,
    pub selectable: bool,
    pub skip_reason: Option<String>,
    pub needs_local_routing: Option<bool>,
    pub website_url: Option<String>,
    pub credential_source: Option<String>,
    pub preferred_auth_label: Option<String>,
}

impl From<&NormalizedProvider> for ProviderListItem {
    fn from(p: &NormalizedProvider) -> Self {
        Self {
            opaque_id: p.opaque_id.clone(),
            source_id: p.source_id.clone(),
            app_type: p.app_type,
            app_label: p.app_type.label_zh().to_string(),
            display_name: p.display_name.clone(),
            category: p.category.clone(),
            auth_kind: p.auth_kind,
            provider_kind: p.provider_kind,
            safe_base_url: p.safe_base_url.clone(),
            masked_key: p.masked_key.clone(),
            configured_protocol: p.configured_protocol,
            protocol_label: p.configured_protocol.map(|x| x.label().to_string()),
            configured_model: p.configured_model.clone(),
            is_current: p.is_current,
            selectable: p.is_selectable(),
            skip_reason: p.skip_reason.clone(),
            needs_local_routing: p.needs_local_routing,
            website_url: p.website_url.clone(),
            credential_source: p.credential_source.clone(),
            preferred_auth_label: p.preferred_auth.map(|a| {
                match a {
                    crate::protocols::types::AuthScheme::Bearer => "Bearer",
                    crate::protocols::types::AuthScheme::XApiKey => "x-api-key",
                    crate::protocols::types::AuthScheme::XGoogApiKey => "x-goog-api-key",
                    crate::protocols::types::AuthScheme::QueryKey => "query-key",
                }
                .to_string()
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryInfo {
    pub found: bool,
    pub database_path: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfoView {
    pub fingerprint_id: String,
    pub user_version: i32,
    /// Legacy status string (verified/compatible/unknown/unsupported).
    pub status: String,
    pub tables: Vec<String>,
    pub providers_columns: Vec<String>,
    pub message: String,
    /// Independent version verification (verified / known_compatible /
    /// unverified_structure_compatible / unknown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_verification: Option<String>,
    /// Capability-level report (provider/endpoint/direct/routing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<crate::ccs_adapter::fingerprint::SchemaCapabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderScanView {
    pub discovery: DiscoveryInfo,
    pub schema: Option<SchemaInfoView>,
    pub providers: Vec<ProviderListItem>,
    /// Capability-based: true when provider_scan + direct_diagnosis are usable.
    pub can_test: bool,
    pub scanned_at: String,
    pub cc_switch_version_hint: Option<String>,
    /// Read-only CCS local route status (may be unavailable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<crate::ccs_adapter::routing::RoutingStatusView>,
}
