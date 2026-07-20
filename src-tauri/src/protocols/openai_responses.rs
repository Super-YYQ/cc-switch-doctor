use super::types::{apply_auth, AuthScheme, BuiltRequest, RequestPurpose, MAX_TOKENS, PROMPT_EN};
use crate::ccs_adapter::ProtocolKind;
use crate::security::url_variants::join_url;
use serde_json::json;
use std::collections::HashMap;

pub fn build_responses_request(
    base: &str,
    model: &str,
    api_key: &str,
    stream: bool,
    tool_call: bool,
    user_agent: Option<&str>,
) -> BuiltRequest {
    let path = if base.trim_end_matches('/').ends_with("/v1") {
        "/responses"
    } else {
        "/v1/responses"
    };
    let url = join_url(base, path);

    let mut body = json!({
        "model": model,
        "input": PROMPT_EN,
        "max_output_tokens": MAX_TOKENS,
        "stream": stream
    });

    if tool_call {
        body["tools"] = json!([{
            "type": "function",
            "name": "ccs_doctor_echo",
            "description": "Echo a value for connectivity testing. No side effects.",
            "parameters": {
                "type": "object",
                "properties": {"value": {"type": "string"}},
                "required": ["value"]
            }
        }]);
        body["tool_choice"] = json!({"type":"function","name":"ccs_doctor_echo"});
        body["input"] = json!("Call the ccs_doctor_echo tool with value \"ok\".");
        body["max_output_tokens"] = json!(64);
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
        protocol: ProtocolKind::OpenAiResponses,
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

pub fn extract_responses_text(value: &serde_json::Value) -> Option<String> {
    if value.get("error").is_some() {
        return None;
    }
    if let Some(s) = value.get("output_text").and_then(|v| v.as_str()) {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    // output[].content[].text
    if let Some(output) = value.get("output").and_then(|v| v.as_array()) {
        let mut out = String::new();
        for item in output {
            if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
                for c in content {
                    if let Some(t) = c.get("text").and_then(|v| v.as_str()) {
                        out.push_str(t);
                    }
                }
            }
            // function call style may not have text
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

pub fn extract_responses_tool_call(value: &serde_json::Value) -> Option<(String, String)> {
    let output = value.get("output")?.as_array()?;
    for item in output {
        let t = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if t == "function_call" || t == "tool_call" {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = item
                .get("arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}")
                .to_string();
            if !name.is_empty() {
                return Some((name, args));
            }
        }
    }
    None
}

pub fn extract_responses_stream_event(data: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    // response.output_text.delta
    if let Some(d) = v.get("delta").and_then(|x| x.as_str()) {
        if !d.is_empty() {
            return Some(d.to_string());
        }
    }
    if let Some(d) = v.pointer("/data/delta").and_then(|x| x.as_str()) {
        if !d.is_empty() {
            return Some(d.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_output_text() {
        let v = json!({"output_text":"CCS_DOCTOR_OK"});
        assert_eq!(extract_responses_text(&v).as_deref(), Some("CCS_DOCTOR_OK"));
    }
}
