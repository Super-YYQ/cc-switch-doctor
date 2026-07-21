use crate::ccs_adapter::{AppType, NormalizedProvider, ProtocolKind};
use crate::security::url_variants::normalize_base_candidates;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisMode {
    Quick,
    Smart,
    Deep,
}

impl DiagnosisMode {
    pub fn max_attempts(self) -> usize {
        match self {
            Self::Quick => 2,
            Self::Smart => 12,
            Self::Deep => 16,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlannedAttempt {
    pub base_url: String,
    pub protocol: ProtocolKind,
    pub model: String,
    pub stream: bool,
    pub tool_call: bool,
    pub use_bearer_for_anthropic: bool,
    pub is_current_config: bool,
    pub score: i32,
    pub label: String,
}

pub fn plan_attempts(provider: &NormalizedProvider, mode: DiagnosisMode) -> Vec<PlannedAttempt> {
    let mut plans = Vec::new();
    let model = provider
        .configured_model
        .clone()
        .or_else(|| provider.model_candidates.first().cloned())
        .unwrap_or_else(|| default_model(provider.app_type));

    let protocol = provider
        .configured_protocol
        .unwrap_or_else(|| default_protocol(provider.app_type));

    // 1. Current config (highest priority)
    let current_use_bearer = matches!(
        provider.preferred_auth,
        Some(crate::protocols::types::AuthScheme::Bearer)
    );
    plans.push(PlannedAttempt {
        base_url: provider.base_url.clone(),
        protocol,
        model: model.clone(),
        stream: false,
        tool_call: false,
        use_bearer_for_anthropic: current_use_bearer,
        is_current_config: true,
        score: 1000,
        label: "当前配置".into(),
    });

    if mode == DiagnosisMode::Quick {
        return plans.into_iter().take(mode.max_attempts()).collect();
    }

    let bases = normalize_base_candidates(&provider.base_url, &provider.endpoint_candidates);
    let protocols = protocol_candidates(
        provider.app_type,
        protocol,
        provider.api_format_hint.as_deref(),
    );

    // URL fixes with same protocol
    for (i, base) in bases.iter().enumerate() {
        if base == &provider.base_url {
            continue;
        }
        plans.push(PlannedAttempt {
            base_url: base.clone(),
            protocol,
            model: model.clone(),
            stream: false,
            tool_call: false,
            use_bearer_for_anthropic: false,
            is_current_config: false,
            score: 900 - i as i32,
            label: format!("URL 修正 + {}", protocol.label()),
        });
    }

    // Protocol fallbacks with URL variants (limited)
    for (pi, proto) in protocols.iter().enumerate() {
        if *proto == protocol {
            continue;
        }
        for (bi, base) in bases.iter().take(3).enumerate() {
            plans.push(PlannedAttempt {
                base_url: base.clone(),
                protocol: *proto,
                model: model.clone(),
                stream: false,
                tool_call: false,
                use_bearer_for_anthropic: false,
                is_current_config: false,
                score: 800 - (pi as i32 * 10) - bi as i32,
                label: format!("协议候选 {}", proto.label()),
            });
        }
    }

    // Anthropic bearer variant (relay style) — only for anthropic protocol
    if matches!(protocol, ProtocolKind::AnthropicMessages)
        || protocols.contains(&ProtocolKind::AnthropicMessages)
    {
        for base in bases.iter().take(2) {
            plans.push(PlannedAttempt {
                base_url: base.clone(),
                protocol: ProtocolKind::AnthropicMessages,
                model: model.clone(),
                stream: false,
                tool_call: false,
                use_bearer_for_anthropic: true,
                is_current_config: false,
                score: 700,
                label: "Anthropic + Bearer 认证变体".into(),
            });
        }
    }

    // Model candidates
    for (mi, m) in provider.model_candidates.iter().skip(1).take(2).enumerate() {
        plans.push(PlannedAttempt {
            base_url: provider.base_url.clone(),
            protocol,
            model: m.clone(),
            stream: false,
            tool_call: false,
            use_bearer_for_anthropic: false,
            is_current_config: false,
            score: 600 - mi as i32,
            label: format!("模型候选 {m}"),
        });
    }

    // Streaming (smart: one try on best base; deep: more)
    if matches!(mode, DiagnosisMode::Smart | DiagnosisMode::Deep) {
        plans.push(PlannedAttempt {
            base_url: provider.base_url.clone(),
            protocol,
            model: model.clone(),
            stream: true,
            tool_call: false,
            use_bearer_for_anthropic: false,
            is_current_config: false,
            score: 500,
            label: "流式 SSE".into(),
        });
    }

    if mode == DiagnosisMode::Deep {
        plans.push(PlannedAttempt {
            base_url: provider.base_url.clone(),
            protocol,
            model: model.clone(),
            stream: false,
            tool_call: true,
            use_bearer_for_anthropic: false,
            is_current_config: false,
            score: 400,
            label: "Tool Calling".into(),
        });
        // stability second generate
        plans.push(PlannedAttempt {
            base_url: provider.base_url.clone(),
            protocol,
            model: model.clone(),
            stream: false,
            tool_call: false,
            use_bearer_for_anthropic: false,
            is_current_config: false,
            score: 390,
            label: "稳定性复测".into(),
        });
    }

    // Sort by score desc, dedupe key
    plans.sort_by_key(|b| std::cmp::Reverse(b.score));
    let mut seen = std::collections::HashSet::new();
    plans.retain(|p| {
        let key = format!(
            "{}|{:?}|{}|{}|{}|{}",
            p.base_url, p.protocol, p.model, p.stream, p.tool_call, p.use_bearer_for_anthropic
        );
        seen.insert(key)
    });

    plans.truncate(mode.max_attempts());
    plans
}

pub fn estimate_attempts(provider_count: usize, mode: DiagnosisMode) -> usize {
    // rough upper bound: per-provider max
    provider_count.saturating_mul(mode.max_attempts())
}

fn protocol_candidates(
    app: AppType,
    current: ProtocolKind,
    api_format: Option<&str>,
) -> Vec<ProtocolKind> {
    let mut v = vec![current];
    let push = |v: &mut Vec<ProtocolKind>, p: ProtocolKind| {
        if !v.contains(&p) {
            v.push(p);
        }
    };
    match app {
        AppType::Codex => {
            push(&mut v, ProtocolKind::OpenAiResponses);
            push(&mut v, ProtocolKind::OpenAiChat);
        }
        AppType::Claude | AppType::ClaudeDesktop => {
            push(&mut v, ProtocolKind::AnthropicMessages);
            if api_format.map(|s| s.contains("openai")).unwrap_or(false) {
                push(&mut v, ProtocolKind::OpenAiChat);
                push(&mut v, ProtocolKind::OpenAiResponses);
            } else {
                // only add openai chat as low-priority for relays
                push(&mut v, ProtocolKind::OpenAiChat);
            }
        }
        AppType::Gemini => {
            push(&mut v, ProtocolKind::GeminiNative);
            push(&mut v, ProtocolKind::OpenAiChat);
        }
        AppType::OpenCode => {
            push(&mut v, ProtocolKind::OpenAiChat);
            push(&mut v, ProtocolKind::AnthropicMessages);
            push(&mut v, ProtocolKind::OpenAiResponses);
        }
        _ => {
            push(&mut v, ProtocolKind::OpenAiChat);
            push(&mut v, ProtocolKind::OpenAiResponses);
            push(&mut v, ProtocolKind::AnthropicMessages);
        }
    }
    v
}

fn default_protocol(app: AppType) -> ProtocolKind {
    match app {
        AppType::Claude | AppType::ClaudeDesktop => ProtocolKind::AnthropicMessages,
        AppType::Codex => ProtocolKind::OpenAiResponses,
        AppType::Gemini => ProtocolKind::GeminiNative,
        _ => ProtocolKind::OpenAiChat,
    }
}

fn default_model(app: AppType) -> String {
    // Profile-bound defaults — never invent dated Anthropic IDs.
    // Source: docs/research/v0.1.7-source-review.md + compatibility routingProfiles.
    crate::ccs_adapter::routing_profile::default_direct_model_guess(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    fn sample() -> NormalizedProvider {
        NormalizedProvider {
            opaque_id: "x".into(),
            source_id: "s".into(),
            app_type: AppType::Codex,
            display_name: "t".into(),
            category: None,
            auth_kind: crate::ccs_adapter::AuthKind::BearerToken,
            provider_kind: crate::ccs_adapter::ProviderKind::ThirdPartyApi,
            base_url: "https://api.example.com".into(),
            api_key: SecretString::from("sk-test-key-12345678"),
            configured_protocol: Some(ProtocolKind::OpenAiResponses),
            configured_model: Some("model-x".into()),
            model_candidates: vec!["model-x".into(), "model-y".into()],
            endpoint_candidates: vec!["https://api.example.com".into()],
            custom_user_agent: None,
            needs_local_routing: Some(true),
            is_current: true,
            skip_reason: None,
            masked_key: "sk-tes…5678".into(),
            safe_base_url: "https://api.example.com".into(),
            website_url: None,
            api_format_hint: None,
            preferred_auth: Some(crate::protocols::types::AuthScheme::Bearer),
            credential_source: Some("OPENAI_API_KEY".into()),
        }
    }

    #[test]
    fn quick_only_current() {
        let p = plan_attempts(&sample(), DiagnosisMode::Quick);
        assert!(p.len() <= 2);
        assert!(p[0].is_current_config);
    }

    #[test]
    fn smart_has_url_and_protocol() {
        let p = plan_attempts(&sample(), DiagnosisMode::Smart);
        assert!(p.len() > 1);
        assert!(p.len() <= 12);
        assert!(p.iter().any(|x| x.base_url.ends_with("/v1")));
        assert!(p.iter().any(|x| x.protocol == ProtocolKind::OpenAiChat));
    }

    #[test]
    fn deep_includes_tool_call() {
        let p = plan_attempts(&sample(), DiagnosisMode::Deep);
        assert!(p.iter().any(|x| x.tool_call));
    }
}
