use super::anthropic::{extract_anthropic_stream_delta, extract_anthropic_tool_use};
use super::gemini::{extract_gemini_text, extract_gemini_tool_call};
use super::openai_chat::{extract_chat_stream_delta, extract_chat_tool_call};
use super::openai_responses::{extract_responses_stream_event, extract_responses_tool_call};
use super::types::{
    evaluate_text, redact_result, AttemptResult, BuiltRequest, RequestPurpose, MAX_BODY_BYTES,
    MAX_ERROR_BYTES,
};
use crate::ccs_adapter::ProtocolKind;
use crate::diagnostics::classifier::{classify_structured_error_envelope, classify_with_evidence};
use crate::security::origin::SameOriginPolicy;
use crate::security::redact::{sanitize_url_for_display, truncate_utf8, SecretRedactor};
use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client, StatusCode};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub struct HttpExecutor {
    client: Client,
}

impl HttpExecutor {
    pub fn new() -> Result<Self, String> {
        // Manual redirects only — we enforce same-origin and never forward credentials cross-host.
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .user_agent(format!(
                "CC-Switch-Doctor/{} (+https://github.com/Super-YYQ/cc-switch-doctor)",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    pub async fn execute(
        &self,
        req: BuiltRequest,
        origin: &SameOriginPolicy,
        redactor: &SecretRedactor,
        cancel: &CancellationToken,
        timeout: Duration,
    ) -> AttemptResult {
        let started = Instant::now();
        let safe_url = sanitize_url_for_display(&req.url);

        if cancel.is_cancelled() {
            return AttemptResult {
                ok: false,
                partial: false,
                status_code: None,
                latency_ms: 0,
                ttft_ms: None,
                protocol: req.protocol,
                model: req.model.clone(),
                url: safe_url,
                stream: req.stream,
                purpose: req.purpose,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("cancelled".into()),
                error_message: Some("已取消".into()),
                response_excerpt: None,
                classification: "CANCELLED".into(),
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

        // Origin check before send
        if let Ok(u) = url::Url::parse(&req.url) {
            if !origin.allows(&u) {
                return redact_result(
                    AttemptResult {
                        ok: false,
                        partial: false,
                        status_code: None,
                        latency_ms: started.elapsed().as_millis() as u64,
                        ttft_ms: None,
                        protocol: req.protocol,
                        model: req.model.clone(),
                        url: safe_url,
                        stream: req.stream,
                        purpose: req.purpose,
                        extracted_text: None,
                        tool_call_ok: None,
                        error_kind: Some("security".into()),
                        error_message: Some("跨 Host URL 被阻断，未发送凭据".into()),
                        response_excerpt: None,
                        classification: "CROSS_ORIGIN_REDIRECT_BLOCKED".into(),
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
                    },
                    redactor,
                );
            }
        }

        let mut builder = self
            .client
            .request(
                req.method.parse().unwrap_or(reqwest::Method::POST),
                &req.url,
            )
            .timeout(timeout);

        for (k, v) in &req.headers {
            builder = builder.header(k, v);
        }
        if let Some(body) = &req.body {
            builder = builder.json(body);
        }

        let send_fut = builder.send();
        let response = tokio::select! {
            _ = cancel.cancelled() => {
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: None,
                    latency_ms: started.elapsed().as_millis() as u64,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: req.stream,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("cancelled".into()),
                    error_message: Some("已取消".into()),
                    response_excerpt: None,
                    classification: "CANCELLED".into(),
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
            res = send_fut => res,
        };

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                let msg = redactor.redact(&e.to_string());
                let classification = if e.is_timeout() {
                    "TIMEOUT"
                } else if e.is_connect() {
                    "NETWORK_UNREACHABLE"
                } else if format!("{e}").to_ascii_lowercase().contains("tls")
                    || format!("{e}").to_ascii_lowercase().contains("certificate")
                {
                    "TLS_ERROR"
                } else {
                    "NETWORK_UNREACHABLE"
                };
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: None,
                    latency_ms: started.elapsed().as_millis() as u64,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: req.stream,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("network".into()),
                    error_message: Some(msg),
                    response_excerpt: None,
                    classification: classification.into(),
                    http_sent: true,
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

        // Handle redirects manually
        let status = response.status();
        if status.is_redirection() {
            let loc = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let blocked = if let Ok(next) = response.url().join(loc) {
                !origin.allows(&next)
            } else {
                true
            };
            return AttemptResult {
                ok: false,
                partial: false,
                status_code: Some(status.as_u16()),
                latency_ms: started.elapsed().as_millis() as u64,
                ttft_ms: None,
                protocol: req.protocol,
                model: req.model.clone(),
                url: safe_url,
                stream: req.stream,
                purpose: req.purpose,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("redirect".into()),
                error_message: Some(if blocked {
                    "跨 Host 重定向已阻断（Location 已脱敏），未携带凭据继续请求".into()
                } else {
                    "同源重定向未自动跟随（安全策略）".into()
                }),
                response_excerpt: None,
                classification: if blocked {
                    "CROSS_ORIGIN_REDIRECT_BLOCKED".into()
                } else {
                    "ENDPOINT_NOT_FOUND".into()
                },
                http_sent: true,
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

        if req.stream {
            return self
                .read_stream(response, req, safe_url, started, redactor, cancel)
                .await;
        }

        // Capture headers before consuming body
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok());

        if let Some(cl) = content_length {
            if cl > MAX_BODY_BYTES {
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: Some(status.as_u16()),
                    latency_ms: started.elapsed().as_millis() as u64,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("body".into()),
                    error_message: Some("响应体 Content-Length 超过 2MB 限制".into()),
                    response_excerpt: None,
                    classification: "UNKNOWN_ERROR".into(),
                    http_sent: true,
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
        }

        let status_code = status.as_u16();
        let bytes = match read_body_bounded(response, MAX_BODY_BYTES, cancel).await {
            Ok(BodyRead::Bytes(b)) => b,
            Ok(BodyRead::TooLarge) => {
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: Some(status_code),
                    latency_ms: started.elapsed().as_millis() as u64,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("body".into()),
                    error_message: Some("响应体超过 2MB 限制（增量读取已中止）".into()),
                    response_excerpt: None,
                    classification: "UNKNOWN_ERROR".into(),
                    http_sent: true,
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
            Ok(BodyRead::Cancelled) => {
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: Some(status_code),
                    latency_ms: started.elapsed().as_millis() as u64,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("cancelled".into()),
                    error_message: Some("已取消".into()),
                    response_excerpt: None,
                    classification: "CANCELLED".into(),
                    http_sent: true,
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
            Err(e) => {
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: Some(status_code),
                    latency_ms: started.elapsed().as_millis() as u64,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("network".into()),
                    error_message: Some(redactor.redact(&e)),
                    response_excerpt: None,
                    classification: "NETWORK_UNREACHABLE".into(),
                    http_sent: true,
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

        let body_text = String::from_utf8_lossy(&bytes).to_string();
        let latency_ms = started.elapsed().as_millis() as u64;
        let ct_ref = content_type.as_deref();

        if !status.is_success() {
            let excerpt = truncate(&redactor.redact(&body_text), MAX_ERROR_BYTES);
            let (classification, ev) = classify_with_evidence(status_code, &body_text, ct_ref);
            return AttemptResult {
                ok: false,
                partial: false,
                status_code: Some(status_code),
                latency_ms,
                ttft_ms: None,
                protocol: req.protocol,
                model: req.model.clone(),
                url: safe_url,
                stream: false,
                purpose: req.purpose,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("http".into()),
                error_message: Some(excerpt.clone()),
                response_excerpt: Some(excerpt),
                classification,
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
                error_evidence: ev,
                channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                response_compatibility: None,
                requested_protocol: None,
                matched_protocol: None,
                configured_model_display: None,
                outbound_model: None,
                model_transform: None,
            };
        }

        // HTML on 2xx → WAF before JSON parse
        if let Some(ct) = ct_ref {
            if ct.to_ascii_lowercase().contains("text/html") {
                let (cls, ev) = classify_with_evidence(status_code, &body_text, ct_ref);
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: Some(status_code),
                    latency_ms,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("waf".into()),
                    error_message: Some("响应 Content-Type 为 HTML，疑似网关/WAF".into()),
                    response_excerpt: Some(truncate(&redactor.redact(&body_text), 512)),
                    classification: if cls == "UNKNOWN_ERROR" {
                        "GATEWAY_OR_WAF".into()
                    } else {
                        cls
                    },
                    http_sent: true,
                    reused_from_cache: false,
                    suggestion_note: None,
                    token_limit_field: None,
                    error_evidence: ev,
                    channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                    response_compatibility: None,
                    requested_protocol: None,
                    matched_protocol: None,
                    configured_model_display: None,
                    outbound_model: None,
                    model_transform: None,
                };
            }
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&body_text).unwrap_or(serde_json::Value::Null);

        // Only structured error envelopes may short-circuit before success parsing.
        // Free-text keyword heuristics must NEVER override a valid protocol success body
        // (e.g. billing_usage in Anthropic success JSON must not become QUOTA_EXHAUSTED).
        if let Some((cls, ev)) =
            classify_structured_error_envelope(status_code, &body_text, &parsed)
        {
            if matches!(
                cls.as_str(),
                "AUTH_INVALID"
                    | "AUTH_PERMISSION_DENIED"
                    | "QUOTA_EXHAUSTED"
                    | "RATE_LIMITED"
                    | "GATEWAY_OR_WAF"
                    | "MODEL_NOT_FOUND"
                    | "UNKNOWN_ERROR"
            ) {
                let excerpt = truncate(&redactor.redact(&body_text), 512);
                return AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: Some(status_code),
                    latency_ms,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("structured_error".into()),
                    error_message: Some(excerpt.clone()),
                    response_excerpt: Some(excerpt),
                    classification: cls,
                    http_sent: true,
                    reused_from_cache: false,
                    suggestion_note: None,
                    token_limit_field: None,
                    error_evidence: ev,
                    channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                    response_compatibility: None,
                    requested_protocol: None,
                    matched_protocol: None,
                    configured_model_display: None,
                    outbound_model: None,
                    model_transform: None,
                };
            }
        }

        if req.purpose == RequestPurpose::ToolCall {
            let tool = match req.protocol {
                ProtocolKind::OpenAiChat => extract_chat_tool_call(&parsed),
                ProtocolKind::OpenAiResponses => extract_responses_tool_call(&parsed),
                ProtocolKind::AnthropicMessages => extract_anthropic_tool_use(&parsed),
                ProtocolKind::GeminiNative => extract_gemini_tool_call(&parsed),
                ProtocolKind::Unknown => None,
            };
            let tool = tool.or_else(|| {
                extract_chat_tool_call(&parsed)
                    .or_else(|| extract_responses_tool_call(&parsed))
                    .or_else(|| extract_anthropic_tool_use(&parsed))
                    .or_else(|| extract_gemini_tool_call(&parsed))
            });
            let tool_ok = tool
                .as_ref()
                .map(|(name, args)| name == "ccs_doctor_echo" && args.contains("ok"))
                .unwrap_or(false);
            return AttemptResult {
                ok: tool_ok,
                partial: tool.is_some() && !tool_ok,
                status_code: Some(status_code),
                latency_ms,
                ttft_ms: None,
                protocol: req.protocol,
                model: req.model.clone(),
                url: safe_url,
                stream: false,
                purpose: req.purpose,
                extracted_text: tool.map(|(_, a)| a),
                tool_call_ok: Some(tool_ok),
                error_kind: if tool_ok { None } else { Some("tool".into()) },
                error_message: if tool_ok {
                    None
                } else {
                    Some("未检测到有效的 ccs_doctor_echo 工具调用".into())
                },
                response_excerpt: None,
                classification: if tool_ok {
                    "TOOL_CALLING_OK".into()
                } else {
                    "TOOL_CALLING_UNSUPPORTED".into()
                },
                http_sent: true,
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

        match super::parse::extract_response_text(req.protocol, &parsed) {
            Some(parsed_text) => {
                let t = parsed_text.text;
                let has_text = evaluate_text(&t).0;
                use crate::protocols::types::ResponseCompatibility;
                let compat = if parsed_text.loose_field {
                    ResponseCompatibility::LooseField
                } else if parsed_text.cross_protocol {
                    ResponseCompatibility::CrossProtocol
                } else {
                    ResponseCompatibility::Native
                };
                // LooseField never counts as full success (ok=true).
                // Native / CrossProtocol: non-empty text is success without product markers.
                let (ok, partial, classification) = match compat {
                    ResponseCompatibility::Native => {
                        if has_text {
                            (true, false, "GENERATE_OK".into())
                        } else {
                            (false, false, "UNKNOWN_ERROR".into())
                        }
                    }
                    ResponseCompatibility::CrossProtocol => {
                        if has_text {
                            (true, false, "RESPONSE_PROTOCOL_VARIANT_OK".into())
                        } else {
                            (false, false, "UNKNOWN_ERROR".into())
                        }
                    }
                    ResponseCompatibility::LooseField => {
                        if has_text {
                            (false, true, "LOOSE_RESPONSE_TEXT_OK".into())
                        } else {
                            (false, false, "UNKNOWN_ERROR".into())
                        }
                    }
                };
                let suggestion_note = if parsed_text.cross_protocol {
                    Some(format!(
                        "返回了有效文本，但响应结构属于 {}，不是配置的 {}。",
                        super::parse::protocol_label(parsed_text.matched_protocol),
                        super::parse::protocol_label(req.protocol)
                    ))
                } else if parsed_text.loose_field {
                    Some("从兼容字段提取到文本（宽松解析，不能证明当前配置协议兼容）".into())
                } else if ok {
                    Some("HTTP 2xx + 原生协议结构 + 非空生成文本".into())
                } else {
                    None
                };
                AttemptResult {
                    ok,
                    partial,
                    status_code: Some(status_code),
                    latency_ms,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: Some(redactor.redact(&t)),
                    tool_call_ok: None,
                    error_kind: if ok || partial {
                        None
                    } else {
                        Some("empty".into())
                    },
                    error_message: if ok {
                        None
                    } else if partial {
                        Some("宽松字段解析到文本，但不能证明当前协议配置可用".into())
                    } else {
                        Some("响应结构成功但无文本".into())
                    },
                    response_excerpt: None,
                    classification,
                    http_sent: true,
                    reused_from_cache: false,
                    suggestion_note,
                    token_limit_field: None,
                    error_evidence: vec![],
                    channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                    response_compatibility: Some(compat),
                    requested_protocol: Some(req.protocol),
                    matched_protocol: Some(parsed_text.matched_protocol),
                    configured_model_display: None,
                    outbound_model: None,
                    model_transform: None,
                }
            }
            None => {
                let (cls, ev) = classify_with_evidence(status_code, &body_text, ct_ref);
                AttemptResult {
                    ok: false,
                    partial: false,
                    status_code: Some(status_code),
                    latency_ms,
                    ttft_ms: None,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: false,
                    purpose: req.purpose,
                    extracted_text: None,
                    tool_call_ok: None,
                    error_kind: Some("parse".into()),
                    error_message: Some("HTTP 成功但无法按协议解析文本".into()),
                    response_excerpt: Some(truncate(&redactor.redact(&body_text), 512)),
                    classification: if cls != "UNKNOWN_ERROR" {
                        cls
                    } else {
                        "RESPONSE_FORMAT_MISMATCH".into()
                    },
                    http_sent: true,
                    reused_from_cache: false,
                    suggestion_note: None,
                    token_limit_field: None,
                    error_evidence: ev,
                    channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                    response_compatibility: None,
                    requested_protocol: None,
                    matched_protocol: None,
                    configured_model_display: None,
                    outbound_model: None,
                    model_transform: None,
                }
            }
        }
    }

    async fn read_stream(
        &self,
        response: reqwest::Response,
        req: BuiltRequest,
        safe_url: String,
        started: Instant,
        redactor: &SecretRedactor,
        cancel: &CancellationToken,
    ) -> AttemptResult {
        let status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok());
        let ct_ref = content_type.as_deref();

        if !response.status().is_success() {
            // Bounded read for non-2xx stream bodies (never response.text())
            if let Some(cl) = content_length {
                if cl > MAX_BODY_BYTES {
                    return AttemptResult {
                        ok: false,
                        partial: false,
                        status_code: Some(status_code),
                        latency_ms: started.elapsed().as_millis() as u64,
                        ttft_ms: None,
                        protocol: req.protocol,
                        model: req.model.clone(),
                        url: safe_url,
                        stream: true,
                        purpose: req.purpose,
                        extracted_text: None,
                        tool_call_ok: None,
                        error_kind: Some("body".into()),
                        error_message: Some("错误响应体 Content-Length 超过 2MB 限制".into()),
                        response_excerpt: None,
                        classification: "RESPONSE_BODY_TOO_LARGE".into(),
                        http_sent: true,
                        reused_from_cache: false,
                        suggestion_note: None,
                        token_limit_field: None,
                        error_evidence: vec![],
                        channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                        response_compatibility: None,
                        requested_protocol: Some(req.protocol),
                        matched_protocol: None,
                        configured_model_display: None,
                        outbound_model: None,
                        model_transform: None,
                    };
                }
            }
            let body_bytes = match read_body_bounded(response, MAX_BODY_BYTES, cancel).await {
                Ok(BodyRead::Bytes(b)) => b,
                Ok(BodyRead::TooLarge) => {
                    return AttemptResult {
                        ok: false,
                        partial: false,
                        status_code: Some(status_code),
                        latency_ms: started.elapsed().as_millis() as u64,
                        ttft_ms: None,
                        protocol: req.protocol,
                        model: req.model.clone(),
                        url: safe_url,
                        stream: true,
                        purpose: req.purpose,
                        extracted_text: None,
                        tool_call_ok: None,
                        error_kind: Some("body".into()),
                        error_message: Some("错误响应体超过 2MB 限制（增量读取已中止）".into()),
                        response_excerpt: None,
                        classification: "RESPONSE_BODY_TOO_LARGE".into(),
                        http_sent: true,
                        reused_from_cache: false,
                        suggestion_note: None,
                        token_limit_field: None,
                        error_evidence: vec![],
                        channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                        response_compatibility: None,
                        requested_protocol: Some(req.protocol),
                        matched_protocol: None,
                        configured_model_display: None,
                        outbound_model: None,
                        model_transform: None,
                    };
                }
                Ok(BodyRead::Cancelled) => {
                    return AttemptResult {
                        ok: false,
                        partial: false,
                        status_code: Some(status_code),
                        latency_ms: started.elapsed().as_millis() as u64,
                        ttft_ms: None,
                        protocol: req.protocol,
                        model: req.model.clone(),
                        url: safe_url,
                        stream: true,
                        purpose: req.purpose,
                        extracted_text: None,
                        tool_call_ok: None,
                        error_kind: Some("cancelled".into()),
                        error_message: Some("已取消".into()),
                        response_excerpt: None,
                        classification: "CANCELLED".into(),
                        http_sent: true,
                        reused_from_cache: false,
                        suggestion_note: None,
                        token_limit_field: None,
                        error_evidence: vec![],
                        channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                        response_compatibility: None,
                        requested_protocol: Some(req.protocol),
                        matched_protocol: None,
                        configured_model_display: None,
                        outbound_model: None,
                        model_transform: None,
                    };
                }
                Err(e) => {
                    return AttemptResult {
                        ok: false,
                        partial: false,
                        status_code: Some(status_code),
                        latency_ms: started.elapsed().as_millis() as u64,
                        ttft_ms: None,
                        protocol: req.protocol,
                        model: req.model.clone(),
                        url: safe_url,
                        stream: true,
                        purpose: req.purpose,
                        extracted_text: None,
                        tool_call_ok: None,
                        error_kind: Some("network".into()),
                        error_message: Some(redactor.redact(&e)),
                        response_excerpt: None,
                        classification: "NETWORK_UNREACHABLE".into(),
                        http_sent: true,
                        reused_from_cache: false,
                        suggestion_note: None,
                        token_limit_field: None,
                        error_evidence: vec![],
                        channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                        response_compatibility: None,
                        requested_protocol: Some(req.protocol),
                        matched_protocol: None,
                        configured_model_display: None,
                        outbound_model: None,
                        model_transform: None,
                    };
                }
            };
            let body = String::from_utf8_lossy(&body_bytes).to_string();
            let excerpt = truncate(&redactor.redact(&body), MAX_ERROR_BYTES);
            let (classification, ev) = classify_with_evidence(status_code, &body, ct_ref);
            return AttemptResult {
                ok: false,
                partial: false,
                status_code: Some(status_code),
                latency_ms: started.elapsed().as_millis() as u64,
                ttft_ms: None,
                protocol: req.protocol,
                model: req.model.clone(),
                url: safe_url,
                stream: true,
                purpose: req.purpose,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("http".into()),
                error_message: Some(excerpt.clone()),
                response_excerpt: Some(excerpt),
                classification,
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
                error_evidence: ev,
                channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                response_compatibility: None,
                requested_protocol: Some(req.protocol),
                matched_protocol: None,
                configured_model_display: None,
                outbound_model: None,
                model_transform: None,
            };
        }

        let mut stream = response.bytes_stream();
        let mut line_buffer = String::new();
        // Full bounded buffer for final fallback parse (max 2MB)
        let mut raw_bounded_buffer = String::new();
        let mut text = String::new();
        let mut ttft: Option<u64> = None;
        let mut total_bytes = 0usize;
        let mut saw_done = false;
        let mut body_too_large = false;
        let mut stream_matched_protocol: Option<ProtocolKind> = None;
        let mut stream_cross = false;

        loop {
            if cancel.is_cancelled() {
                return AttemptResult {
                    ok: false,
                    partial: !text.is_empty(),
                    status_code: Some(status_code),
                    latency_ms: started.elapsed().as_millis() as u64,
                    ttft_ms: ttft,
                    protocol: req.protocol,
                    model: req.model.clone(),
                    url: safe_url,
                    stream: true,
                    purpose: req.purpose,
                    extracted_text: if text.is_empty() {
                        None
                    } else {
                        Some(redactor.redact(&text))
                    },
                    tool_call_ok: None,
                    error_kind: Some("cancelled".into()),
                    error_message: Some("已取消".into()),
                    response_excerpt: None,
                    classification: "CANCELLED".into(),
                    http_sent: true,
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

            let next = tokio::select! {
                _ = cancel.cancelled() => None,
                item = stream.next() => item,
            };

            let Some(item) = next else { break };
            let chunk = match item {
                Ok(c) => c,
                Err(e) => {
                    return AttemptResult {
                        ok: false,
                        partial: !text.is_empty(),
                        status_code: Some(status_code),
                        latency_ms: started.elapsed().as_millis() as u64,
                        ttft_ms: ttft,
                        protocol: req.protocol,
                        model: req.model.clone(),
                        url: safe_url,
                        stream: true,
                        purpose: req.purpose,
                        extracted_text: if text.is_empty() {
                            None
                        } else {
                            Some(redactor.redact(&text))
                        },
                        tool_call_ok: None,
                        error_kind: Some("stream".into()),
                        error_message: Some(redactor.redact(&e.to_string())),
                        response_excerpt: None,
                        classification: "STREAMING_UNSUPPORTED".into(),
                        http_sent: true,
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
            total_bytes += chunk.len();
            if total_bytes > MAX_BODY_BYTES {
                body_too_large = true;
                break;
            }
            let chunk_str = String::from_utf8_lossy(&chunk);
            if raw_bounded_buffer.len() < MAX_BODY_BYTES {
                let remain = MAX_BODY_BYTES - raw_bounded_buffer.len();
                raw_bounded_buffer.push_str(&chunk_str[..chunk_str.len().min(remain)]);
            }
            line_buffer.push_str(&chunk_str);

            while let Some(pos) = line_buffer.find('\n') {
                let mut line = line_buffer[..pos].to_string();
                line_buffer = line_buffer[pos + 1..].to_string();
                if line.ends_with('\r') {
                    line.pop();
                }
                if line.is_empty() {
                    continue;
                }
                // Accept both SSE `data:` and bare NDJSON lines
                let data = line
                    .strip_prefix("data:")
                    .map(|s| s.trim())
                    .unwrap_or(line.trim());
                if data.is_empty() {
                    continue;
                }
                if data == "[DONE]" {
                    saw_done = true;
                    continue;
                }
                let (delta, matched, cross) = extract_stream_delta_layered(req.protocol, data);
                if let Some(d) = delta {
                    if ttft.is_none() {
                        ttft = Some(started.elapsed().as_millis() as u64);
                    }
                    text.push_str(&d);
                    if stream_matched_protocol.is_none() {
                        stream_matched_protocol = matched;
                        stream_cross = cross;
                    }
                }
            }
            if saw_done {
                break;
            }
        }

        let latency_ms = started.elapsed().as_millis() as u64;
        if body_too_large && text.is_empty() {
            return AttemptResult {
                ok: false,
                partial: false,
                status_code: Some(status_code),
                latency_ms,
                ttft_ms: ttft,
                protocol: req.protocol,
                model: req.model.clone(),
                url: safe_url,
                stream: true,
                purpose: req.purpose,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("body".into()),
                error_message: Some("流式响应体超过 2MB 限制".into()),
                response_excerpt: None,
                classification: "RESPONSE_BODY_TOO_LARGE".into(),
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
                error_evidence: vec![],
                channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                response_compatibility: None,
                requested_protocol: Some(req.protocol),
                matched_protocol: None,
                configured_model_display: None,
                outbound_model: None,
                model_transform: None,
            };
        }
        if text.is_empty() {
            // Fallback using the full bounded buffer (SSE ignored stream=true / NDJSON / full JSON)
            let full = raw_bounded_buffer.trim();
            if !full.is_empty() {
                let mut recovered = String::new();
                let mut matched = None;
                let mut cross = false;
                for line in full.lines() {
                    let line = line.trim();
                    if line.is_empty() || line == "[DONE]" {
                        continue;
                    }
                    let data = line.strip_prefix("data:").map(|s| s.trim()).unwrap_or(line);
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(p) = super::parse::extract_response_text(req.protocol, &v) {
                            recovered.push_str(&p.text);
                            matched = Some(p.matched_protocol);
                            cross = p.cross_protocol;
                            continue;
                        }
                    }
                    let (delta, m, c) = extract_stream_delta_layered(req.protocol, data);
                    if let Some(d) = delta {
                        recovered.push_str(&d);
                        matched = m;
                        cross = c;
                    }
                }
                if recovered.is_empty() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(full) {
                        if let Some(p) = super::parse::extract_response_text(req.protocol, &v) {
                            recovered = p.text;
                            matched = Some(p.matched_protocol);
                            cross = p.cross_protocol;
                        }
                    }
                }
                if !recovered.is_empty() {
                    let (ok, partial) = evaluate_text(&recovered);
                    let classification = if ok {
                        if cross {
                            "STREAM_PROTOCOL_VARIANT_OK".into()
                        } else {
                            "STREAM_OK".into()
                        }
                    } else {
                        "STREAMING_UNSUPPORTED".into()
                    };
                    return AttemptResult {
                        ok,
                        partial,
                        status_code: Some(status_code),
                        latency_ms,
                        ttft_ms: ttft,
                        protocol: req.protocol,
                        model: req.model.clone(),
                        url: safe_url,
                        stream: true,
                        purpose: req.purpose,
                        extracted_text: Some(redactor.redact(&recovered)),
                        tool_call_ok: None,
                        error_kind: None,
                        error_message: if ok {
                            None
                        } else {
                            Some("流式/完整 JSON 未解析到非空文本增量".into())
                        },
                        response_excerpt: None,
                        classification,
                        http_sent: true,
                        reused_from_cache: false,
                        suggestion_note: Some(
                            "stream=true 未解析到 SSE 增量，已回退完整 JSON/NDJSON 解析".into(),
                        ),
                        token_limit_field: None,
                        error_evidence: vec![],
                        channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                        response_compatibility: Some(if cross {
                            crate::protocols::types::ResponseCompatibility::CrossProtocol
                        } else {
                            crate::protocols::types::ResponseCompatibility::Native
                        }),
                        requested_protocol: Some(req.protocol),
                        matched_protocol: matched,
                        configured_model_display: None,
                        outbound_model: None,
                        model_transform: None,
                    };
                }
            }
            return AttemptResult {
                ok: false,
                partial: false,
                status_code: Some(status_code),
                latency_ms,
                ttft_ms: ttft,
                protocol: req.protocol,
                model: req.model.clone(),
                url: safe_url,
                stream: true,
                purpose: req.purpose,
                extracted_text: None,
                tool_call_ok: None,
                error_kind: Some("stream".into()),
                error_message: Some("流式响应未解析到文本增量".into()),
                response_excerpt: Some(truncate(&redactor.redact(&raw_bounded_buffer), 512)),
                classification: "STREAMING_UNSUPPORTED".into(),
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
                error_evidence: vec![],
                channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
                response_compatibility: None,
                requested_protocol: Some(req.protocol),
                matched_protocol: None,
                configured_model_display: None,
                outbound_model: None,
                model_transform: None,
            };
        }
        let (ok, partial) = evaluate_text(&text);
        let classification = if ok {
            if stream_cross {
                "STREAM_PROTOCOL_VARIANT_OK".into()
            } else {
                "STREAM_OK".into()
            }
        } else {
            "STREAMING_UNSUPPORTED".into()
        };
        AttemptResult {
            ok,
            partial,
            status_code: Some(status_code),
            latency_ms,
            ttft_ms: ttft,
            protocol: req.protocol,
            model: req.model.clone(),
            url: safe_url,
            stream: true,
            purpose: req.purpose,
            extracted_text: Some(redactor.redact(&text)),
            tool_call_ok: None,
            error_kind: None,
            error_message: if ok {
                None
            } else {
                Some("流式未解析到非空文本增量".into())
            },
            response_excerpt: None,
            classification,
            http_sent: true,
            reused_from_cache: false,
            suggestion_note: if stream_cross {
                Some("流式跨协议解析成功".into())
            } else {
                None
            },
            token_limit_field: None,
            error_evidence: vec![],
            channel: crate::protocols::types::DiagnosisChannel::DirectUpstream,
            response_compatibility: Some(if stream_cross {
                crate::protocols::types::ResponseCompatibility::CrossProtocol
            } else {
                crate::protocols::types::ResponseCompatibility::Native
            }),
            requested_protocol: Some(req.protocol),
            matched_protocol: stream_matched_protocol.or(Some(req.protocol)),
            configured_model_display: None,
            outbound_model: None,
            model_transform: None,
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    truncate_utf8(s, max)
}

/// Layered stream delta extraction: native protocol first, then other protocols.
fn extract_stream_delta_layered(
    target: ProtocolKind,
    data: &str,
) -> (Option<String>, Option<ProtocolKind>, bool) {
    let order: &[ProtocolKind] = match target {
        ProtocolKind::OpenAiChat => &[
            ProtocolKind::OpenAiChat,
            ProtocolKind::OpenAiResponses,
            ProtocolKind::AnthropicMessages,
            ProtocolKind::GeminiNative,
        ],
        ProtocolKind::OpenAiResponses => &[
            ProtocolKind::OpenAiResponses,
            ProtocolKind::OpenAiChat,
            ProtocolKind::AnthropicMessages,
            ProtocolKind::GeminiNative,
        ],
        ProtocolKind::AnthropicMessages => &[
            ProtocolKind::AnthropicMessages,
            ProtocolKind::OpenAiChat,
            ProtocolKind::OpenAiResponses,
            ProtocolKind::GeminiNative,
        ],
        ProtocolKind::GeminiNative => &[
            ProtocolKind::GeminiNative,
            ProtocolKind::OpenAiChat,
            ProtocolKind::AnthropicMessages,
            ProtocolKind::OpenAiResponses,
        ],
        ProtocolKind::Unknown => &[
            ProtocolKind::OpenAiChat,
            ProtocolKind::OpenAiResponses,
            ProtocolKind::AnthropicMessages,
            ProtocolKind::GeminiNative,
        ],
    };
    for kind in order {
        let delta = match kind {
            ProtocolKind::OpenAiChat => extract_chat_stream_delta(data),
            ProtocolKind::OpenAiResponses => extract_responses_stream_event(data),
            ProtocolKind::AnthropicMessages => extract_anthropic_stream_delta(data),
            ProtocolKind::GeminiNative => {
                extract_gemini_text(&serde_json::from_str(data).unwrap_or_default())
            }
            ProtocolKind::Unknown => None,
        };
        if let Some(d) = delta {
            return (Some(d), Some(*kind), *kind != target);
        }
    }
    // Full JSON event fallback
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
        if let Some(p) = super::parse::extract_response_text(target, &v) {
            return (Some(p.text), Some(p.matched_protocol), p.cross_protocol);
        }
    }
    (None, None, false)
}

enum BodyRead {
    Bytes(bytes::Bytes),
    TooLarge,
    Cancelled,
}

async fn read_body_bounded(
    response: reqwest::Response,
    max_bytes: usize,
    cancel: &CancellationToken,
) -> Result<BodyRead, String> {
    let mut stream = response.bytes_stream();
    let mut acc: Vec<u8> = Vec::new();
    loop {
        if cancel.is_cancelled() {
            return Ok(BodyRead::Cancelled);
        }
        let next = tokio::select! {
            _ = cancel.cancelled() => return Ok(BodyRead::Cancelled),
            item = stream.next() => item,
        };
        let Some(item) = next else { break };
        let chunk = item.map_err(|e| e.to_string())?;
        if acc.len().saturating_add(chunk.len()) > max_bytes {
            return Ok(BodyRead::TooLarge);
        }
        acc.extend_from_slice(&chunk);
    }
    Ok(BodyRead::Bytes(bytes::Bytes::from(acc)))
}

// silence unused import warning for StatusCode in some builds
#[allow(dead_code)]
fn _status_code_use(s: StatusCode) -> u16 {
    s.as_u16()
}

#[cfg(test)]
mod parse_integration_tests {
    use crate::ccs_adapter::ProtocolKind;
    use crate::protocols::parse::extract_response_text;
    use crate::protocols::types::evaluate_text;
    use serde_json::json;

    #[test]
    fn anthropic_success_with_billing_usage_is_not_quota_error() {
        let body = json!({
            "id": "f9bbb78d-17ae-94fb-8230-b4c6ad4c0f4f",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello."}],
            "stop_reason": "end_turn",
            "model": "grok-4.5-build-free",
            "usage": {
                "input_tokens": 206,
                "output_tokens": 6,
                "billing_usage": {"source": "oai_chat", "semantic": "openai"}
            }
        });
        let parsed = extract_response_text(ProtocolKind::AnthropicMessages, &body).unwrap();
        assert_eq!(parsed.text, "Hello.");
        let (ok, partial) = evaluate_text(&parsed.text);
        assert!(ok);
        assert!(!partial);
        // classifier must not call this a quota hit
        let raw = body.to_string();
        let (cls, _) = crate::diagnostics::classifier::classify_with_evidence(
            200,
            &raw,
            Some("application/json"),
        );
        assert_ne!(cls, "QUOTA_EXHAUSTED");
    }

    #[test]
    fn cross_protocol_openai_on_anthropic_target() {
        let body = json!({"choices":[{"message":{"content":"Hello."}}]});
        let p = extract_response_text(ProtocolKind::AnthropicMessages, &body).unwrap();
        assert!(p.cross_protocol);
        assert_eq!(p.text, "Hello.");
        assert!(evaluate_text(&p.text).0);
    }

    #[test]
    fn empty_native_text_is_not_generate_ok() {
        assert!(!evaluate_text("").0);
        assert!(!evaluate_text("   ").0);
    }
}
