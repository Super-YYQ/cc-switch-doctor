use super::types::{apply_auth, AuthScheme, BuiltRequest, RequestPurpose, MAX_TOKENS, PROMPT_EN};
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
) -> BuiltRequest {
    let path = if base.trim_end_matches('/').ends_with("/v1") {
        "/chat/completions"
    } else {
        "/v1/chat/completions"
    };
    // Also try without forcing v1 when base already includes full path-like ending
    let url = join_url(base, path);

    let mut body = json!({
        "model": model,
        "messages": [{"role":"user","content": PROMPT_EN}],
        "max_tokens": MAX_TOKENS,
        "stream": stream
    });

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
        body["max_tokens"] = json!(64);
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
    // nested error
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
    // reasoning_content is auxiliary only
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_url_with_v1() {
        let r = build_chat_request("https://api.example.com", "m", "k", false, false, None);
        assert!(r.url.ends_with("/v1/chat/completions"));
    }

    #[test]
    fn extracts_content() {
        let v = json!({"choices":[{"message":{"content":"CCS_DOCTOR_OK"}}]});
        assert_eq!(extract_chat_text(&v).as_deref(), Some("CCS_DOCTOR_OK"));
    }
}
