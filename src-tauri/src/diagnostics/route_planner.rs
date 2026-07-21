//! Build CCS local-route client-protocol attempts.
//!
//! Route channel sends the *client* protocol to the already-running CCS loopback
//! proxy using the CCS placeholder credential — never the provider real key.

use crate::ccs_adapter::routing::{
    route_base_url, AppRoutingStatusView, RoutingStatusView, CCS_PROXY_PLACEHOLDER_TOKEN,
};
use crate::ccs_adapter::{AppType, NormalizedProvider, ProtocolKind};
use crate::diagnostics::planner::DiagnosisMode;
use crate::protocols::anthropic::build_anthropic_request;
use crate::protocols::gemini::build_gemini_request_with_auth;
use crate::protocols::openai_chat::build_chat_request;
use crate::protocols::openai_responses::build_responses_request;
use crate::protocols::types::{AuthScheme, BuiltRequest, DiagnosisChannel, TokenLimitField};

/// Max real CCS route HTTP sends per app per diagnosis session.
pub const ROUTE_SEND_BUDGET_PER_APP: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerifyMode {
    #[default]
    Auto,
    DirectOnly,
    DirectAndRoute,
}

impl VerifyMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "direct" | "direct_only" | "direct-only" => Self::DirectOnly,
            "direct_and_route" | "direct+route" | "both" => Self::DirectAndRoute,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteAttemptPlan {
    pub app_type: AppType,
    pub base_url: String,
    pub protocol: ProtocolKind,
    pub model: String,
    pub stream: bool,
    pub label: String,
    pub expected_provider_id: Option<String>,
    pub expected_provider_name: Option<String>,
    pub auto_failover: bool,
}

/// Whether route verification applies for this provider under the given mode/status.
pub fn route_applicable(
    provider: &NormalizedProvider,
    routing: &RoutingStatusView,
    mode: VerifyMode,
) -> RouteApplicability {
    if mode == VerifyMode::DirectOnly {
        return RouteApplicability::Skip("验证方式：仅直连".into());
    }
    if !routing.config_detected {
        return RouteApplicability::Skip("路由状态不可用".into());
    }
    if routing.connect_host.is_none() || routing.listen_port.is_none() {
        return RouteApplicability::Skip("监听地址非 loopback 或端口未知".into());
    }
    if !routing.server_running && !routing.health_reachable {
        if mode == VerifyMode::DirectAndRoute {
            return RouteApplicability::NotRunning;
        }
        // Auto: if route not running, skip silently to direct-only
        return RouteApplicability::Skip("CCS 路由未运行".into());
    }

    let app_row = routing
        .apps
        .iter()
        .find(|a| a.app_type == provider.app_type.as_str());
    let Some(app) = app_row else {
        return RouteApplicability::Skip(format!(
            "应用 {} 无 CCS 路由配置",
            provider.app_type.label_zh()
        ));
    };
    if !app.enabled && !routing.global_enabled {
        return RouteApplicability::Skip("应用路由未开启".into());
    }
    if !app.enabled {
        // global may be on but this app not taken over
        return RouteApplicability::Skip(format!("{} 路由未接管", provider.app_type.label_zh()));
    }

    // Only current route target may own route results
    if !provider.is_current {
        return RouteApplicability::NotCurrentTarget;
    }
    if let Some(active_id) = &app.active_provider_id {
        if active_id != &provider.source_id {
            return RouteApplicability::TargetMismatch {
                expected: provider.source_id.clone(),
                actual: active_id.clone(),
                actual_name: app.active_provider_name.clone(),
            };
        }
    }

    if mode == VerifyMode::Auto && !app.enabled {
        return RouteApplicability::Skip("自动模式：应用路由关闭".into());
    }

    RouteApplicability::Yes(app.clone())
}

#[derive(Debug, Clone)]
pub enum RouteApplicability {
    Yes(AppRoutingStatusView),
    Skip(String),
    NotRunning,
    NotCurrentTarget,
    TargetMismatch {
        expected: String,
        actual: String,
        actual_name: Option<String>,
    },
}

/// Client protocol for route channel (NOT upstream provider protocol).
pub fn client_protocol_for_app(app: AppType) -> ProtocolKind {
    match app {
        AppType::Claude | AppType::ClaudeDesktop => ProtocolKind::AnthropicMessages,
        AppType::Codex => ProtocolKind::OpenAiResponses,
        AppType::Gemini => ProtocolKind::GeminiNative,
        // Only stable entries; unknown apps not route-tested
        _ => ProtocolKind::Unknown,
    }
}

/// Client-visible model for route tests (profile-bound role aliases).
///
/// Source: `compatibility/manifest.json` → `routingProfiles` (see
/// `docs/research/v0.1.7-source-review.md`). Never scatter dated Anthropic IDs.
pub fn client_model_for_app(app: AppType, provider: &NormalizedProvider) -> String {
    if let Some(m) = crate::ccs_adapter::routing_profile::client_route_model(app) {
        return m;
    }
    // Profile missing: prefer provider-configured model, then refuse inventing dated IDs.
    provider
        .configured_model
        .clone()
        .or_else(|| provider.model_candidates.first().cloned())
        .unwrap_or_else(|| crate::ccs_adapter::routing_profile::default_direct_model_guess(app))
}

pub fn plan_route_attempts(
    provider: &NormalizedProvider,
    routing: &RoutingStatusView,
    mode: DiagnosisMode,
    app_row: &AppRoutingStatusView,
) -> Vec<RouteAttemptPlan> {
    // Unknown / missing profile: do not invent route business requests.
    if !crate::ccs_adapter::routing_profile::route_profile_verified() {
        return vec![];
    }
    let host = match routing.connect_host.as_deref() {
        Some(h) => h,
        None => return vec![],
    };
    let port = match routing.listen_port {
        Some(p) => p,
        None => return vec![],
    };
    let base = route_base_url(host, port);
    let protocol = client_protocol_for_app(provider.app_type);
    if protocol == ProtocolKind::Unknown {
        return vec![];
    }
    let model = client_model_for_app(provider.app_type, provider);

    let mut plans = vec![RouteAttemptPlan {
        app_type: provider.app_type,
        base_url: base.clone(),
        protocol,
        model: model.clone(),
        stream: false,
        label: format!("CCS 路由链 · {}", provider.app_type.label_zh()),
        expected_provider_id: Some(provider.source_id.clone()),
        expected_provider_name: Some(provider.display_name.clone()),
        auto_failover: app_row.auto_failover_enabled,
    }];

    // Deep: one streaming route probe (budget 2 total)
    if mode == DiagnosisMode::Deep {
        plans.push(RouteAttemptPlan {
            app_type: provider.app_type,
            base_url: base,
            protocol,
            model,
            stream: true,
            label: format!("CCS 路由链 Streaming · {}", provider.app_type.label_zh()),
            expected_provider_id: Some(provider.source_id.clone()),
            expected_provider_name: Some(provider.display_name.clone()),
            auto_failover: app_row.auto_failover_enabled,
        });
    }

    plans.truncate(ROUTE_SEND_BUDGET_PER_APP);
    plans
}

/// Build a route request using placeholder token only.
pub fn build_route_request(plan: &RouteAttemptPlan) -> Option<BuiltRequest> {
    let key = CCS_PROXY_PLACEHOLDER_TOKEN;
    let req = match plan.protocol {
        ProtocolKind::AnthropicMessages => {
            // Claude client uses x-api-key style against local proxy
            build_anthropic_request(
                &plan.base_url,
                &plan.model,
                key,
                plan.stream,
                false,
                false, // x-api-key not bearer for anthropic client path
                None,
            )
        }
        ProtocolKind::OpenAiChat => build_chat_request(
            &plan.base_url,
            &plan.model,
            key,
            plan.stream,
            false,
            None,
            TokenLimitField::MaxCompletionTokens,
        ),
        ProtocolKind::OpenAiResponses => {
            build_responses_request(&plan.base_url, &plan.model, key, plan.stream, false, None)
        }
        ProtocolKind::GeminiNative => build_gemini_request_with_auth(
            &plan.base_url,
            &plan.model,
            key,
            plan.stream,
            false,
            None,
            AuthScheme::XGoogApiKey,
        ),
        ProtocolKind::Unknown => return None,
    };
    // Ensure Authorization / key never carries a real provider secret — already placeholder.
    // Mark purpose already set by builders.
    let _ = DiagnosisChannel::CcsLocalRoute;
    Some(req)
}

/// Combine **attempted** route + direct outcomes into a provider-level primary status.
///
/// Call only when at least one CCS route business request was actually sent
/// (`channel == CcsLocalRoute && http_sent`). Route dispositions that never sent a
/// request (NotRunning / NotCurrentTarget / DirectOnly / Skip / etc.) must NOT be
/// passed here — the engine keeps `primary = direct_status` in those cases.
///
/// `route_target_mismatch` here means mismatch observed **after** a real route send.
pub fn combine_attempted_route_and_direct(
    route_ok: Option<bool>,
    direct_native_ok: bool,
    direct_variant_ok: bool,
    direct_failed: bool,
    route_target_mismatch: bool,
) -> String {
    if route_target_mismatch {
        return "CCS_ROUTE_TARGET_MISMATCH".into();
    }

    match route_ok {
        Some(true) => {
            if direct_native_ok {
                "CCS_ROUTE_OK_DIRECT_NATIVE_OK".into()
            } else if direct_variant_ok {
                "CCS_ROUTE_OK_DIRECT_VARIANT".into()
            } else if direct_failed {
                "CCS_ROUTE_OK_DIRECT_PARSE_FAILED".into()
            } else {
                "CCS_ROUTE_OK".into()
            }
        }
        Some(false) => {
            if direct_native_ok || direct_variant_ok {
                "CCS_ROUTE_FAILED_DIRECT_OK".into()
            } else {
                "CCS_ROUTE_AND_DIRECT_FAILED".into()
            }
        }
        // Leader path should always set route_ok after a real send; fall back conservatively.
        None => {
            if direct_native_ok {
                "CURRENT_CONFIG_OK".into()
            } else if direct_variant_ok {
                "DIRECT_PROTOCOL_VARIANT_OK".into()
            } else if direct_failed {
                // Prefer not inventing a route status when route_ok was never recorded.
                "UNKNOWN_ERROR".into()
            } else {
                "UNKNOWN_ERROR".into()
            }
        }
    }
}

/// Backward-compatible wrapper used by older tests and transitional call sites.
///
/// When the route was **not** actually attempted, primary status is always
/// `direct_status` (or a native/variant success shorthand). Route disposition
/// codes (`CCS_ROUTE_NOT_*`) never become primary.
#[allow(clippy::too_many_arguments)]
pub fn combine_route_direct_status(
    route_ok: Option<bool>,
    _route_classification: Option<&str>,
    direct_native_ok: bool,
    direct_variant_ok: bool,
    direct_failed: bool,
    route_not_running: bool,
    route_not_applicable: bool,
    route_target_mismatch: bool,
    direct_status: &str,
) -> String {
    // No real route business request → primary is pure direct outcome.
    // Disposition flags are auxiliary only (surfaced via route_status).
    if route_not_running || route_not_applicable {
        return direct_status.to_string();
    }

    // Target mismatch without a route_ok means pre-send applicability only.
    if route_target_mismatch && route_ok.is_none() {
        return direct_status.to_string();
    }

    if route_ok.is_none() && !route_target_mismatch {
        return direct_status.to_string();
    }

    combine_attempted_route_and_direct(
        route_ok,
        direct_native_ok,
        direct_variant_ok,
        direct_failed,
        route_target_mismatch,
    )
}

pub fn route_side_effect_notice(auto_failover: bool) -> String {
    let base = "本工具不会修改或切换 CCS 路由配置；但真实路由验证会被 CCS 视为一次正常请求，可能写入日志/统计，并可能触发已配置的重试或故障转移。";
    if auto_failover {
        format!("{base} 当前开启自动故障转移：结果验证的是当前 CCS 路由链，不代表固定 Provider。")
    } else {
        base.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccs_adapter::routing::RoutingStatusView;
    use crate::ccs_adapter::{AuthKind, ProviderKind};
    use secrecy::SecretString;

    fn sample_provider(is_current: bool, source_id: &str) -> NormalizedProvider {
        NormalizedProvider {
            opaque_id: "op1".into(),
            source_id: source_id.into(),
            app_type: AppType::Claude,
            display_name: "Relay".into(),
            category: None,
            auth_kind: AuthKind::ApiKey,
            provider_kind: ProviderKind::ThirdPartyApi,
            base_url: "https://api.example.com/v1".into(),
            api_key: SecretString::from("sk-real-secret-key-value"),
            configured_protocol: Some(ProtocolKind::OpenAiResponses),
            configured_model: Some("upstream-model".into()),
            model_candidates: vec![],
            endpoint_candidates: vec![],
            custom_user_agent: None,
            needs_local_routing: None,
            is_current,
            skip_reason: None,
            masked_key: "sk-rea…alue".into(),
            safe_base_url: "https://api.example.com/v1".into(),
            website_url: None,
            api_format_hint: None,
            preferred_auth: None,
            credential_source: None,
        }
    }

    fn sample_routing(running: bool, active: &str) -> RoutingStatusView {
        RoutingStatusView {
            config_detected: true,
            global_enabled: true,
            listen_address: Some("127.0.0.1".into()),
            listen_port: Some(15721),
            health_reachable: running,
            server_running: running,
            failover_count: Some(0),
            apps: vec![AppRoutingStatusView {
                app_type: "claude".into(),
                app_label: "Claude Code".into(),
                enabled: true,
                auto_failover_enabled: false,
                max_retries: Some(3),
                streaming_first_byte_timeout: Some(60),
                streaming_idle_timeout: Some(120),
                non_streaming_timeout: Some(600),
                active_provider_id: Some(active.into()),
                active_provider_name: Some("Relay".into()),
            }],
            warning: None,
            connect_host: Some("127.0.0.1".into()),
        }
    }

    #[test]
    fn client_protocol_is_anthropic_for_claude_even_if_upstream_responses() {
        assert_eq!(
            client_protocol_for_app(AppType::Claude),
            ProtocolKind::AnthropicMessages
        );
    }

    #[test]
    fn route_request_uses_placeholder_not_real_key() {
        let p = sample_provider(true, "p1");
        let routing = sample_routing(true, "p1");
        let app = routing.apps[0].clone();
        let plans = plan_route_attempts(&p, &routing, DiagnosisMode::Smart, &app);
        assert_eq!(plans.len(), 1);
        // Profile-bound role alias (not dated Anthropic ID).
        assert_eq!(plans[0].model, "claude-sonnet-5");
        assert!(!plans[0].model.contains("20250514"));
        let req = build_route_request(&plans[0]).unwrap();
        let blob = format!("{:?}", req.headers);
        assert!(
            blob.contains(CCS_PROXY_PLACEHOLDER_TOKEN) || req.url.contains("key=") || {
                // x-api-key header
                req.headers
                    .values()
                    .any(|v| v == CCS_PROXY_PLACEHOLDER_TOKEN)
            }
        );
        assert!(!blob.contains("sk-real-secret-key-value"));
        assert!(!req.url.contains("sk-real-secret-key-value"));
        assert!(req.url.contains("127.0.0.1"));
        assert!(req.url.contains("/v1/messages") || req.url.contains("/messages"));
        assert_eq!(req.protocol, ProtocolKind::AnthropicMessages);
    }

    #[test]
    fn non_current_provider_not_applicable() {
        let p = sample_provider(false, "other");
        let routing = sample_routing(true, "p1");
        match route_applicable(&p, &routing, VerifyMode::Auto) {
            RouteApplicability::NotCurrentTarget => {}
            other => panic!("expected NotCurrentTarget, got {other:?}"),
        }
    }

    #[test]
    fn target_mismatch_detected() {
        let p = sample_provider(true, "p-selected");
        let routing = sample_routing(true, "p-other");
        match route_applicable(&p, &routing, VerifyMode::DirectAndRoute) {
            RouteApplicability::TargetMismatch {
                expected, actual, ..
            } => {
                assert_eq!(expected, "p-selected");
                assert_eq!(actual, "p-other");
            }
            other => panic!("expected mismatch, got {other:?}"),
        }
    }

    #[test]
    fn combine_route_ok_direct_variant() {
        let s = combine_attempted_route_and_direct(Some(true), false, true, false, false);
        assert_eq!(s, "CCS_ROUTE_OK_DIRECT_VARIANT");
    }

    #[test]
    fn combine_route_ok_direct_parse_failed() {
        let s = combine_attempted_route_and_direct(Some(true), false, false, true, false);
        assert_eq!(s, "CCS_ROUTE_OK_DIRECT_PARSE_FAILED");
    }

    #[test]
    fn not_current_target_plus_network_unreachable_keeps_direct() {
        let s = combine_route_direct_status(
            None,
            Some("CCS_ROUTE_NOT_APPLICABLE"),
            false,
            false,
            true,
            false,
            true, // route_not_applicable
            false,
            "NETWORK_UNREACHABLE",
        );
        assert_eq!(s, "NETWORK_UNREACHABLE");
    }

    #[test]
    fn not_current_target_plus_auth_invalid_keeps_direct() {
        let s = combine_route_direct_status(
            None,
            Some("CCS_ROUTE_NOT_APPLICABLE"),
            false,
            false,
            true,
            false,
            true,
            false,
            "AUTH_INVALID",
        );
        assert_eq!(s, "AUTH_INVALID");
    }

    #[test]
    fn not_current_target_plus_current_config_ok_keeps_direct() {
        let s = combine_route_direct_status(
            None,
            Some("CCS_ROUTE_NOT_APPLICABLE"),
            true,
            false,
            false,
            false,
            true,
            false,
            "CURRENT_CONFIG_OK",
        );
        assert_eq!(s, "CURRENT_CONFIG_OK");
    }

    #[test]
    fn direct_only_skip_never_becomes_primary() {
        // DirectOnly maps to route_not_applicable via Skip; primary must stay direct.
        let s = combine_route_direct_status(
            None,
            Some("CCS_ROUTE_NOT_APPLICABLE"),
            false,
            false,
            true,
            false,
            true,
            false,
            "KEY_INVALID",
        );
        assert_eq!(s, "KEY_INVALID");
    }

    #[test]
    fn not_running_plus_direct_success_keeps_direct() {
        let s = combine_route_direct_status(
            None,
            Some("CCS_ROUTE_NOT_RUNNING"),
            true,
            false,
            false,
            true, // route_not_running
            false,
            false,
            "CURRENT_CONFIG_OK",
        );
        assert_eq!(s, "CURRENT_CONFIG_OK");
    }

    #[test]
    fn not_running_plus_direct_failure_keeps_direct() {
        let s = combine_route_direct_status(
            None,
            Some("CCS_ROUTE_NOT_RUNNING"),
            false,
            false,
            true,
            true,
            false,
            false,
            "NETWORK_UNREACHABLE",
        );
        assert_eq!(s, "NETWORK_UNREACHABLE");
    }

    #[test]
    fn pre_send_target_mismatch_keeps_direct() {
        let s = combine_route_direct_status(
            None,
            Some("CCS_ROUTE_TARGET_MISMATCH"),
            false,
            false,
            true,
            false,
            false,
            true, // mismatch without route_ok → disposition only
            "MODEL_NOT_FOUND",
        );
        assert_eq!(s, "MODEL_NOT_FOUND");
    }

    #[test]
    fn attempted_route_ok_with_direct_failure_combines() {
        let s = combine_route_direct_status(
            Some(true),
            Some("GENERATE_OK"),
            false,
            false,
            true,
            false,
            false,
            false,
            "NETWORK_UNREACHABLE",
        );
        assert_eq!(s, "CCS_ROUTE_OK_DIRECT_PARSE_FAILED");
    }

    #[test]
    fn attempted_route_failed_with_direct_ok_combines() {
        let s = combine_attempted_route_and_direct(Some(false), true, false, false, false);
        assert_eq!(s, "CCS_ROUTE_FAILED_DIRECT_OK");
    }

    #[test]
    fn attempted_route_and_direct_both_failed() {
        let s = combine_attempted_route_and_direct(Some(false), false, false, true, false);
        assert_eq!(s, "CCS_ROUTE_AND_DIRECT_FAILED");
    }

    #[test]
    fn deep_mode_plans_two_route_attempts() {
        let p = sample_provider(true, "p1");
        let routing = sample_routing(true, "p1");
        let app = routing.apps[0].clone();
        let plans = plan_route_attempts(&p, &routing, DiagnosisMode::Deep, &app);
        assert_eq!(plans.len(), 2);
        assert!(plans[1].stream);
    }
}
