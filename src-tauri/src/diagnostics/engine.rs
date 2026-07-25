use super::classifier::{best_classification, final_status_from_attempts, ModelSuccessKind};
use super::outcome::{
    disposition_from_skip_message, CapabilityOutcome, DirectChannelSummary, RouteChannelSummary,
    RouteDisposition,
};
use super::planner::{plan_attempts, success_evidence_rank, DiagnosisMode, PlannedAttempt};
use super::route_flight::{RouteFlight, RouteReservation};
use super::route_planner::{
    build_route_request, combine_attempted_route_and_direct, plan_route_attempts, route_applicable,
    route_side_effect_notice, RouteApplicability, VerifyMode, ROUTE_SEND_BUDGET_PER_APP,
};
use super::session_budget::{
    cache_key_from_built, key_fingerprint, provider_send_budget, OriginKey, SessionBudget,
};
use crate::ccs_adapter::routing::{active_provider_for_app, probe_status_only, RoutingStatusView};
use crate::ccs_adapter::ModelCandidateSource;
use crate::ccs_adapter::{NormalizedProvider, ProtocolKind};
use crate::protocols::anthropic::build_anthropic_request;
use crate::protocols::gemini::build_gemini_request_with_auth;
use crate::protocols::http_executor::HttpExecutor;
use crate::protocols::openai_chat::build_chat_request;
use crate::protocols::openai_responses::build_responses_request;
use crate::protocols::types::AuthScheme;
use crate::protocols::types::{
    default_timeout, is_max_completion_tokens_unsupported, AttemptResult, DiagnosisChannel,
    RequestPurpose, TokenLimitField,
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
    /// auto | direct_only | direct_and_route
    #[serde(default)]
    pub verify_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
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
    /// Primary provider outcome. Derived from direct when route was not attempted;
    /// only combines route+direct when a real CCS route business request was sent.
    /// Kept as `status` for v0.1.6 UI compatibility (alias of primary_outcome).
    pub status: String,
    /// Explicit primary outcome (same value as `status` in v0.1.7).
    #[serde(default)]
    pub primary_outcome: String,
    pub current_config_ok: bool,
    pub any_success: bool,
    pub safe_base_url: String,
    pub configured_protocol: Option<String>,
    pub configured_model: Option<String>,
    /// Wire / outbound model that succeeded (may differ from configured by [1M] strip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbound_model: Option<String>,
    /// Human note about model transform (e.g. local [1M] normalization).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_transform: Option<String>,
    pub success_url: Option<String>,
    pub success_protocol: Option<String>,
    pub success_model: Option<String>,
    pub needs_local_routing: Option<bool>,
    pub suggestion: String,
    pub evidence: Vec<String>,
    pub attempts: Vec<AttemptResult>,
    pub confidence: String,
    /// Layered direct-channel summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct: Option<DirectChannelSummary>,
    /// Layered route-channel summary (disposition is never primary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<RouteChannelSummary>,
    /// Legacy flat route status code (derived from route.overall_status / disposition).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_status: Option<String>,
    /// Legacy flat direct status code (derived from direct.status).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_status: Option<String>,
    #[serde(default)]
    pub route_side_effect_notice: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn run_diagnosis(
    run_id: String,
    providers: Vec<NormalizedProvider>,
    mode: DiagnosisMode,
    concurrency: u32,
    cancel: CancellationToken,
    emit: impl Fn(DiagnosisEvent) + Send + Sync + 'static,
    routing: Option<RoutingStatusView>,
    verify_mode: VerifyMode,
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
                    primary_outcome: "UNKNOWN_ERROR".into(),
                    current_config_ok: false,
                    any_success: false,
                    safe_base_url: String::new(),
                    configured_protocol: None,
                    configured_model: None,
                    outbound_model: None,
                    model_transform: None,
                    success_url: None,
                    success_protocol: None,
                    success_model: None,
                    needs_local_routing: None,
                    suggestion: format!("无法创建 HTTP 客户端：{e}"),
                    evidence: vec![],
                    attempts: vec![],
                    confidence: "low".into(),
                    direct: None,
                    route: None,
                    route_status: None,
                    direct_status: None,
                    route_side_effect_notice: None,
                }],
            });
            return;
        }
    };

    let session_budget = Arc::new(SessionBudget::new());
    // Atomic per-app route single-flight (async waiters; no Condvar).
    let route_flight = Arc::new(RouteFlight::new());
    let concurrency = concurrency.clamp(1, 3) as usize;
    let mut summaries = Vec::new();
    let mut chunks = providers;
    let routing = Arc::new(routing);

    if concurrency == 1 {
        for p in chunks {
            if cancel.is_cancelled() {
                route_flight.cancel_all();
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
                routing.as_ref().clone(),
                verify_mode,
                Arc::clone(&route_flight),
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
                let routing = routing.as_ref().clone();
                let flight = Arc::clone(&route_flight);
                async move {
                    diagnose_one(
                        exec,
                        &run_id,
                        p,
                        mode,
                        &cancel,
                        emit_ref,
                        budget,
                        routing,
                        verify_mode,
                        flight,
                    )
                    .await
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;
        if cancel.is_cancelled() {
            route_flight.cancel_all();
            emit(DiagnosisEvent::RunCancelled {
                run_id: run_id.clone(),
            });
        }
        summaries.extend(results);
    }

    emit(DiagnosisEvent::RunFinished { run_id, summaries });
}

#[allow(clippy::too_many_arguments)]
async fn diagnose_one(
    exec: &HttpExecutor,
    run_id: &str,
    provider: NormalizedProvider,
    mode: DiagnosisMode,
    cancel: &CancellationToken,
    emit: &impl Fn(DiagnosisEvent),
    session_budget: Arc<SessionBudget>,
    routing: Option<RoutingStatusView>,
    verify_mode: VerifyMode,
    route_flight: Arc<RouteFlight>,
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
    let mut provider_real_sends: usize = 0;
    let provider_budget = provider_send_budget(mode);
    let model_is_guessed = provider.configured_model.is_none();

    // --- CCS local route channel ---
    // Route disposition is auxiliary metadata. It only participates in Provider
    // primary status when a real route HTTP request was sent (route_attempted).
    let mut route_ok: Option<bool> = None;
    let mut route_target_mismatch = false;
    let mut route_notice: Option<String> = None;
    let mut route_disposition = RouteDisposition::NotRequested;
    let mut route_generate: Option<CapabilityOutcome> = None;
    let mut route_streaming: Option<CapabilityOutcome> = None;
    let mut route_actual_provider_id: Option<String> = None;
    let mut route_actual_provider_name: Option<String> = None;
    let mut failover_count_before: Option<u64> = None;
    let mut failover_count_after: Option<u64> = None;
    let mut after_status_unconfirmed = false;
    let route_index_base = 10_000usize;

    if let Some(ref routing_view) = routing {
        match route_applicable(&provider, routing_view, verify_mode) {
            RouteApplicability::Yes(app_row) => {
                let app_key = provider.app_type.as_str().to_string();
                match route_flight.reserve(&app_key) {
                    RouteReservation::AlreadyCompleted(shared) => {
                        // App route already verified by another leader this run.
                        // Reuse App evidence only — never claim this Provider succeeded.
                        route_disposition = RouteDisposition::NotCurrentTarget;
                        route_notice = shared.notice;
                        failover_count_before = shared.failover_count_before;
                        failover_count_after = shared.failover_count_after;
                        route_actual_provider_id = shared.actual_provider_id;
                        route_actual_provider_name = shared.actual_provider_name;
                    }
                    RouteReservation::Waiter(rx) => {
                        if let Ok(shared) = rx.await {
                            route_disposition = RouteDisposition::NotCurrentTarget;
                            route_notice = shared.notice;
                            failover_count_before = shared.failover_count_before;
                            failover_count_after = shared.failover_count_after;
                            route_actual_provider_id = shared.actual_provider_id;
                            route_actual_provider_name = shared.actual_provider_name;
                        } else {
                            route_disposition = RouteDisposition::NotCurrentTarget;
                        }
                    }
                    RouteReservation::Leader => {
                        // /status before the business request.
                        let before_snap = match (
                            routing_view.connect_host.as_deref(),
                            routing_view.listen_port,
                        ) {
                            (Some(h), Some(p)) => probe_status_only(h, p).await,
                            _ => None,
                        };
                        if let Some(ref snap) = before_snap {
                            failover_count_before = Some(snap.failover_count);
                            if let Some((id, name)) =
                                active_provider_for_app(snap, provider.app_type.as_str())
                            {
                                route_actual_provider_id = Some(id.to_string());
                                route_actual_provider_name = name.map(|s| s.to_string());
                                if id != provider.source_id {
                                    route_target_mismatch = true;
                                }
                            }
                        }

                        let rplans = plan_route_attempts(&provider, routing_view, mode, &app_row);
                        let mut sent = 0usize;
                        for (ri, rplan) in rplans.iter().enumerate() {
                            if cancel.is_cancelled() || sent >= ROUTE_SEND_BUDGET_PER_APP {
                                break;
                            }
                            let Some(built) = build_route_request(rplan) else {
                                continue;
                            };
                            let origin_policy = SameOriginPolicy::parse_url(&rplan.base_url)
                                .unwrap_or(SameOriginPolicy {
                                    scheme: "http".into(),
                                    host: "127.0.0.1".into(),
                                    port: routing_view.listen_port,
                                });
                            let idx = route_index_base + ri;
                            emit(DiagnosisEvent::AttemptStarted {
                                run_id: run_id.to_string(),
                                opaque_id: provider.opaque_id.clone(),
                                index: idx,
                                label: rplan.label.clone(),
                                url: crate::security::sanitize_url_for_display(&built.url),
                                protocol: rplan.protocol.as_str().to_string(),
                                model: rplan.model.clone(),
                            });
                            let mut result = exec
                                .execute(
                                    built,
                                    &origin_policy,
                                    &redactor,
                                    cancel,
                                    default_timeout(mode == DiagnosisMode::Deep),
                                )
                                .await;
                            result.channel = DiagnosisChannel::CcsLocalRoute;
                            result.requested_protocol = Some(rplan.protocol);
                            if result.http_sent {
                                sent += 1;
                                route_disposition = RouteDisposition::Attempted;
                            }
                            if result.ok {
                                route_ok = Some(true);
                                route_notice = Some(route_side_effect_notice(rplan.auto_failover));
                                if rplan.auto_failover {
                                    result.suggestion_note = Some(route_side_effect_notice(true));
                                }
                            } else if route_ok.is_none() {
                                route_ok = Some(false);
                            }
                            let cap = CapabilityOutcome::from_ok(
                                result.ok,
                                result.classification.clone(),
                            );
                            if rplan.stream {
                                route_streaming = Some(cap);
                            } else {
                                route_generate = Some(cap);
                            }
                            emit(DiagnosisEvent::AttemptFinished {
                                run_id: run_id.to_string(),
                                opaque_id: provider.opaque_id.clone(),
                                index: idx,
                                result: result.clone(),
                            });
                            attempts.push(result);
                        }

                        // /status after the business request.
                        let after_snap = match (
                            routing_view.connect_host.as_deref(),
                            routing_view.listen_port,
                        ) {
                            (Some(h), Some(p)) => probe_status_only(h, p).await,
                            _ => None,
                        };
                        if let Some(ref snap) = after_snap {
                            failover_count_after = Some(snap.failover_count);
                            if let Some((id, name)) =
                                active_provider_for_app(snap, provider.app_type.as_str())
                            {
                                route_actual_provider_id = Some(id.to_string());
                                route_actual_provider_name = name.map(|s| s.to_string());
                                if id != provider.source_id {
                                    route_target_mismatch = true;
                                }
                            }
                            if let (Some(before), Some(after)) =
                                (failover_count_before, failover_count_after)
                            {
                                if after > before {
                                    let note = format!(
                                        "请求期间故障转移次数 {before} → {after}；结果验证的是当前 CCS 路由链。"
                                    );
                                    route_notice = Some(match route_notice.take() {
                                        Some(prev) => format!("{prev} {note}"),
                                        None => note,
                                    });
                                }
                            }
                        } else if sent > 0 {
                            after_status_unconfirmed = true;
                            let note =
                                "无法确认请求后实际路由目标（/status 刷新失败）。".to_string();
                            route_notice = Some(match route_notice.take() {
                                Some(prev) => format!("{prev} {note}"),
                                None => note,
                            });
                        }

                        if route_notice.is_none() && app_row.auto_failover_enabled {
                            route_notice = Some(route_side_effect_notice(true));
                        }

                        if route_target_mismatch && route_ok == Some(true) {
                            if let Some(last) = attempts
                                .iter_mut()
                                .rev()
                                .find(|a| a.channel == DiagnosisChannel::CcsLocalRoute && a.ok)
                            {
                                last.classification = "CCS_ROUTE_TARGET_MISMATCH".into();
                                last.suggestion_note = Some(
                                    "CCS 路由请求成功，但实际由另一 Provider 处理；本结果验证的是当前路由链，不代表所选 Provider 已通过。".into(),
                                );
                            }
                        }

                        if sent == 0 {
                            route_disposition = RouteDisposition::NotConfigured;
                        }

                        // Publish App-level summary for waiters (not a Provider success claim).
                        let shared = RouteChannelSummary {
                            disposition: if sent > 0 {
                                RouteDisposition::Attempted
                            } else {
                                RouteDisposition::NotConfigured
                            },
                            attempted: sent > 0,
                            generate: route_generate.clone(),
                            streaming: route_streaming.clone(),
                            overall_status: if route_target_mismatch && sent > 0 {
                                Some("CCS_ROUTE_TARGET_MISMATCH".into())
                            } else if route_ok == Some(true) {
                                Some("CCS_ROUTE_OK".into())
                            } else if route_ok == Some(false) {
                                Some("CCS_ROUTE_FAILED".into())
                            } else {
                                Some("CCS_ROUTE_NOT_APPLICABLE".into())
                            },
                            actual_provider_id: route_actual_provider_id.clone(),
                            actual_provider_name: route_actual_provider_name.clone(),
                            failover_count_before,
                            failover_count_after,
                            notice: route_notice.clone(),
                        };
                        route_flight.complete(&app_key, shared);
                    }
                }
            }
            RouteApplicability::NotRunning => {
                route_disposition = RouteDisposition::NotRunning;
            }
            RouteApplicability::NotCurrentTarget => {
                route_disposition = RouteDisposition::NotCurrentTarget;
            }
            RouteApplicability::TargetMismatch {
                actual,
                actual_name,
                ..
            } => {
                // Pre-send applicability only: disposition, not primary.
                route_disposition = RouteDisposition::NotCurrentTarget;
                route_actual_provider_id = Some(actual);
                route_actual_provider_name = actual_name;
            }
            RouteApplicability::Skip(msg) => {
                route_disposition = disposition_from_skip_message(&msg);
            }
        }
    } else if verify_mode == VerifyMode::DirectAndRoute {
        route_disposition = RouteDisposition::NotRunning;
    } else if verify_mode == VerifyMode::DirectOnly {
        route_disposition = RouteDisposition::NotRequested;
    }

    for (index, plan) in plans.iter().enumerate() {
        if cancel.is_cancelled() || stop_all {
            break;
        }
        if provider_real_sends >= provider_budget {
            let r = AttemptResult::budget_stopped(
                plan.protocol,
                &plan.model,
                &crate::security::sanitize_url_for_display(&plan.base_url),
                &format!(
                    "已停止继续请求：该 Provider 在本模式（{:?}）下真实请求已达上限 {} 次。",
                    mode, provider_budget
                ),
                "PROVIDER_BUDGET_EXHAUSTED",
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
            AuthScheme::XGoogApiKey,
        )
        .await;

        // Gemini: if header auth fails with AUTH, try query key once
        if plan.protocol == ProtocolKind::GeminiNative
            && !result.ok
            && !result.reused_from_cache
            && matches!(
                result.classification.as_str(),
                "AUTH_INVALID" | "KEY_INVALID" | "AUTH_PERMISSION_DENIED" | "ENDPOINT_NOT_FOUND"
            )
        {
            if result.http_sent {
                provider_real_sends += 1;
            }
            emit(DiagnosisEvent::AttemptFinished {
                run_id: run_id.to_string(),
                opaque_id: provider.opaque_id.clone(),
                index,
                result: result.clone(),
            });
            attempts.push(result);

            if provider_real_sends >= provider_budget {
                break;
            }
            emit(DiagnosisEvent::AttemptStarted {
                run_id: run_id.to_string(),
                opaque_id: provider.opaque_id.clone(),
                index,
                label: "Gemini 认证兼容：Header → Query key".into(),
                url: crate::security::sanitize_url_for_display(&plan.base_url),
                protocol: plan.protocol.as_str().to_string(),
                model: plan.model.clone(),
            });
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
                TokenLimitField::MaxCompletionTokens,
                AuthScheme::QueryKey,
            )
            .await;
            if result.ok {
                result.suggestion_note =
                    Some("Header x-goog-api-key 失败后，Query ?key= 认证成功。".into());
            }
        }

        if plan.protocol == ProtocolKind::OpenAiChat
            && !result.ok
            && !result.reused_from_cache
            && is_max_completion_tokens_unsupported(&result)
        {
            if result.http_sent {
                provider_real_sends += 1;
            }
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

            if provider_real_sends >= provider_budget {
                break;
            }
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
                AuthScheme::XGoogApiKey,
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

        if result.http_sent {
            provider_real_sends += 1;
        }

        if result.classification == "HOST_BUDGET_EXHAUSTED"
            || result.classification == "HOST_RATE_LIMIT_STOPPED"
            || result.classification == "PROVIDER_BUDGET_EXHAUSTED"
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
            // Native direct success on a current-config-equivalent model
            // (ConfiguredModel or LocalMarkerNormalized) sets current_ok.
            // Cross-protocol / loose / DoctorGuess must never do so.
            let counts_as_current = plan.equivalent_to_current
                && plan.model_source.is_current_config_equivalent()
                && !model_is_guessed
                && result.is_native_success()
                && !matches!(
                    result.classification.as_str(),
                    "RESPONSE_PROTOCOL_VARIANT_OK"
                        | "DIRECT_PROTOCOL_VARIANT_OK"
                        | "LOOSE_RESPONSE_TEXT_OK"
                        | "STREAM_PROTOCOL_VARIANT_OK"
                );
            if counts_as_current {
                current_ok = true;
            }
            // Prefer higher-quality success evidence over first-in-time success.
            let take = match success_plan {
                None => true,
                Some(prev) => success_evidence_rank(plan) > success_evidence_rank(prev),
            };
            if take {
                success_plan = Some(plan);
                success_result = Some(result.clone());
            }
            if mode == DiagnosisMode::Smart
                && !plan.stream
                && !plan.tool_call
                && !plan.is_current_config
                && !plan.equivalent_to_current
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
    let model_success_kind = success_plan
        .map(|p| match p.model_source {
            ModelCandidateSource::ConfiguredModel | ModelCandidateSource::LocalMarkerNormalized => {
                ModelSuccessKind::CurrentConfig
            }
            ModelCandidateSource::ConfiguredRoleMapping => ModelSuccessKind::ConfiguredRoleMapping,
            ModelCandidateSource::DoctorGuess => ModelSuccessKind::DoctorGuess,
            ModelCandidateSource::DiscoveredModel => ModelSuccessKind::TrueVariant,
        })
        .unwrap_or_else(|| {
            if model_is_guessed && any_ok && !current_ok {
                ModelSuccessKind::DoctorGuess
            } else if any_ok && !current_ok {
                ModelSuccessKind::TrueVariant
            } else {
                ModelSuccessKind::CurrentConfig
            }
        });

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
        // Prefer best successful classification when any_ok for status mapping
        let success_cls: Vec<&str> = attempts
            .iter()
            .filter(|a| a.ok || a.classification == "LOOSE_RESPONSE_TEXT_OK")
            .map(|a| a.classification.as_str())
            .collect();
        if !success_cls.is_empty() {
            // Prefer native GENERATE_OK over variants
            if success_cls.contains(&"GENERATE_OK")
                || success_cls.contains(&"STREAM_OK")
                || success_cls.contains(&"TOOL_CALLING_OK")
            {
                "GENERATE_OK".into()
            } else if success_cls.contains(&"RESPONSE_PROTOCOL_VARIANT_OK")
                || success_cls.contains(&"DIRECT_PROTOCOL_VARIANT_OK")
            {
                "RESPONSE_PROTOCOL_VARIANT_OK".into()
            } else if success_cls.contains(&"LOOSE_RESPONSE_TEXT_OK") {
                "LOOSE_RESPONSE_TEXT_OK".into()
            } else {
                success_cls.first().unwrap_or(&"UNKNOWN_ERROR").to_string()
            }
        } else if failed.is_empty() {
            "UNKNOWN_ERROR".into()
        } else {
            best_classification(failed)
        }
    };

    let direct_native_ok = current_ok;
    let direct_variant_ok = attempts.iter().any(|a| {
        a.ok && a.channel == DiagnosisChannel::DirectUpstream
            && matches!(
                a.classification.as_str(),
                "RESPONSE_PROTOCOL_VARIANT_OK"
                    | "DIRECT_PROTOCOL_VARIANT_OK"
                    | "PROTOCOL_FALLBACK_OK"
            )
    });
    let direct_failed = attempts
        .iter()
        .any(|a| a.channel == DiagnosisChannel::DirectUpstream && !a.ok && a.http_sent);

    let mut direct_status = if provider.skip_reason.is_some() {
        "MANAGED_AUTH_SKIPPED".into()
    } else {
        final_status_from_attempts(
            current_ok,
            any_ok,
            &best_class,
            protocol_changed,
            url_changed,
            model_success_kind,
            needs_local,
        )
    };
    if model_is_guessed
        && any_ok
        && !current_ok
        && direct_status != "LOCAL_ROUTING_REQUIRED"
        && direct_status != "CONFIGURED_MODEL_MAPPING_OK"
        && (direct_status == "CURRENT_CONFIG_OK"
            || direct_status == "MODEL_VARIANT_OK"
            || direct_status == "AUTH_VARIANT_OK"
            || direct_status == "MODEL_GUESS_OK"
            || (!protocol_changed && !url_changed))
    {
        direct_status = "MODEL_GUESS_OK".into();
    }

    // Route disposition metadata (NotRunning / NotCurrentTarget / Skip / etc.)
    // is retained in route_status for the UI, but MUST NOT become the primary
    // provider outcome unless a real CCS route business request was sent.
    let route_attempted = attempts
        .iter()
        .any(|a| a.channel == DiagnosisChannel::CcsLocalRoute && a.http_sent);
    if route_attempted {
        route_disposition = RouteDisposition::Attempted;
    }

    let primary_status = if provider.skip_reason.is_some() {
        "MANAGED_AUTH_SKIPPED".into()
    } else if route_attempted {
        // Only combine when a real route HTTP request was sent.
        // Target mismatch observed after a real send may still raise CCS_ROUTE_TARGET_MISMATCH.
        combine_attempted_route_and_direct(
            route_ok,
            direct_native_ok,
            direct_variant_ok,
            direct_failed,
            route_target_mismatch,
        )
    } else {
        // NotRequested / NotRunning / NotCurrentTarget / Skip / DirectOnly /
        // BlockedNonLoopback / already-deduped app route → primary = direct.
        direct_status.clone()
    };
    let status = primary_status.clone();

    let direct_attempted = attempts.iter().any(|a| {
        a.channel == DiagnosisChannel::DirectUpstream && (a.http_sent || a.reused_from_cache)
    });
    let best_direct_idx = attempts.iter().enumerate().find_map(|(i, a)| {
        if a.channel == DiagnosisChannel::DirectUpstream && a.ok {
            Some(i)
        } else {
            None
        }
    });
    let direct_summary = DirectChannelSummary {
        attempted: direct_attempted || provider.skip_reason.is_some(),
        status: direct_status.clone(),
        success: any_ok
            && attempts
                .iter()
                .any(|a| a.channel == DiagnosisChannel::DirectUpstream && a.ok),
        native_success: direct_native_ok,
        best_attempt_index: best_direct_idx,
    };

    let route_overall = if route_attempted {
        if route_target_mismatch {
            Some("CCS_ROUTE_TARGET_MISMATCH".into())
        } else if route_ok == Some(true) {
            Some("CCS_ROUTE_OK".into())
        } else if route_ok == Some(false) {
            // Prefer generate failure status when present.
            route_generate
                .as_ref()
                .map(|c| c.status.clone())
                .or_else(|| route_streaming.as_ref().map(|c| c.status.clone()))
                .or_else(|| Some("CCS_ROUTE_FAILED".into()))
        } else {
            None
        }
    } else {
        match route_disposition {
            RouteDisposition::NotRunning => Some("CCS_ROUTE_NOT_RUNNING".into()),
            RouteDisposition::NotCurrentTarget => Some("CCS_ROUTE_NOT_APPLICABLE".into()),
            RouteDisposition::NotConfigured
            | RouteDisposition::UnsupportedApp
            | RouteDisposition::BlockedNonLoopback => Some("CCS_ROUTE_NOT_APPLICABLE".into()),
            RouteDisposition::NotRequested => None,
            RouteDisposition::Attempted => None,
        }
    };

    let mut route_summary = RouteChannelSummary {
        disposition: route_disposition,
        attempted: route_attempted,
        generate: route_generate,
        streaming: route_streaming,
        overall_status: route_overall.clone(),
        actual_provider_id: route_actual_provider_id,
        actual_provider_name: route_actual_provider_name,
        failover_count_before,
        failover_count_after,
        notice: route_notice.clone(),
    };
    let route_status_str = route_summary
        .legacy_route_status_code()
        .or(route_overall.clone());
    // Keep overall_status populated for Attempted path.
    if route_summary.overall_status.is_none() {
        route_summary.overall_status = route_status_str.clone();
    }

    let mut suggestion = build_suggestion(
        &provider,
        current_ok,
        any_ok,
        success_plan,
        needs_local,
        &status,
    );
    if status == "CCS_ROUTE_OK_DIRECT_VARIANT"
        || status == "CCS_ROUTE_OK_DIRECT_PARSE_FAILED"
        || status == "CCS_ROUTE_OK"
        || status == "CCS_ROUTE_OK_DIRECT_NATIVE_OK"
    {
        suggestion = "无需修改当前 CC Switch 配置。上游协议与客户端协议可能不同，当前由 CCS 路由完成转换；结果表示当前 CCS 路由链可用。".into();
        if let Some(n) = &route_notice {
            suggestion = format!("{suggestion} {n}");
        }
    } else if status == "CCS_ROUTE_TARGET_MISMATCH" {
        suggestion = "CCS 路由请求成功，但实际由另一 Provider 处理；本结果验证的是当前路由链，不代表所选 Provider 已通过。".into();
    } else if status == "CCS_ROUTE_FAILED_DIRECT_OK" {
        suggestion = "上游直连可用，但当前 CCS 路由链请求失败。请检查 CCS 路由是否运行、目标 Provider 与映射。".into();
    } else if status == "CCS_ROUTE_AND_DIRECT_FAILED" {
        suggestion = "CCS 路由与上游直连均失败。请先查看直连错误与路由尝试链。".into();
    } else if !route_attempted {
        // Route disposition is auxiliary only — never rewrite the direct-based suggestion.
        match route_disposition {
            RouteDisposition::NotRunning => {
                suggestion =
                    format!("{suggestion} （辅助：CCS 路由已配置但未运行，本次未执行路由验证。）");
            }
            RouteDisposition::NotCurrentTarget => {
                suggestion = format!(
                    "{suggestion} （辅助：CCS 路由未验证——该 Provider 不是当前 CCS 路由目标。）"
                );
            }
            RouteDisposition::NotRequested => {
                if verify_mode == VerifyMode::DirectOnly {
                    // DirectOnly: silent, no auxiliary noise.
                }
            }
            RouteDisposition::NotConfigured
            | RouteDisposition::UnsupportedApp
            | RouteDisposition::BlockedNonLoopback => {
                suggestion = format!(
                    "{suggestion} （辅助：CCS 路由未验证——配置不可用、非 loopback 或应用不支持。）"
                );
            }
            RouteDisposition::Attempted => {}
        }
    }
    if model_is_guessed && any_ok {
        suggestion = format!(
            "{suggestion} 使用 Doctor 推测模型测试成功，但不能证明 CC Switch 当前模型配置可用。"
        );
    }
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
            if a.http_sent {
                line.push_str(" [真实发送]");
            }
            if let Some(TokenLimitField::MaxTokens) = a.token_limit_field {
                line.push_str(" [max_tokens]");
            }
            if let Some(note) = &a.suggestion_note {
                line.push_str(&format!(" — {note}"));
            }
            if !a.error_evidence.is_empty() {
                let bits: Vec<String> = a
                    .error_evidence
                    .iter()
                    .map(|e| {
                        let mut s = e.source.clone();
                        if let Some(c) = &e.code {
                            s.push_str(&format!(" code={c}"));
                        }
                        if let Some(k) = &e.matched_keyword {
                            s.push_str(&format!(" kw={k}"));
                        }
                        if let Some(m) = &e.message {
                            s.push_str(&format!(" msg={}", m.chars().take(80).collect::<String>()));
                        }
                        s
                    })
                    .collect();
                line.push_str(&format!(" | 依据: {}", bits.join("; ")));
            }
            line
        })
        .collect();

    let mut confidence = if current_ok {
        "high"
    } else if any_ok {
        "medium"
    } else if attempts.iter().any(|a| a.status_code == Some(401)) {
        "high"
    } else {
        "low"
    }
    .to_string();
    // After /status refresh failed: keep business result but lower confidence.
    if after_status_unconfirmed && confidence == "high" {
        confidence = "medium".into();
    }

    let summary = ProviderDiagnosisSummary {
        opaque_id: provider.opaque_id.clone(),
        source_id: provider.source_id.clone(),
        display_name: provider.display_name.clone(),
        app_label: provider.app_type.label_zh().to_string(),
        status: status.clone(),
        primary_outcome: primary_status,
        current_config_ok: current_ok,
        any_success: any_ok,
        safe_base_url: provider.safe_base_url.clone(),
        configured_protocol: provider.configured_protocol.map(|p| p.label().to_string()),
        configured_model: provider.configured_model.clone(),
        outbound_model: success_plan.map(|p| p.model.clone()).or_else(|| {
            success_result
                .as_ref()
                .and_then(|r| r.outbound_model.clone())
        }),
        model_transform: success_plan.and_then(|p| {
            p.model_candidate()
                .transform_label()
                .map(|s| s.to_string())
                .or_else(|| {
                    if p.display_model != p.model {
                        Some(format!("{} → {}", p.display_model, p.model))
                    } else {
                        None
                    }
                })
        }),
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
        direct: Some(direct_summary),
        route: Some(route_summary),
        route_status: route_status_str,
        direct_status: Some(direct_status),
        route_side_effect_notice: route_notice,
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
    gemini_auth: AuthScheme,
) -> AttemptResult {
    let safe_url = crate::security::redact::sanitize_url_with_redactor(&plan.base_url, redactor);

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
            channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
            response_compatibility: None,
            requested_protocol: None,
            matched_protocol: None,
            configured_model_display: None,
            outbound_model: None,
            model_transform: None,
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
        ProtocolKind::GeminiNative => gemini_auth,
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
        ProtocolKind::GeminiNative => build_gemini_request_with_auth(
            &plan.base_url,
            &plan.model,
            key,
            plan.stream,
            plan.tool_call,
            ua,
            auth_scheme,
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
                channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                response_compatibility: None,
                requested_protocol: None,
                matched_protocol: None,
                configured_model_display: None,
                outbound_model: None,
                model_transform: None,
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
                channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                response_compatibility: None,
                requested_protocol: None,
                matched_protocol: None,
                configured_model_display: None,
                outbound_model: None,
                model_transform: None,
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
            channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
            response_compatibility: None,
            requested_protocol: None,
            matched_protocol: None,
            configured_model_display: None,
            outbound_model: None,
            model_transform: None,
        };
        session_budget.finish_flight(&cache_key, r.clone());
        return r;
    }

    let timeout = default_timeout(mode == DiagnosisMode::Deep || plan.stream);
    let mut result = exec
        .execute(built, origin_policy, redactor, cancel, timeout)
        .await;
    result.token_limit_field = token_for_key;
    // Always record configured vs outbound model semantics on the attempt.
    result.configured_model_display = Some(plan.display_model.clone());
    result.outbound_model = Some(plan.model.clone());
    result.model_transform = plan
        .model_candidate()
        .transform_label()
        .map(|s| s.to_string());
    if result.model_transform.is_none() && plan.display_model != plan.model {
        result.model_transform = Some(format!("{} → {}", plan.display_model, plan.model));
    }

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
        if let Some(s) = success {
            if s.model_source == ModelCandidateSource::LocalMarkerNormalized
                || s.display_model != s.model
            {
                return format!(
                    "当前配置可用。`[1M]` 是 Claude/CC Switch 的本地上下文能力标记，发送上游时已按 CC Switch 规则使用 `{}`（配置值：{}）。无需更换模型或修改配置。本工具未修改任何配置。",
                    s.model,
                    s.display_model
                );
            }
        }
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
            match s.model_source {
                ModelCandidateSource::ConfiguredRoleMapping => {
                    return format!(
                        "当前 Provider 配置中的模型映射可用。成功模型：{}（来自当前 CC Switch Provider 的角色模型映射）。无需修改配置。本工具未自动修改任何配置。",
                        s.model
                    );
                }
                ModelCandidateSource::DoctorGuess => {
                    return format!(
                        "使用 Doctor 推测模型 `{}` 测试成功，但不能证明 CC Switch 当前模型配置可用。本工具未自动修改任何配置。",
                        s.model
                    );
                }
                ModelCandidateSource::DiscoveredModel
                | ModelCandidateSource::ConfiguredModel
                | ModelCandidateSource::LocalMarkerNormalized => {
                    if !s.equivalent_to_current {
                        let current = provider.configured_model.as_deref().unwrap_or("—");
                        return format!(
                            "当前模型不可用，其他模型可用。当前模型：{}；成功模型：{}。建议在 CC Switch 中确认模型名。本工具未自动修改任何配置。",
                            current, s.model
                        );
                    }
                }
            }
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
        "KEY_INVALID" | "AUTH_INVALID" => {
            "鉴权失败：API Key 无效或未授权。请在 CC Switch 中更新 Key。本工具未修改配置。".into()
        }
        "QUOTA_EXHAUSTED" => "额度不足或配额耗尽。请检查供应商余额。本工具未修改配置。".into(),
        "RATE_LIMITED" | "HOST_RATE_LIMIT_STOPPED" => {
            "触发限流（429）。请稍后重试或降低并发。本工具未修改配置。".into()
        }
        "HOST_BUDGET_EXHAUSTED" => {
            "该 Host 在本次诊断会话中已达到 30 次请求上限。本工具未修改配置。".into()
        }
        "MODEL_NOT_FOUND" => {
            "当前模型名或当前分组没有可用渠道。请检查模型名映射与分组。本工具未修改配置。".into()
        }
        "ENDPOINT_NOT_FOUND" => {
            "端点不存在（404）。可尝试在智能诊断中修正 /v1 或协议。本工具未修改配置。".into()
        }
        "NETWORK_UNREACHABLE" => "网络不可达。请检查代理、DNS 与防火墙。本工具未修改配置。".into(),
        "TLS_ERROR" => {
            "TLS/证书错误。请通过系统信任链修复证书，不要关闭校验。本工具未修改配置。".into()
        }
        "MANAGED_AUTH_SKIPPED" => "托管登录/OAuth 配置已安全跳过。".into(),
        "CONFIGURED_MODEL_MAPPING_OK" => {
            "当前 Provider 配置中的模型映射可用。无需修改配置。本工具未修改任何配置。".into()
        }
        _ => format!("诊断状态：{status}。请查看尝试链与错误摘要。本工具未修改任何配置。"),
    }
}
