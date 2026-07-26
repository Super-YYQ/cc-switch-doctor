use crate::ccs_adapter::{
    AppType, ModelCandidate, ModelCandidateSource, NormalizedProvider, ProtocolKind,
};
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
            // Quick is low-impact: only the current-config generate request.
            Self::Quick => 1,
            Self::Smart => 12,
            Self::Deep => 16,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlannedAttempt {
    pub base_url: String,
    pub protocol: ProtocolKind,
    /// Wire model placed in the HTTP request body.
    pub model: String,
    /// Display / configured model (may retain local `[1M]` marker).
    pub display_model: String,
    pub model_source: ModelCandidateSource,
    pub equivalent_to_current: bool,
    pub stream: bool,
    pub tool_call: bool,
    pub use_bearer_for_anthropic: bool,
    pub is_current_config: bool,
    pub score: i32,
    pub label: String,
}

impl PlannedAttempt {
    pub fn model_candidate(&self) -> ModelCandidate {
        ModelCandidate::new(
            self.display_model.clone(),
            self.model.clone(),
            self.model_source,
            self.equivalent_to_current,
        )
    }
}

pub fn plan_attempts(provider: &NormalizedProvider, mode: DiagnosisMode) -> Vec<PlannedAttempt> {
    let mut plans = Vec::new();

    let current_candidate = resolve_current_candidate(provider);
    let model = current_candidate.wire_model.clone();
    let display_model = current_candidate.display_model.clone();
    let model_source = current_candidate.source;
    let equivalent = current_candidate.equivalent_to_current;

    let protocol = provider
        .configured_protocol
        .unwrap_or_else(|| default_protocol(provider.app_type));

    // 1. Current config (highest priority) — always send wire_model, never raw [1M].
    let current_use_bearer = matches!(
        provider.preferred_auth,
        Some(crate::protocols::types::AuthScheme::Bearer)
    );
    let current_label = match model_source {
        ModelCandidateSource::LocalMarkerNormalized => {
            format!("当前配置（{display_model} → {model}）")
        }
        _ => "当前配置".into(),
    };
    plans.push(PlannedAttempt {
        base_url: provider.base_url.clone(),
        protocol,
        model: model.clone(),
        display_model: display_model.clone(),
        model_source,
        equivalent_to_current: equivalent,
        stream: false,
        tool_call: false,
        use_bearer_for_anthropic: current_use_bearer,
        is_current_config: true,
        score: 1000,
        label: current_label,
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
            display_model: display_model.clone(),
            model_source,
            equivalent_to_current: equivalent,
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
                display_model: display_model.clone(),
                model_source,
                equivalent_to_current: equivalent,
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
                display_model: display_model.clone(),
                model_source,
                equivalent_to_current: equivalent,
                stream: false,
                tool_call: false,
                use_bearer_for_anthropic: true,
                is_current_config: false,
                score: 700,
                label: "Anthropic + Bearer 认证变体".into(),
            });
        }
    }

    // Additional model candidates (role mappings / discovered), skip current wire model.
    for (mi, cand) in provider
        .model_candidates
        .iter()
        .filter(|c| c.wire_model != model || c.source != model_source)
        .take(3)
        .enumerate()
    {
        let label = match cand.source {
            ModelCandidateSource::ConfiguredRoleMapping => {
                format!("配置内模型映射 {}", cand.wire_model)
            }
            ModelCandidateSource::DiscoveredModel => {
                format!("发现模型 {}", cand.wire_model)
            }
            ModelCandidateSource::DoctorGuess => format!("推测模型 {}", cand.wire_model),
            ModelCandidateSource::LocalMarkerNormalized => {
                format!("模型候选 {} → {}", cand.display_model, cand.wire_model)
            }
            ModelCandidateSource::ConfiguredModel => format!("模型候选 {}", cand.wire_model),
        };
        plans.push(PlannedAttempt {
            base_url: provider.base_url.clone(),
            protocol,
            model: cand.wire_model.clone(),
            display_model: cand.display_model.clone(),
            model_source: cand.source,
            equivalent_to_current: cand.equivalent_to_current,
            stream: false,
            tool_call: false,
            use_bearer_for_anthropic: false,
            is_current_config: false,
            score: 600 - mi as i32,
            label,
        });
    }

    // Streaming (smart: one try on best base; deep: more)
    if matches!(mode, DiagnosisMode::Smart | DiagnosisMode::Deep) {
        plans.push(PlannedAttempt {
            base_url: provider.base_url.clone(),
            protocol,
            model: model.clone(),
            display_model: display_model.clone(),
            model_source,
            equivalent_to_current: equivalent,
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
            display_model: display_model.clone(),
            model_source,
            equivalent_to_current: equivalent,
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
            display_model: display_model.clone(),
            model_source,
            equivalent_to_current: equivalent,
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

fn resolve_current_candidate(provider: &NormalizedProvider) -> ModelCandidate {
    // Prefer structured candidate that is equivalent to current config.
    if let Some(c) = provider
        .model_candidates
        .iter()
        .find(|c| c.equivalent_to_current)
    {
        return c.clone();
    }
    if let Some(c) = provider.model_candidates.first() {
        return c.clone();
    }
    if let Some(ref m) = provider.configured_model {
        return if matches!(provider.app_type, AppType::Claude | AppType::ClaudeDesktop) {
            ModelCandidate::from_configured_claude(m)
        } else {
            ModelCandidate::from_configured_plain(m)
        };
    }
    // No configured model → Doctor guess (not current-config equivalent).
    ModelCandidate::doctor_guess(&default_model(provider.app_type))
}

pub fn estimate_attempts(provider_count: usize, mode: DiagnosisMode) -> usize {
    // rough upper bound: per-provider max
    provider_count.saturating_mul(mode.max_attempts())
}

/// Rank successful evidence quality (higher is better). Used only for final
/// success_plan selection — does not rewrite request execution order.
pub fn success_evidence_rank(plan: &PlannedAttempt) -> u32 {
    let model_rank: u32 = match plan.model_source {
        ModelCandidateSource::ConfiguredModel => 90,
        ModelCandidateSource::LocalMarkerNormalized => 80,
        ModelCandidateSource::ConfiguredRoleMapping => 70,
        ModelCandidateSource::DiscoveredModel => 30,
        ModelCandidateSource::DoctorGuess => 20,
    };
    let mut score: u32 = model_rank;
    // Prefer current URL + current protocol.
    if plan.is_current_config {
        score += 100;
    }
    if plan.equivalent_to_current {
        score += 50;
    }
    // URL / auth / protocol variants are lower than current-config model success.
    if plan.label.contains("URL 修正") {
        score = score.saturating_sub(20).max(55);
    }
    if plan.label.contains("认证变体") {
        score = score.saturating_sub(25).max(50);
    }
    if plan.label.contains("协议候选") {
        score = score.saturating_sub(30).max(45);
    }
    if plan.stream || plan.tool_call {
        score = score.saturating_sub(5);
    }
    score
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
            model_candidates: vec![
                ModelCandidate::from_configured_plain("model-x"),
                ModelCandidate::role_mapping("model-y", "model-y"),
            ],
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

    fn claude_one_m() -> NormalizedProvider {
        let mut p = sample();
        p.app_type = AppType::Claude;
        p.configured_protocol = Some(ProtocolKind::AnthropicMessages);
        p.configured_model = Some("GLM-5.2[1M]".into());
        p.model_candidates = vec![ModelCandidate::from_configured_claude("GLM-5.2[1M]")];
        p.preferred_auth = Some(crate::protocols::types::AuthScheme::XApiKey);
        p
    }

    #[test]
    fn quick_only_current() {
        let p = plan_attempts(&sample(), DiagnosisMode::Quick);
        assert_eq!(p.len(), 1);
        assert!(p[0].is_current_config);
        assert!(!p[0].stream);
        assert!(!p[0].tool_call);
        assert_eq!(DiagnosisMode::Quick.max_attempts(), 1);
    }

    #[test]
    fn quick_has_no_variants_stream_or_tools() {
        let p = plan_attempts(&sample(), DiagnosisMode::Quick);
        assert!(p
            .iter()
            .all(|x| !x.stream && !x.tool_call && x.is_current_config));
        assert!(!p.iter().any(|x| x.label.contains("URL")));
        assert!(!p.iter().any(|x| x.label.contains("协议")));
        assert!(!p.iter().any(|x| x.label.contains("认证")));
        assert!(!p.iter().any(|x| x.label.contains("模型")));
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

    #[test]
    fn current_config_sends_wire_model_not_raw_one_m() {
        let p = plan_attempts(&claude_one_m(), DiagnosisMode::Quick);
        assert_eq!(p[0].model, "GLM-5.2");
        assert_eq!(p[0].display_model, "GLM-5.2[1M]");
        assert_eq!(
            p[0].model_source,
            ModelCandidateSource::LocalMarkerNormalized
        );
        assert!(p[0].equivalent_to_current);
        assert!(p[0].is_current_config);
    }

    #[test]
    fn evidence_rank_prefers_local_marker_over_guess() {
        let local = PlannedAttempt {
            base_url: "https://api.example.com".into(),
            protocol: ProtocolKind::AnthropicMessages,
            model: "GLM-5.2".into(),
            display_model: "GLM-5.2[1M]".into(),
            model_source: ModelCandidateSource::LocalMarkerNormalized,
            equivalent_to_current: true,
            stream: false,
            tool_call: false,
            use_bearer_for_anthropic: false,
            is_current_config: true,
            score: 1000,
            label: "当前配置".into(),
        };
        let guess = PlannedAttempt {
            model: "guess-model".into(),
            display_model: "guess-model".into(),
            model_source: ModelCandidateSource::DoctorGuess,
            equivalent_to_current: false,
            is_current_config: false,
            label: "推测模型".into(),
            ..local.clone()
        };
        assert!(success_evidence_rank(&local) > success_evidence_rank(&guess));
    }
}
