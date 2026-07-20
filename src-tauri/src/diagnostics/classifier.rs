/// Classify HTTP failures into diagnostic codes.
pub fn classify_http_failure(status: u16, body: &str) -> String {
    let lower = body.to_ascii_lowercase();
    match status {
        401 => "KEY_INVALID".into(),
        403 => {
            if lower.contains("quota") || lower.contains("insufficient") {
                "QUOTA_EXHAUSTED".into()
            } else {
                "PERMISSION_DENIED".into()
            }
        }
        402 => "QUOTA_EXHAUSTED".into(),
        404 => {
            if lower.contains("model") {
                "MODEL_NOT_FOUND".into()
            } else {
                "ENDPOINT_NOT_FOUND".into()
            }
        }
        429 => {
            if lower.contains("insufficient_quota")
                || lower.contains("quota")
                || lower.contains("balance")
                || lower.contains("credit")
            {
                "QUOTA_EXHAUSTED".into()
            } else {
                // rate-limit text or ambiguous 429 without clear body
                "RATE_LIMITED".into()
            }
        }
        408 | 504 => "TIMEOUT".into(),
        500..=599 => "UNKNOWN_ERROR".into(),
        _ => {
            if lower.contains("invalid_api_key")
                || lower.contains("incorrect api key")
                || lower.contains("authentication")
            {
                "KEY_INVALID".into()
            } else if lower.contains("model") && lower.contains("not found") {
                "MODEL_NOT_FOUND".into()
            } else if lower.contains("insufficient_quota") {
                "QUOTA_EXHAUSTED".into()
            } else {
                "UNKNOWN_ERROR".into()
            }
        }
    }
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
    best_classification.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_401() {
        assert_eq!(classify_http_failure(401, "unauthorized"), "KEY_INVALID");
    }

    #[test]
    fn classifies_quota() {
        assert_eq!(
            classify_http_failure(429, r#"{"error":{"code":"insufficient_quota"}}"#),
            "QUOTA_EXHAUSTED"
        );
    }

    #[test]
    fn classifies_model_404() {
        assert_eq!(
            classify_http_failure(404, "model not found"),
            "MODEL_NOT_FOUND"
        );
    }
}
