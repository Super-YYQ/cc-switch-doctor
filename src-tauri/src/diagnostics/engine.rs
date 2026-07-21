use super::classifier::{best_classification, final_status_from_attempts};
use super::planner::{plan_attempts, DiagnosisMode, PlannedAttempt};
use super::session_budget::{cache_key_from_built, key_fingerprint, OriginKey, SessionBudget};
use crate::ccs_adapter::{NormalizedProvider, ProtocolKind};
use crate::protocols::anthropic::build_anthropic_request;
use crate::protocols::gemini::build_gemini_request;
use crate::protocols::http_executor::HttpExecutor;
use crate::protocols::openai_chat::build_chat_request;
use crate::protocols::openai_responses::build_responses_request;
use crate::protocols::types::AuthScheme;
use crate::protocols::types::{
    default_timeout, is_max_completion_tokens_unsupported, AttemptResult, RequestPurpose,
    TokenLimitField,
};
use crate::security::origin::SameOriginPolicy;
use crate::security::redact::SecretRedactor;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDiagnosisRequest {
    pub opaque_ids: Vec<String>,
    pub mode: DiagnosisMode,
    pub concurrency: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiagnosisEvent {
    RunStarted {
        run_id: String,
        provider_count: usize,
        estimated_attempts: usize,
        mode: DiagnosisMode,
    },
    ProviderStarted {
        run_id: String,
        opaque_id: String,
        display_name: String,
        attempt_count: usize,
    },
    AttemptStarted {
        run_id: String,
        opaque_id: String,
        index: usize,
        label: String,
        url: String,
        protocol: String,
        model: String,
    },
    AttemptFinished {
        run_id: String,
        opaque_id: String,
        index: usize,
        result: AttemptResult,
    },
    ProviderFinished {
        run_id: String,
        opaque_id: String,
        summary: ProviderDiagnosisSummary,
    },
    RunCancelled {
        run_id: String,
    },
    RunFinished {
        run_id: String,
        summaries: Vec<ProviderDiagnosisSummary>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDiagnosisSummary {
    pub opaque_id: String,
    pub source_id: String,
    pub display_name: String,
    pub app_label: String,
    pub status: String,
    pub current_config_ok: bool,
    pub any_success: bool,
    pub safe_base_url: String,
    pub configured_protocol: Option<String>,
    pub configured_model: Option<String>,
    pub success_url: Option<String>,
    pub success_protocol: Option<String>,
    pub success_model: Option<String>,
    pub needs_local_routing: Option<bool>,
    pub suggestion: String,
    pub evidence: Vec<String>,
    pub attempts: Vec<AttemptResult>,
    pub confidence: String,
}

pub async fn run_diagnosis(
    run_id: String,
    providers: Vec<NormalizedProvider>,
    mode: DiagnosisMode,
    concurrency: u32,
    cancel: CancellationToken,
    emit: impl Fn(DiagnosisEvent) + Send + Sync + 'static,
) {
    let estimated: usize = providers.iter().map(|p| plan_attempts(p, mode).len()).sum();
    emit(DiagnosisEvent::RunStarted {
        run_id: run_id.clone(),
        provider_count: providers.len(),
        estimated_attempts: estimated,
        mode,
    });

    let exec = match HttpExecutor::new() {
        Ok(e) => e,
        Err(e) => {
            emit(DiagnosisEvent::RunFinished {
                run_id: run_id.clone(),
                summaries: vec![ProviderDiagnosisSummary {
                    opaque_id: String::new(),
                    source_id: String::new(),
                    display_name: "HTTP 客户端".into(),
                    app_label: String::new(),
                    status: "UNKNOWN_ERROR".into(),
                    current_config_ok: false,
                    any_success: false,
                    safe_base_url: String::new(),
                    configured_protocol: None,
                    configured_model: None,
                    success_url: None,
                    success_protocol: None,
                    success_model: None,
                    needs_local_routing: None,
                    suggestion: format!("无法创建 HTTP 客户端：{e}"),
                    evidence: vec![],
                    attempts: vec![],
                    confidence: "low".into(),
                }],
            });
            return;
        }
    };

    let session_budget = Arc::new(SessionBudget::new());
    let concurrency = concurrency.clamp(1, 3) as usize;
    let mut summaries = Vec::new();
    let mut chunks = providers;

    if concurrency == 1 {
        for p in chunks {
            if cancel.is_cancelled() {
                emit(DiagnosisEvent::RunCancelled {
                    run_id: run_id.clone(),
                });
                emit(DiagnosisEvent::RunFinished {
                    run_id: run_id.clone(),
                    summaries: summaries.clone(),
                });
                return;
            }
            let s = diagnose_one(
                &exec,
                &run_id,
                p,
                mode,
                &cancel,
                &emit,
                Arc::clone(&session_budget),
            )
            .await;
            summaries.push(s);
        }
    } else {
        use futures::stream::{self, StreamExt};
        let run_id2 = run_id.clone();
        let cancel2 = cancel.clone();
        let results: Vec<_> = stream::iter(chunks.drain(..))
            .map(|p| {
                let exec = &exec;
                let run_id = run_id2.clone();
                let cancel = cancel2.clone();
                let emit_ref = &emit;
                let budget = Arc::clone(&session_budget);
                async move { diagnose_one(exec, &run_id, p, mode, &cancel, emit_ref, budget).await }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;
        if cancel.is_cancelled() {
            emit(DiagnosisEvent::RunCancelled {
                run_id: run_id.clone(),
            });
        }
        summaries.extend(results);
    }

    emit(DiagnosisEvent::RunFinished { run_id, summaries });
}

async fn diagnose_one(
    exec: &HttpExecutor,
    run_id: &str,
    provider: NormalizedProvider,
    mode: DiagnosisMode,
    cancel: &CancellationToken,
    emit: &impl Fn(DiagnosisEvent),
    session_budget: Arc<SessionBudget>,
) -> ProviderDiagnosisSummary {
    let plans = plan_attempts(&provider, mode);
    emit(DiagnosisEvent::ProviderStarted {
        run_id: run_id.to_string(),
        opaque_id: provider.opaque_id.clone(),
        display_name: provider.display_name.clone(),
        attempt_count: plans.len(),
    });

    let mut redactor = SecretRedactor::new();
    redactor.register_key(provider.api_key.expose_secret());

    let origin_policy =
        SameOriginPolicy::parse_url(&provider.base_url).unwrap_or(SameOriginPolicy {
            scheme: "https".into(),
            host: "invalid.invalid".into(),
            port: None,
        });
    let origin_key = OriginKey::from_policy(&origin_policy);
    let key_fp = key_fingerprint(provider.api_key.expose_secret());

    let mut attempts: Vec<AttemptResult> = Vec::new();
    let mut current_ok = false;
    let mut any_ok = false;
    let mut success_plan: Option<&PlannedAttempt> = None;
    let mut success_result: Option<AttemptResult> = None;
    let mut stop_all = false;
    let mut tried_token_fallback = false;
    let mut token_fallback_note: Option<String> = None;

    for (index, plan) in plans.iter().enumerate() {
        if cancel.is_cancelled() || stop_all {
            break;
        }
        if current_ok && !plan.is_current_config {
            if mode == DiagnosisMode::Smart {
                break;
            }
            if !plan.stream && !plan.tool_call {
                continue;
            }
        }

        if let Some(reason) = session_budget.stop_reason(&origin_key) {
            let r = AttemptResult::budget_stopped(
                plan.protocol,
                &plan.model,
                &crate::security::sanitize_url_for_display(&plan.base_url),
                reason.message(),
                reason.classification(),
            );
            emit(DiagnosisEvent::AttemptFinished {
                run_id: run_id.to_string(),
                opaque_id: provider.opaque_id.clone(),
                index,
                result: r.clone(),
            });
            attempts.push(r);
            break;
        }

        emit(DiagnosisEvent::AttemptStarted {
            run_id: run_id.to_string(),
            opaque_id: provider.opaque_id.clone(),
            index,
            label: plan.label.clone(),
            url: crate::security::sanitize_url_for_display(&plan.base_url),
            protocol: plan.protocol.as_str().to_string(),
            model: plan.model.clone(),
        });

        let mut result = execute_plan(
            exec,
            &provider,
            plan,
            &origin_policy,
            &origin_key,
            &key_fp,
            &redactor,
            cancel,
            mode,
            &session_budget,
            TokenLimitField::MaxCompletionTokens,
        )
        .await;

        if plan.protocol == ProtocolKind::OpenAiChat
            && !result.ok
            && !result.reused_from_cache
            && is_max_completion_tokens_unsupported(&result)
        {
            tried_token_fallback = true;
            emit(DiagnosisEvent::AttemptStarted {
                run_id: run_id.to_string(),
                opaque_id: provider.opaque_id.clone(),
                index,
                label: "字段兼容回退：max_completion_tokens → max_tokens".into(),
                url: crate::security::sanitize_url_for_display(&plan.base_url),
                protocol: plan.protocol.as_str().to_string(),
                model: plan.model.clone(),
            });
            emit(DiagnosisEvent::AttemptFinished {
                run_id: run_id.to_string(),
                opaque_id: provider.opaque_id.clone(),
                index,
                result: result.clone(),
            });
            attempts.push(result);

            result = execute_plan(
                exec,
                &provider,
                plan,
                &origin_policy,
                &origin_key,
                &key_fp,
                &redactor,
                cancel,
                mode,
                &session_budget,
                TokenLimitField::MaxTokens,
            )
            .await;
            if result.ok {
                token_fallback_note =
                    Some("接口不支持 max_completion_tokens，切换为 max_tokens 后请求成功。".into());
                result.suggestion_note = token_fallback_note.clone();
            } else {
                result.suggestion_note = Some(
                    "字段兼容回退：max_completion_tokens → max_tokens 均已尝试，仍未成功。".into(),
                );
            }
        }

        if result.classification == "HOST_BUDGET_EXHAUSTED"
            || result.classification == "HOST_RATE_LIMIT_STOPPED"
        {
            emit(DiagnosisEvent::AttemptFinished {
                run_id: run_id.to_string(),
                opaque_id: provider.opaque_id.clone(),
                index,
                result: result.clone(),
            });
            attempts.push(result);
            break;
        }

        if result.classification == "QUOTA_EXHAUSTED" && plan.is_current_config {
            stop_all = true;
        }
        if matches!(
            result.classification.as_str(),
            "KEY_INVALID" | "AUTH_INVALID" | "AUTH_PERMISSION_DENIED" | "QUOTA_EXHAUSTED"
        ) && plan.is_current_config
        {
            stop_all = true;
        }

        if result.ok {
            any_ok = true;
            if plan.is_current_config {
                current_ok = true;
            }
            if success_plan.is_none() {
                success_plan = Some(plan);
                success_result = Some(result.clone());
            }
            if mode == DiagnosisMode::Smart
                && !plan.stream
                && !plan.tool_call
                && !plan.is_current_config
            {
                stop_all = true;
            }
        }

        emit(DiagnosisEvent::AttemptFinished {
            run_id: run_id.to_string(),
            opaque_id: provider.opaque_id.clone(),
            index,
            result: result.clone(),
        });
        attempts.push(result);
    }

    let protocol_changed = success_plan
        .map(|p| Some(p.protocol) != provider.configured_protocol)
        .unwrap_or(false);
    let url_changed = success_plan
        .map(|p| p.base_url.trim_end_matches('/') != provider.base_url.trim_end_matches('/'))
        .unwrap_or(false);
    let model_changed = success_plan
        .map(|p| Some(p.model.as_str()) != provider.configured_model.as_deref())
        .unwrap_or(false);

    let needs_local = if provider.app_type == crate::ccs_adapter::AppType::Codex {
        if let Some(sp) = success_plan {
            if sp.protocol == ProtocolKind::OpenAiChat
                && provider.configured_protocol != Some(ProtocolKind::OpenAiChat)
            {
                true
            } else {
                provider.needs_local_routing.unwrap_or(false)
                    && sp.protocol == ProtocolKind::OpenAiChat
            }
        } else {
            provider.needs_local_routing.unwrap_or(false)
        }
    } else {
        false
    };

    let best_class = {
        let failed: Vec<&str> = attempts
            .iter()
            .filter(|a| !a.ok)
            .map(|a| a.classification.as_str())
            .collect();
        if failed.is_empty() {
            "UNKNOWN_ERROR".into()
        } else {
            best_classification(failed)
        }
    };

    let status = if provider.skip_reason.is_some() {
        "MANAGED_AUTH_SKIPPED".into()
    } else {
        final_status_from_attempts(
            current_ok,
            any_ok,
            &best_class,
            protocol_changed,
            url_changed,
            model_changed,
            needs_local,
        )
    };

    let mut suggestion = build_suggestion(
        &provider,
        current_ok,
        any_ok,
        success_plan,
        needs_local,
        &status,
    );
    if let Some(note) = token_fallback_note {
        suggestion = format!("{suggestion} {note}");
    } else if tried_token_fallback && !any_ok {
        suggestion =
            format!("{suggestion} 字段兼容回退：max_completion_tokens → max_tokens 均已尝试。");
    }

    let evidence: Vec<String> = attempts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let mut line = format!(
                "尝试 {}：{} {} -> {} ({})",
                i + 1,
                if a.stream { "STREAM" } else { "POST" },
                a.url,
                a.status_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "—".into()),
                a.classification
            );
            if a.reused_from_cache {
                line.push_str(" [复用缓存]");
            }
            if let Some(TokenLimitField::MaxTokens) = a.token_limit_field {
                line.push_str(" [max_tokens]");
            }
            if let Some(note) = &a.suggestion_note {
                line.push_str(&format!(" — {note}"));
            }
            line
        })
        .collect();

    let confidence = if current_ok {
        "high"
    } else if any_ok {
        "medium"
    } else if attempts.iter().any(|a| a.status_code == Some(401)) {
        "high"
    } else {
        "low"
    }
    .to_string();

    let summary = ProviderDiagnosisSummary {
        opaque_id: provider.opaque_id.clone(),
        source_id: provider.source_id.clone(),
        display_name: provider.display_name.clone(),
        app_label: provider.app_type.label_zh().to_string(),
        status: status.clone(),
        current_config_ok: current_ok,
        any_success: any_ok,
        safe_base_url: provider.safe_base_url.clone(),
        configured_protocol: provider.configured_protocol.map(|p| p.label().to_string()),
        configured_model: provider.configured_model.clone(),
        success_url: success_result.as_ref().map(|r| r.url.clone()),
        success_protocol: success_plan.map(|p| p.protocol.label().to_string()),
        success_model: success_plan.map(|p| p.model.clone()),
        needs_local_routing: if needs_local {
            Some(true)
        } else {
            provider.needs_local_routing
        },
        suggestion,
        evidence,
        attempts,
        confidence,
    };

    emit(DiagnosisEvent::ProviderFinished {
        run_id: run_id.to_string(),
        opaque_id: provider.opaque_id.clone(),
        summary: summary.clone(),
    });

    summary
}

#[allow(clippy::too_many_arguments)]
async fn execute_plan(
    exec: &HttpExecutor,
    provider: &NormalizedProvider,
    plan: &PlannedAttempt,
    origin_policy: &SameOriginPolicy,
    origin_key: &OriginKey,
    key_fp: &str,
    redactor: &SecretRedactor,
    cancel: &CancellationToken,
    mode: DiagnosisMode,
    session_budget: &SessionBudget,
    token_field: TokenLimitField,
) -> AttemptResult {
    let safe_url = crate::security::sanitize_url_for_display(&plan.base_url);

    if cancel.is_cancelled() {
        return AttemptResult {
            ok: false,
            partial: false,
            status_code: None,
            latency_ms: 0,
            ttft_ms: None,
            protocol: plan.protocol,
            model: plan.model.clone(),
            url: safe_url,
            stream: plan.stream,
            purpose: if plan.tool_call {
                RequestPurpose::ToolCall
            } else if plan.stream {
                RequestPurpose::StreamGenerate
            } else {
                RequestPurpose::Generate
            },
            extracted_text: None,
            tool_call_ok: None,
            error_kind: Some("cancelled".into()),
            error_message: Some("已取消".into()),
            response_excerpt: None,
            classification: "CANCELLED".into(),
            http_sent: false,
            reused_from_cache: false,
            suggestion_note: None,
            token_limit_field: Some(token_field),
            error_evidence: vec![],
        };
    }

    let key = provider.api_key.expose_secret();
    let ua = provider.custom_user_agent.as_deref();
    let auth_scheme = match plan.protocol {
        ProtocolKind::OpenAiChat | ProtocolKind::OpenAiResponses => AuthScheme::Bearer,
        ProtocolKind::AnthropicMessages => {
            if plan.use_bearer_for_anthropic {
                AuthScheme::Bearer
            } else {
                provider.preferred_auth.unwrap_or(AuthScheme::XApiKey)
            }
        }
        ProtocolKind::GeminiNative => AuthScheme::XGoogApiKey,
        ProtocolKind::Unknown => AuthScheme::Bearer,
    };

    let built = match plan.protocol {
        ProtocolKind::OpenAiChat => build_chat_request(
            &plan.base_url,
            &plan.model,
            key,
            plan.stream,
            plan.tool_call,
            ua,
            token_field,
        ),
        ProtocolKind::OpenAiResponses => build_responses_request(
            &plan.base_url,
            &plan.model,
            key,
            plan.stream,
            plan.tool_call,
            ua,
        ),
        ProtocolKind::AnthropicMessages => build_anthropic_request(
            &plan.base_url,
            &plan.model,
            key,
            plan.stream,
            plan.tool_call,
            matches!(auth_scheme, AuthScheme::Bearer),
            ua,
        ),
        ProtocolKind::GeminiNative => build_gemini_request(
            &plan.base_url,
            &plan.model,
            key,
            plan.stream,
            plan.tool_call,
            ua,
        ),
        ProtocolKind::Unknown => {
            return AttemptResult {
                ok: false,
                partial: false,
                status_code: None,
                latency_ms: 0,
                ttft_ms: None,
                protocol: plan.protocol,
                model: plan.model.clone(),
                url: safe_url,
                stream: plan.stream,
                purpose: RequestPurpose::Generate,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("protocol".into()),
                error_message: Some("未知协议".into()),
                response_excerpt: None,
                classification: "UNSUPPORTED_PROTOCOL".into(),
                http_sent: false,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
                error_evidence: vec![],
            };
        }
    };

    let token_for_key = if plan.protocol == ProtocolKind::OpenAiChat {
        Some(token_field)
    } else {
        None
    };
    let cache_key = cache_key_from_built(origin_key, &built, key_fp, token_for_key, auth_scheme);

    if let Some(cached) = session_budget.get_cached(&cache_key) {
        return cached;
    }
    if let Some(rx) = session_budget.begin_flight(&cache_key) {
        return match rx.await {
            Ok(mut r) => {
                r.reused_from_cache = true;
                r.http_sent = false;
                r
            }
            Err(_) => AttemptResult {
                ok: false,
                partial: false,
                status_code: None,
                latency_ms: 0,
                ttft_ms: None,
                protocol: plan.protocol,
                model: plan.model.clone(),
                url: safe_url.clone(),
                stream: plan.stream,
                purpose: built.purpose,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("cancelled".into()),
                error_message: Some("等待相同请求时被取消".into()),
                response_excerpt: None,
                classification: "CANCELLED".into(),
                http_sent: false,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: token_for_key,
                error_evidence: vec![],
            },
        };
    }

    if let Err(reason) = session_budget.try_reserve_send(origin_key) {
        let r = AttemptResult::budget_stopped(
            plan.protocol,
            &plan.model,
            &safe_url,
            reason.message(),
            reason.classification(),
        );
        session_budget.finish_flight(&cache_key, r.clone());
        return r;
    }

    if cancel.is_cancelled() {
        session_budget.release_unsent(origin_key);
        let r = AttemptResult {
            ok: false,
            partial: false,
            status_code: None,
            latency_ms: 0,
            ttft_ms: None,
            protocol: plan.protocol,
            model: plan.model.clone(),
            url: safe_url,
            stream: plan.stream,
            purpose: built.purpose,
            extracted_text: None,
            tool_call_ok: None,
            error_kind: Some("cancelled".into()),
            error_message: Some("已取消".into()),
            response_excerpt: None,
            classification: "CANCELLED".into(),
            http_sent: false,
            reused_from_cache: false,
            suggestion_note: None,
            token_limit_field: token_for_key,
            error_evidence: vec![],
        };
        session_budget.finish_flight(&cache_key, r.clone());
        return r;
    }

    let timeout = default_timeout(mode == DiagnosisMode::Deep || plan.stream);
    let mut result = exec
        .execute(built, origin_policy, redactor, cancel, timeout)
        .await;
    result.token_limit_field = token_for_key;

    if !result.http_sent {
        session_budget.release_unsent(origin_key);
    } else {
        session_budget.record_result(origin_key, &result.classification);
    }
    session_budget.finish_flight(&cache_key, result.clone());
    result
}

fn build_suggestion(
    provider: &NormalizedProvider,
    current_ok: bool,
    any_ok: bool,
    success: Option<&PlannedAttempt>,
    needs_local: bool,
    status: &str,
) -> String {
    if current_ok {
        return format!(
            "当前配置可用。Base URL：{}，协议：{}，模型：{}。本工具未修改任何配置。",
            provider.safe_base_url,
            provider
                .configured_protocol
                .map(|p| p.label())
                .unwrap_or("—"),
            provider.configured_model.as_deref().unwrap_or("—")
        );
    }
    if let Some(s) = success {
        if any_ok {
            let mut msg = format!(
                "供应商、Key 与模型可用，但当前配置未直接成功。成功组合：Base URL = {}，协议 = {}，模型 = {}。",
                crate::security::sanitize_url_for_display(&s.base_url),
                s.protocol.label(),
                s.model
            );
            if needs_local {
                msg.push_str(" Codex 场景下上游更像 Chat Completions：建议在 CC Switch 中将 API 格式设为 Chat，并启用本地路由/协议转换。");
            } else {
                msg.push_str(" 建议在 CC Switch 中按成功组合调整 Base URL / 协议 / 模型字段。");
            }
            msg.push_str(" 本工具未自动修改任何配置。");
            return msg;
        }
    }
    match status {
        "KEY_INVALID" => {
            "鉴权失败：API Key 无效或未授权。请在 CC Switch 中更新 Key。本工具未修改配置。".into()
        }
        "QUOTA_EXHAUSTED" => "额度不足或配额耗尽。请检查供应商余额。本工具未修改配置。".into(),
        "RATE_LIMITED" | "HOST_RATE_LIMIT_STOPPED" => {
            "触发限流（429）。请稍后重试或降低并发。本工具未修改配置。".into()
        }
        "HOST_BUDGET_EXHAUSTED" => {
            "该 Host 在本次诊断会话中已达到 30 次请求上限。本工具未修改配置。".into()
        }
        "MODEL_NOT_FOUND" => "模型不存在或无权访问。请检查模型名映射。本工具未修改配置。".into(),
        "ENDPOINT_NOT_FOUND" => {
            "端点不存在（404）。可尝试在智能诊断中修正 /v1 或协议。本工具未修改配置。".into()
        }
        "NETWORK_UNREACHABLE" => "网络不可达。请检查代理、DNS 与防火墙。本工具未修改配置。".into(),
        "TLS_ERROR" => {
            "TLS/证书错误。请通过系统信任链修复证书，不要关闭校验。本工具未修改配置。".into()
        }
        "MANAGED_AUTH_SKIPPED" => "托管登录/OAuth 配置已安全跳过。".into(),
        _ => format!("诊断状态：{status}。请查看尝试链与错误摘要。本工具未修改任何配置。"),
    }
}
