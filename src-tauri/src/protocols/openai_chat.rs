use super::types::{
    apply_auth, AuthScheme, BuiltRequest, RequestPurpose, TokenLimitField, MAX_TOKENS, PROMPT_EN,
};
use crate::ccs_adapter::ProtocolKind;
use crate::security::url_variants::join_url;
use serde_json::json;
use std::collections::HashMap;

pub fn build_chat_request(
    base: &str,
    model: &str,
    api_key: &str,
    stream: bool,
    tool_call: bool,
    user_agent: Option<&str>,
    token_limit_field: TokenLimitField,
) -> BuiltRequest {
    let path = if base.trim_end_matches('/').ends_with("/v1") {
        "/chat/completions"
    } else {
        "/v1/chat/completions"
    };
    let url = join_url(base, path);

    let token_cap = if tool_call { 64 } else { MAX_TOKENS };
    let mut body = json!({
        "model": model,
        "messages": [{"role":"user","content": PROMPT_EN}],
        "stream": stream
    });
    match token_limit_field {
        TokenLimitField::MaxCompletionTokens => {
            body["max_completion_tokens"] = json!(token_cap);
        }
        TokenLimitField::MaxTokens => {
            body["max_tokens"] = json!(token_cap);
        }
    }

    if tool_call {
        body["tools"] = json!([{
            "type": "function",
            "function": {
                "name": "ccs_doctor_echo",
                "description": "Echo a value for connectivity testing. No side effects.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "value": {"type": "string"}
                    },
                    "required": ["value"]
                }
            }
        }]);
        body["tool_choice"] = json!({
            "type": "function",
            "function": {"name": "ccs_doctor_echo"}
        });
        body["messages"] = json!([
            {"role":"user","content":"Call the ccs_doctor_echo tool with value \"ok\". Do not answer otherwise."}
        ]);
    }

    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    apply_auth(&mut headers, AuthScheme::Bearer, api_key);
    if let Some(ua) = user_agent {
        if !ua.trim().is_empty() {
            headers.insert("User-Agent".into(), ua.trim().to_string());
        }
    }

    BuiltRequest {
        method: "POST".into(),
        url,
        headers,
        body: Some(body),
        stream,
        protocol: ProtocolKind::OpenAiChat,
        model: model.to_string(),
        purpose: if tool_call {
            RequestPurpose::ToolCall
        } else if stream {
            RequestPurpose::StreamGenerate
        } else {
            RequestPurpose::Generate
        },
    }
}

pub fn extract_chat_text(value: &serde_json::Value) -> Option<String> {
    if value.get("error").is_some() {
        return None;
    }
    value
        .pointer("/choices/0/message/content")
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Array(arr) => {
                let mut out = String::new();
                for part in arr {
                    if let Some(t) = part.get("text").and_then(|x| x.as_str()) {
                        out.push_str(t);
                    } else if let Some(t) = part.as_str() {
                        out.push_str(t);
                    }
                }
                if out.is_empty() {
                    None
                } else {
                    Some(out)
                }
            }
            _ => None,
        })
}

pub fn extract_chat_tool_call(value: &serde_json::Value) -> Option<(String, String)> {
    let tools = value.pointer("/choices/0/message/tool_calls")?.as_array()?;
    let first = tools.first()?;
    let name = first
        .pointer("/function/name")
        .and_then(|v| v.as_str())?
        .to_string();
    let args = first
        .pointer("/function/arguments")
        .and_then(|v| v.as_str())
        .unwrap_or("{}")
        .to_string();
    Some((name, args))
}

pub fn extract_chat_stream_delta(data: &str) -> Option<String> {
    if data.trim() == "[DONE]" {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    if let Some(c) = v
        .pointer("/choices/0/delta/content")
        .and_then(|x| x.as_str())
    {
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::types::is_max_completion_tokens_unsupported;
    use crate::protocols::types::AttemptResult;
    use serde_json::json;

    #[test]
    fn builds_url_with_v1() {
        let r = build_chat_request(
            "https://api.example.com",
            "m",
            "k",
            false,
            false,
            None,
            TokenLimitField::MaxCompletionTokens,
        );
        assert!(r.url.ends_with("/v1/chat/completions"));
    }

    #[test]
    fn first_request_uses_max_completion_tokens_only() {
        let r = build_chat_request(
            "https://api.example.com",
            "m",
            "k",
            false,
            false,
            None,
            TokenLimitField::MaxCompletionTokens,
        );
        let body = r.body.unwrap();
        assert!(body.get("max_completion_tokens").is_some());
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn fallback_request_uses_max_tokens_only() {
        let r = build_chat_request(
            "https://api.example.com",
            "m",
            "k",
            false,
            false,
            None,
            TokenLimitField::MaxTokens,
        );
        let body = r.body.unwrap();
        assert!(body.get("max_tokens").is_some());
        assert!(body.get("max_completion_tokens").is_none());
    }

    #[test]
    fn extracts_content() {
        let v = json!({"choices":[{"message":{"content":"CCS_DOCTOR_OK"}}]});
        assert_eq!(extract_chat_text(&v).as_deref(), Some("CCS_DOCTOR_OK"));
    }

    fn failed(status: u16, classification: &str, msg: &str) -> AttemptResult {
        AttemptResult {
            ok: false,
            partial: false,
            status_code: Some(status),
            latency_ms: 1,
            ttft_ms: None,
            protocol: ProtocolKind::OpenAiChat,
            model: "m".into(),
            url: "https://api.example.com/v1/chat/completions".into(),
            stream: false,
            purpose: RequestPurpose::Generate,
            extracted_text: None,
            tool_call_ok: None,
            error_kind: Some("http".into()),
            error_message: Some(msg.into()),
            response_excerpt: Some(msg.into()),
            classification: classification.into(),
            http_sent: true,
            reused_from_cache: false,
            suggestion_note: None,
            token_limit_field: Some(TokenLimitField::MaxCompletionTokens),
            error_evidence: vec![],
        }
    }

    #[test]
    fn unknown_parameter_triggers_fallback() {
        let r = failed(
            400,
            "UNKNOWN_ERROR",
            r#"{"error":{"message":"Unsupported parameter: 'max_completion_tokens'"}}"#,
        );
        assert!(is_max_completion_tokens_unsupported(&r));
    }

    #[test]
    fn auth_401_does_not_trigger_fallback() {
        let r = failed(401, "KEY_INVALID", "invalid api key max_completion_tokens");
        assert!(!is_max_completion_tokens_unsupported(&r));
    }

    #[test]
    fn rate_limit_429_does_not_trigger_fallback() {
        let r = failed(429, "RATE_LIMITED", "rate limit max_completion_tokens");
        assert!(!is_max_completion_tokens_unsupported(&r));
    }

    #[test]
    fn model_not_found_does_not_trigger_fallback() {
        let r = failed(
            404,
            "MODEL_NOT_FOUND",
            "model not found max_completion_tokens",
        );
        assert!(!is_max_completion_tokens_unsupported(&r));
    }
}
