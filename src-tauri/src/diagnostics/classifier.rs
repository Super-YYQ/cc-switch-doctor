//! Classify HTTP failures into diagnostic codes with evidence priority.

use crate::protocols::types::ErrorEvidence;

/// Lower number = higher priority when choosing final status.
pub fn classification_priority(code: &str) -> u32 {
    match code {
        "SECURITY_BLOCKED" | "CROSS_ORIGIN_REDIRECT_BLOCKED" => 1,
        "AUTH_INVALID" | "KEY_INVALID" => 2,
        "AUTH_PERMISSION_DENIED" | "PERMISSION_DENIED" => 3,
        "QUOTA_EXHAUSTED" => 4,
        "RATE_LIMITED" => 5,
        "MODEL_NOT_FOUND" => 6,
        "ENDPOINT_NOT_FOUND" => 7,
        "GATEWAY_OR_WAF" => 8,
        "TLS_ERROR" => 9,
        "NETWORK_UNREACHABLE" => 10,
        "TIMEOUT" => 11,
        "INVALID_REQUEST_PARAMETER" => 12,
        "RESPONSE_FORMAT_MISMATCH" => 13,
        "UNSUPPORTED_PROTOCOL" => 14,
        "HOST_BUDGET_EXHAUSTED" | "HOST_RATE_LIMIT_STOPPED" => 15,
        "CANCELLED" => 16,
        _ => 50,
    }
}

pub fn best_classification<'a>(candidates: impl IntoIterator<Item = &'a str>) -> String {
    candidates
        .into_iter()
        .min_by_key(|c| classification_priority(c))
        .unwrap_or("UNKNOWN_ERROR")
        .to_string()
}

pub fn classify_http_failure(status: u16, body: &str) -> String {
    classify_with_evidence(status, body, None).0
}

/// Detect an explicit 2xx business-error envelope (not free-text keywords).
/// Returns (classification, evidence) only when structure is clearly an error.
pub fn classify_structured_error_envelope(
    status: u16,
    body: &str,
    parsed: &serde_json::Value,
) -> Option<(String, Vec<ErrorEvidence>)> {
    // Top-level {"error": ...}
    if let Some(err) = parsed.get("error") {
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .or_else(|| err.as_str())
            .unwrap_or("")
            .to_string();
        let code = err
            .get("code")
            .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| {
                v.as_i64().map(|n| n.to_string())
            }));
        let (cls, mut evidence) = classify_with_evidence(status, body, None);
        if evidence.is_empty() {
            evidence.push(ErrorEvidence {
                source: "error_envelope".into(),
                code: code.clone().or_else(|| Some(status.to_string())),
                message: if msg.is_empty() { None } else { Some(msg) },
                matched_keyword: None,
            });
        } else {
            for e in &mut evidence {
                e.source = "error_envelope".into();
                if e.message.is_none() && !msg.is_empty() {
                    e.message = Some(msg.clone());
                }
            }
        }
        return Some((cls, evidence));
    }

    // {"success": false, ...} / {"ok": false, ...} / {"status": "error", ...}
    let success_false = parsed
        .get("success")
        .and_then(|v| v.as_bool())
        .map(|b| !b)
        .unwrap_or(false);
    let ok_false = parsed
        .get("ok")
        .and_then(|v| v.as_bool())
        .map(|b| !b)
        .unwrap_or(false);
    let status_error = parsed
        .get("status")
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("error") || s.eq_ignore_ascii_case("failed"))
        .unwrap_or(false);

    if success_false || ok_false || status_error {
        let msg = parsed
            .get("message")
            .or_else(|| parsed.get("msg"))
            .or_else(|| parsed.get("error_message"))
            .and_then(|v| v.as_str())
            .unwrap_or(body);
        let (cls, mut evidence) = classify_with_evidence(status, msg, None);
        if evidence.is_empty() {
            evidence.push(ErrorEvidence {
                source: "structured_flag".into(),
                code: Some(status.to_string()),
                message: Some(msg.chars().take(200).collect()),
                matched_keyword: None,
            });
        } else {
            for e in &mut evidence {
                e.source = "structured_flag".into();
            }
        }
        // Prefer a real classification over UNKNOWN when flag is explicitly false
        let cls = if cls == "UNKNOWN_ERROR" {
            "UNKNOWN_ERROR".into()
        } else {
            cls
        };
        return Some((cls, evidence));
    }

    None
}

pub fn classify_with_evidence(
    status: u16,
    body: &str,
    content_type: Option<&str>,
) -> (String, Vec<ErrorEvidence>) {
    let lower = body.to_ascii_lowercase();
    let ct = content_type.unwrap_or("").to_ascii_lowercase();
    let mut evidence = Vec::new();

    if ct.contains("text/html")
        || lower.contains("cloudflare")
        || lower.contains("just a moment")
        || lower.contains("captcha")
        || lower.contains("web application firewall")
        || (lower.contains("access denied") && lower.contains("<html"))
        || lower.contains("bad gateway")
    {
        evidence.push(ErrorEvidence {
            source: "content_type_or_html".into(),
            code: Some(status.to_string()),
            message: None,
            matched_keyword: Some("gateway/waf".into()),
        });
        return ("GATEWAY_OR_WAF".into(), evidence);
    }

    let auth_kw = [
        "invalid api key",
        "incorrect api key",
        "unauthorized",
        "authentication failed",
        "invalid token",
        "token expired",
        "signature invalid",
        "api key not valid",
        "invalid x-api-key",
        "invalid_api_key",
    ];
    let perm_kw = ["permission denied", "forbidden", "access denied"];
    // Never use bare "billing"/"usage"/"credit"/"payment" — they appear in
    // normal success payloads (e.g. billing_usage, token usage, credit_cost).
    let quota_kw = [
        "insufficient_quota",
        "quota exceeded",
        "quota exhausted",
        "insufficient balance",
        "balance insufficient",
        "no balance",
        "credit exhausted",
        "credits exhausted",
        "payment required",
        "billing error",
        "billing disabled",
        "billing hard limit",
        "billing_hard_limit",
        "余额不足",
        "额度不足",
        "欠费",
        "无可用额度",
    ];
    let rate_kw = [
        "rate limit",
        "too many requests",
        "requests per minute",
        "tokens per minute",
        "retry after",
        "retry-after",
        "限流",
        "请求过于频繁",
    ];
    let model_kw = [
        "model not found",
        "unknown model",
        "invalid model",
        "model does not exist",
        "no access to model",
        "模型不存在",
        "无权访问模型",
    ];

    let hit = |words: &[&str]| -> Option<String> {
        words
            .iter()
            .find(|w| lower.contains(*w))
            .map(|s| (*s).to_string())
    };

    // Non-2xx: HTTP status is authoritative first for 401/402/403/404/429.
    // For 2xx keyword scans, still run (caller should only invoke after success
    // parse failed, except for structured envelopes).
    match status {
        401 => {
            evidence.push(ErrorEvidence {
                source: "http_status".into(),
                code: Some("401".into()),
                message: None,
                matched_keyword: hit(&auth_kw),
            });
            return ("AUTH_INVALID".into(), evidence);
        }
        402 => {
            evidence.push(ErrorEvidence {
                source: "http_status".into(),
                code: Some("402".into()),
                message: None,
                matched_keyword: hit(&quota_kw),
            });
            return ("QUOTA_EXHAUSTED".into(), evidence);
        }
        403 => {
            if let Some(k) = hit(&perm_kw) {
                evidence.push(ErrorEvidence {
                    source: "text_keyword".into(),
                    code: Some("403".into()),
                    message: None,
                    matched_keyword: Some(k),
                });
            } else {
                evidence.push(ErrorEvidence {
                    source: "http_status".into(),
                    code: Some("403".into()),
                    message: None,
                    matched_keyword: None,
                });
            }
            return ("AUTH_PERMISSION_DENIED".into(), evidence);
        }
        404 => {
            if let Some(k) = hit(&model_kw) {
                evidence.push(ErrorEvidence {
                    source: "text_keyword".into(),
                    code: Some("404".into()),
                    message: None,
                    matched_keyword: Some(k),
                });
                return ("MODEL_NOT_FOUND".into(), evidence);
            }
            evidence.push(ErrorEvidence {
                source: "http_status".into(),
                code: Some("404".into()),
                message: None,
                matched_keyword: None,
            });
            return ("ENDPOINT_NOT_FOUND".into(), evidence);
        }
        429 => {
            // Prefer quota over rate-limit when body says so
            if let Some(k) = hit(&quota_kw) {
                evidence.push(ErrorEvidence {
                    source: "text_keyword".into(),
                    code: Some("429".into()),
                    message: None,
                    matched_keyword: Some(k),
                });
                return ("QUOTA_EXHAUSTED".into(), evidence);
            }
            evidence.push(ErrorEvidence {
                source: "http_status".into(),
                code: Some("429".into()),
                message: None,
                matched_keyword: hit(&rate_kw),
            });
            return ("RATE_LIMITED".into(), evidence);
        }
        408 | 504 => return ("TIMEOUT".into(), evidence),
        500..=599 => {
            if lower.contains("nginx") || lower.contains("bad gateway") {
                return ("GATEWAY_OR_WAF".into(), evidence);
            }
            return ("UNKNOWN_ERROR".into(), evidence);
        }
        _ => {}
    }

    if let Some(k) = hit(&quota_kw) {
        evidence.push(ErrorEvidence {
            source: "text_keyword".into(),
            code: Some(status.to_string()),
            message: None,
            matched_keyword: Some(k),
        });
        return ("QUOTA_EXHAUSTED".into(), evidence);
    }
    if let Some(k) = hit(&auth_kw) {
        evidence.push(ErrorEvidence {
            source: "text_keyword".into(),
            code: Some(status.to_string()),
            message: None,
            matched_keyword: Some(k),
        });
        return ("AUTH_INVALID".into(), evidence);
    }
    if let Some(k) = hit(&model_kw) {
        evidence.push(ErrorEvidence {
            source: "text_keyword".into(),
            code: Some(status.to_string()),
            message: None,
            matched_keyword: Some(k),
        });
        return ("MODEL_NOT_FOUND".into(), evidence);
    }
    if let Some(k) = hit(&rate_kw) {
        evidence.push(ErrorEvidence {
            source: "text_keyword".into(),
            code: Some(status.to_string()),
            message: None,
            matched_keyword: Some(k),
        });
        return ("RATE_LIMITED".into(), evidence);
    }
    if let Some(k) = hit(&perm_kw) {
        evidence.push(ErrorEvidence {
            source: "text_keyword".into(),
            code: Some(status.to_string()),
            message: None,
            matched_keyword: Some(k),
        });
        return ("AUTH_PERMISSION_DENIED".into(), evidence);
    }

    if (lower.contains("unknown parameter")
        || lower.contains("unsupported parameter")
        || lower.contains("unrecognized"))
        && (lower.contains("max_completion_tokens") || lower.contains("max_tokens"))
    {
        return ("INVALID_REQUEST_PARAMETER".into(), evidence);
    }

    ("UNKNOWN_ERROR".into(), evidence)
}

pub fn final_status_from_attempts(
    current_ok: bool,
    any_ok: bool,
    best_classification: &str,
    protocol_changed: bool,
    url_changed: bool,
    model_changed: bool,
    needs_local_routing: bool,
) -> String {
    if current_ok {
        return "CURRENT_CONFIG_OK".into();
    }
    if any_ok {
        if needs_local_routing {
            return "LOCAL_ROUTING_REQUIRED".into();
        }
        if protocol_changed {
            return "PROTOCOL_FALLBACK_OK".into();
        }
        if model_changed {
            return "MODEL_VARIANT_OK".into();
        }
        if url_changed {
            return "CORRECTED_BASE_PATH_OK".into();
        }
        return "AUTH_VARIANT_OK".into();
    }
    if best_classification == "KEY_INVALID" {
        return "AUTH_INVALID".into();
    }
    if best_classification == "PERMISSION_DENIED" {
        return "AUTH_PERMISSION_DENIED".into();
    }
    best_classification.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_401() {
        assert_eq!(
            classify_http_failure(401, r#"{"error":{"message":"invalid api key"}}"#),
            "AUTH_INVALID"
        );
    }

    #[test]
    fn classifies_quota_chinese() {
        assert_eq!(
            classify_http_failure(200, r#"{"code":1008,"msg":"余额不足"}"#),
            "QUOTA_EXHAUSTED"
        );
    }

    #[test]
    fn classifies_waf_html() {
        let (c, _) = classify_with_evidence(
            200,
            "<html>cloudflare just a moment</html>",
            Some("text/html"),
        );
        assert_eq!(c, "GATEWAY_OR_WAF");
    }

    #[test]
    fn classifies_model_404() {
        assert_eq!(
            classify_http_failure(404, "model not found: gpt-x"),
            "MODEL_NOT_FOUND"
        );
    }

    #[test]
    fn classifies_rate_limit() {
        assert_eq!(
            classify_http_failure(429, "rate limit exceeded"),
            "RATE_LIMITED"
        );
    }

    #[test]
    fn priority_prefers_auth_over_endpoint() {
        assert_eq!(
            best_classification(["ENDPOINT_NOT_FOUND", "AUTH_INVALID"]),
            "AUTH_INVALID"
        );
    }

    #[test]
    fn classifies_quota_on_429() {
        assert_eq!(
            classify_http_failure(429, r#"{"error":{"code":"insufficient_quota"}}"#),
            "QUOTA_EXHAUSTED"
        );
    }

    #[test]
    fn bare_billing_usage_is_not_quota() {
        let body = r#"{
            "id":"f9bbb78d","type":"message","role":"assistant",
            "content":[{"type":"text","text":"CCS_DOCTOR_OK"}],
            "stop_reason":"end_turn","model":"grok-4.5-build-free",
            "usage":{"input_tokens":206,"output_tokens":6,
              "billing_usage":{"source":"oai_chat","semantic":"openai"}}
        }"#;
        let (c, _) = classify_with_evidence(200, body, Some("application/json"));
        assert_ne!(c, "QUOTA_EXHAUSTED", "billing_usage must not trigger quota");
        assert_eq!(c, "UNKNOWN_ERROR");
    }

    #[test]
    fn openai_usage_is_not_quota() {
        let body = r#"{"id":"x","choices":[{"message":{"content":"CCS_DOCTOR_OK"}}],
            "usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13}}"#;
        let (c, _) = classify_with_evidence(200, body, Some("application/json"));
        assert_ne!(c, "QUOTA_EXHAUSTED");
    }

    #[test]
    fn structured_success_false_quota() {
        let body = r#"{"success":false,"message":"余额不足"}"#;
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        let (c, ev) = classify_structured_error_envelope(200, body, &parsed).unwrap();
        assert_eq!(c, "QUOTA_EXHAUSTED");
        assert!(!ev.is_empty());
    }

    #[test]
    fn structured_error_insufficient_balance() {
        let body = r#"{"error":{"message":"insufficient balance"}}"#;
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        let (c, _) = classify_structured_error_envelope(200, body, &parsed).unwrap();
        assert_eq!(c, "QUOTA_EXHAUSTED");
    }

    #[test]
    fn structured_envelope_absent_on_success_body() {
        let body = r#"{"content":[{"type":"text","text":"ok"}],"usage":{"billing_usage":{}}}"#;
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert!(classify_structured_error_envelope(200, body, &parsed).is_none());
    }
}
