use super::classifier::final_status_from_attempts;
use super::planner::{plan_attempts, DiagnosisMode, PlannedAttempt};
use crate::ccs_adapter::{NormalizedProvider, ProtocolKind};
use crate::protocols::anthropic::build_anthropic_request;
use crate::protocols::gemini::build_gemini_request;
use crate::protocols::http_executor::HttpExecutor;
use crate::protocols::openai_chat::build_chat_request;
use crate::protocols::openai_responses::build_responses_request;
use crate::protocols::types::{default_timeout, AttemptResult};
use crate::security::origin::SameOriginPolicy;
use crate::security::redact::SecretRedactor;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
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

    let concurrency = concurrency.clamp(1, 3) as usize;
    let mut summaries = Vec::new();

    // Process with limited concurrency
    let mut chunks = providers;
    // simple sequential if concurrency=1, else join_set limited
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
            let s = diagnose_one(&exec, &run_id, p, mode, &cancel, &emit).await;
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
                async move { diagnose_one(exec, &run_id, p, mode, &cancel, emit_ref).await }
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

    let origin = SameOriginPolicy::parse_url(&provider.base_url).unwrap_or(SameOriginPolicy {
        scheme: "https".into(),
        host: "invalid.invalid".into(),
        port: None,
    });

    let mut attempts: Vec<AttemptResult> = Vec::new();
    let mut current_ok = false;
    let mut any_ok = false;
    let mut success_plan: Option<&PlannedAttempt> = None;
    let mut success_result: Option<AttemptResult> = None;
    let mut stop_all = false;
    let mut rate_limited = false;

    for (index, plan) in plans.iter().enumerate() {
        if cancel.is_cancelled() || stop_all {
            break;
        }
        // After current success in smart/deep, skip repair attempts but allow deep extras carefully
        if current_ok && !plan.is_current_config {
            if mode == DiagnosisMode::Smart {
                break;
            }
            // deep: still run stream/tool if planned and current was generate
            if !plan.stream && !plan.tool_call {
                continue;
            }
        }
        if rate_limited {
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

        let key = provider.api_key.expose_secret();
        let ua = provider.custom_user_agent.as_deref();
        let built = match plan.protocol {
            ProtocolKind::OpenAiChat => build_chat_request(
                &plan.base_url,
                &plan.model,
                key,
                plan.stream,
                plan.tool_call,
                ua,
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
                plan.use_bearer_for_anthropic,
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
                let r = AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: None,
                    latency_ms: 0,
                    ttft_ms: None,
                    protocol: plan.protocol,
                    model: plan.model.clone(),
                    url: plan.base_url.clone(),
                    stream: plan.stream,
                    purpose: crate::protocols::RequestPurpose::Generate,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("protocol".into()),
                    error_message: Some("未知协议".into()),
                    response_excerpt: None,
                    classification: "UNSUPPORTED_PROTOCOL".into(),
                };
                emit(DiagnosisEvent::AttemptFinished {
                    run_id: run_id.to_string(),
                    opaque_id: provider.opaque_id.clone(),
                    index,
                    result: r.clone(),
                });
                attempts.push(r);
                continue;
            }
        };

        let timeout = default_timeout(mode == DiagnosisMode::Deep || plan.stream);
        let result = exec
            .execute(built, &origin, &redactor, cancel, timeout)
            .await;

        if result.classification == "RATE_LIMITED" || result.classification == "QUOTA_EXHAUSTED" {
            if result.classification == "RATE_LIMITED" {
                rate_limited = true;
            }
            if result.classification == "QUOTA_EXHAUSTED" && plan.is_current_config {
                stop_all = true;
            }
        }
        if result.classification == "KEY_INVALID" && plan.is_current_config {
            // stop expensive attempts if key invalid on current endpoint
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
            if mode == DiagnosisMode::Smart && !plan.stream && !plan.tool_call {
                // found working combo — stop further repair (deep continues limited)
                if !plan.is_current_config {
                    stop_all = true;
                }
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

    // Local routing: codex wants responses but only chat works
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

    let best_class = attempts
        .iter()
        .find(|a| !a.ok)
        .map(|a| a.classification.clone())
        .unwrap_or_else(|| "UNKNOWN_ERROR".into());

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

    let suggestion = build_suggestion(
        &provider,
        current_ok,
        any_ok,
        success_plan,
        needs_local,
        &status,
    );

    let evidence: Vec<String> = attempts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            format!(
                "尝试 {}：{} {} -> {} ({})",
                i + 1,
                if a.stream { "STREAM" } else { "POST" },
                a.url,
                a.status_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "—".into()),
                a.classification
            )
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

    // zeroize-ish: SecretString drops with provider
    summary
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
        "RATE_LIMITED" => "触发限流（429）。请稍后重试或降低并发。本工具未修改配置。".into(),
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
