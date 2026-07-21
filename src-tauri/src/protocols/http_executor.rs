use super::anthropic::{
    extract_anthropic_stream_delta, extract_anthropic_text, extract_anthropic_tool_use,
};
use super::gemini::{extract_gemini_text, extract_gemini_tool_call};
use super::openai_chat::{extract_chat_stream_delta, extract_chat_text, extract_chat_tool_call};
use super::openai_responses::{
    extract_responses_stream_event, extract_responses_text, extract_responses_tool_call,
};
use super::types::{
    evaluate_text, redact_result, AttemptResult, BuiltRequest, RequestPurpose, MAX_BODY_BYTES,
    MAX_ERROR_BYTES,
};
use crate::ccs_adapter::ProtocolKind;
use crate::diagnostics::classifier::classify_http_failure;
use crate::security::origin::SameOriginPolicy;
use crate::security::redact::{sanitize_url_for_display, SecretRedactor};
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
            };
        }

        if req.stream {
            return self
                .read_stream(response, req, safe_url, started, redactor, cancel)
                .await;
        }

        let status_code = status.as_u16();
        let bytes = match response.bytes().await {
            Ok(b) => b,
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
                    error_message: Some(redactor.redact(&e.to_string())),
                    response_excerpt: None,
                    classification: "NETWORK_UNREACHABLE".into(),
                    http_sent: true,
                    reused_from_cache: false,
                    suggestion_note: None,
                    token_limit_field: None,
                };
            }
        };

        if bytes.len() > MAX_BODY_BYTES {
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
                error_message: Some("响应体超过 2MB 限制".into()),
                response_excerpt: None,
                classification: "UNKNOWN_ERROR".into(),
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
            };
        }

        let body_text = String::from_utf8_lossy(&bytes).to_string();
        let latency_ms = started.elapsed().as_millis() as u64;

        if !status.is_success() {
            let excerpt = truncate(&redactor.redact(&body_text), MAX_ERROR_BYTES);
            let classification = classify_http_failure(status_code, &body_text);
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
            };
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&body_text).unwrap_or(serde_json::Value::Null);
        if parsed.get("error").is_some() {
            let excerpt = truncate(&redactor.redact(&body_text), 512);
            let classification = classify_http_failure(status_code, &body_text);
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
                error_kind: Some("nested_error".into()),
                error_message: Some(excerpt.clone()),
                response_excerpt: Some(excerpt),
                classification,
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
            };
        }

        if req.purpose == RequestPurpose::ToolCall {
            let tool = match req.protocol {
                ProtocolKind::OpenAiChat => extract_chat_tool_call(&parsed),
                ProtocolKind::OpenAiResponses => extract_responses_tool_call(&parsed),
                ProtocolKind::AnthropicMessages => extract_anthropic_tool_use(&parsed),
                ProtocolKind::GeminiNative => extract_gemini_tool_call(&parsed),
                ProtocolKind::Unknown => None,
            };
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
            };
        }

        let text = match req.protocol {
            ProtocolKind::OpenAiChat => extract_chat_text(&parsed),
            ProtocolKind::OpenAiResponses => extract_responses_text(&parsed),
            ProtocolKind::AnthropicMessages => extract_anthropic_text(&parsed),
            ProtocolKind::GeminiNative => extract_gemini_text(&parsed),
            ProtocolKind::Unknown => None,
        };

        match text {
            Some(t) => {
                let (ok, partial) = evaluate_text(&t);
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
                        Some("返回了有效文本但未包含 CCS_DOCTOR_OK 标记".into())
                    } else {
                        Some("响应结构成功但无文本".into())
                    },
                    response_excerpt: None,
                    classification: if ok {
                        "GENERATE_OK".into()
                    } else if partial {
                        "PARTIAL_TEXT".into()
                    } else {
                        "UNKNOWN_ERROR".into()
                    },
                    http_sent: true,
                    reused_from_cache: false,
                    suggestion_note: None,
                    token_limit_field: None,
                }
            }
            None => AttemptResult {
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
                classification: "UNSUPPORTED_PROTOCOL".into(),
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
            },
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
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            let excerpt = truncate(&redactor.redact(&body), MAX_ERROR_BYTES);
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
                classification: classify_http_failure(status_code, &body),
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
            };
        }

        let mut stream = response.bytes_stream();
        let mut buf = String::new();
        let mut text = String::new();
        let mut ttft: Option<u64> = None;
        let mut total_bytes = 0usize;
        let mut saw_done = false;

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
                    };
                }
            };
            total_bytes += chunk.len();
            if total_bytes > MAX_BODY_BYTES {
                break;
            }
            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buf.find('\n') {
                let mut line = buf[..pos].to_string();
                buf = buf[pos + 1..].to_string();
                if line.ends_with('\r') {
                    line.pop();
                }
                if line.is_empty() {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        saw_done = true;
                        continue;
                    }
                    let delta = match req.protocol {
                        ProtocolKind::OpenAiChat => extract_chat_stream_delta(data),
                        ProtocolKind::OpenAiResponses => extract_responses_stream_event(data),
                        ProtocolKind::AnthropicMessages => extract_anthropic_stream_delta(data),
                        ProtocolKind::GeminiNative => {
                            // Gemini SSE may send full JSON chunks
                            extract_gemini_text(&serde_json::from_str(data).unwrap_or_default())
                        }
                        ProtocolKind::Unknown => None,
                    };
                    if let Some(d) = delta {
                        if ttft.is_none() {
                            ttft = Some(started.elapsed().as_millis() as u64);
                        }
                        text.push_str(&d);
                    }
                }
            }
            if saw_done {
                break;
            }
        }

        let latency_ms = started.elapsed().as_millis() as u64;
        if text.is_empty() {
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
                response_excerpt: None,
                classification: "STREAMING_UNSUPPORTED".into(),
                http_sent: true,
                reused_from_cache: false,
                suggestion_note: None,
                token_limit_field: None,
            };
        }
        let (ok, partial) = evaluate_text(&text);
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
            } else if partial {
                Some("流式返回有效文本但缺少 CCS_DOCTOR_OK".into())
            } else {
                None
            },
            response_excerpt: None,
            classification: if ok {
                "STREAM_OK".into()
            } else if partial {
                "PARTIAL_TEXT".into()
            } else {
                "STREAMING_UNSUPPORTED".into()
            },
            http_sent: true,
            reused_from_cache: false,
            suggestion_note: None,
            token_limit_field: None,
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

// silence unused import warning for StatusCode in some builds
#[allow(dead_code)]
fn _status_code_use(s: StatusCode) -> u16 {
    s.as_u16()
}
