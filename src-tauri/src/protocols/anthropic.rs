use super::types::{
    apply_auth, AuthScheme, BuiltRequest, RequestPurpose, BASIC_GENERATE_PROMPT, MAX_TOKENS,
};
use crate::ccs_adapter::ProtocolKind;
use crate::security::url_variants::join_url;
use serde_json::json;
use std::collections::HashMap;

pub fn build_anthropic_request(
    base: &str,
    model: &str,
    api_key: &str,
    stream: bool,
    tool_call: bool,
    use_bearer: bool,
    user_agent: Option<&str>,
) -> BuiltRequest {
    let path = if base.trim_end_matches('/').ends_with("/v1") {
        "/messages"
    } else {
        "/v1/messages"
    };
    let url = join_url(base, path);

    let mut body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "messages": [{"role":"user","content": BASIC_GENERATE_PROMPT}],
        "stream": stream
    });

    if tool_call {
        body["tools"] = json!([{
            "name": "ccs_doctor_echo",
            "description": "Echo a value for connectivity testing. No side effects.",
            "input_schema": {
                "type": "object",
                "properties": {"value": {"type": "string"}},
                "required": ["value"]
            }
        }]);
        body["tool_choice"] = json!({"type":"tool","name":"ccs_doctor_echo"});
        body["messages"] = json!([
            {"role":"user","content":"Call the ccs_doctor_echo tool with value \"ok\"."}
        ]);
        body["max_tokens"] = json!(64);
    }

    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    if use_bearer {
        apply_auth(&mut headers, AuthScheme::Bearer, api_key);
        headers
            .entry("anthropic-version".into())
            .or_insert_with(|| "2023-06-01".into());
    } else {
        apply_auth(&mut headers, AuthScheme::XApiKey, api_key);
    }
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
        protocol: ProtocolKind::AnthropicMessages,
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

pub fn extract_anthropic_text(value: &serde_json::Value) -> Option<String> {
    if let Some(err) = value.get("error") {
        if crate::diagnostics::classifier::is_meaningful_error_value(err) {
            return None;
        }
    }
    // content as string
    if let Some(s) = value.get("content").and_then(|v| v.as_str()) {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    let content = value.get("content")?.as_array()?;
    let mut out = String::new();
    for c in content {
        if c.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = c.get("text").and_then(|x| x.as_str()) {
                out.push_str(t);
            }
        } else if let Some(t) = c.get("text").and_then(|x| x.as_str()) {
            // some gateways omit type
            out.push_str(t);
        } else if let Some(t) = c.as_str() {
            out.push_str(t);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub fn extract_anthropic_tool_use(value: &serde_json::Value) -> Option<(String, String)> {
    let content = value.get("content")?.as_array()?;
    for c in content {
        if c.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
            let name = c.get("name").and_then(|v| v.as_str())?.to_string();
            let input = c.get("input").cloned().unwrap_or(json!({}));
            return Some((name, input.to_string()));
        }
    }
    None
}

pub fn extract_anthropic_stream_delta(data: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    // content_block_delta
    if let Some(t) = v
        .pointer("/delta/text")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
    {
        return Some(t.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_text() {
        let v = json!({"content":[{"type":"text","text":"CCS_DOCTOR_OK"}]});
        assert_eq!(extract_anthropic_text(&v).as_deref(), Some("CCS_DOCTOR_OK"));
    }
}
