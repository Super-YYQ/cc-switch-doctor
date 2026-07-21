use super::types::{apply_auth, AuthScheme, BuiltRequest, RequestPurpose, MAX_TOKENS, PROMPT_EN};
use crate::ccs_adapter::ProtocolKind;
use crate::security::url_variants::join_url;
use serde_json::json;
use std::collections::HashMap;

pub fn build_gemini_request(
    base: &str,
    model: &str,
    api_key: &str,
    stream: bool,
    tool_call: bool,
    user_agent: Option<&str>,
) -> BuiltRequest {
    build_gemini_request_with_auth(
        base,
        model,
        api_key,
        stream,
        tool_call,
        user_agent,
        AuthScheme::XGoogApiKey,
    )
}

pub fn build_gemini_request_with_auth(
    base: &str,
    model: &str,
    api_key: &str,
    stream: bool,
    tool_call: bool,
    user_agent: Option<&str>,
    auth: AuthScheme,
) -> BuiltRequest {
    let action = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    let mut url = build_gemini_url(base, model, action);

    let mut body = json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": PROMPT_EN}]
        }],
        "generationConfig": {
            "maxOutputTokens": MAX_TOKENS
        }
    });

    if tool_call {
        body["tools"] = json!([{
            "functionDeclarations": [{
                "name": "ccs_doctor_echo",
                "description": "Echo a value for connectivity testing.",
                "parameters": {
                    "type": "object",
                    "properties": {"value": {"type": "string"}},
                    "required": ["value"]
                }
            }]
        }]);
        body["contents"] = json!([{
            "role":"user",
            "parts":[{"text":"Call ccs_doctor_echo with value \"ok\"."}]
        }]);
        body["generationConfig"] = json!({"maxOutputTokens": 64});
    }

    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    match auth {
        AuthScheme::QueryKey => {
            url = append_query(&url, "key", api_key);
        }
        other => {
            apply_auth(&mut headers, other, api_key);
        }
    }
    if let Some(ua) = user_agent {
        if !ua.trim().is_empty() {
            headers.insert("User-Agent".into(), ua.trim().to_string());
        }
    }

    if stream {
        url = append_query(&url, "alt", "sse");
    }

    BuiltRequest {
        method: "POST".into(),
        url,
        headers,
        body: Some(body),
        stream,
        protocol: ProtocolKind::GeminiNative,
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

fn build_gemini_url(base: &str, model: &str, action: &str) -> String {
    let mut b = base.trim().trim_end_matches('/').to_string();
    for suffix in [
        "/generateContent",
        "/streamGenerateContent",
        ":generateContent",
        ":streamGenerateContent",
    ] {
        if let Some(idx) = b.rfind(suffix) {
            b.truncate(idx);
        }
    }
    if let Some(idx) = b.rfind("/models/") {
        b.truncate(idx);
    } else if b.ends_with("/models") {
        b.truncate(b.len() - "/models".len());
    }
    for ver in ["/v1beta", "/v1"] {
        if b.ends_with(ver) {
            b.truncate(b.len() - ver.len());
            break;
        }
    }
    let path = format!("/v1beta/models/{model}:{action}");
    join_url(&b, &path)
}

fn append_query(url: &str, key: &str, value: &str) -> String {
    if url.contains(&format!("{key}=")) {
        return url.to_string();
    }
    if url.contains('?') {
        format!("{url}&{key}={value}")
    } else {
        format!("{url}?{key}={value}")
    }
}

pub fn extract_gemini_text(value: &serde_json::Value) -> Option<String> {
    if value.get("error").is_some() {
        return None;
    }
    let cands = value.get("candidates")?.as_array()?;
    let mut out = String::new();
    for c in cands {
        if let Some(parts) = c.pointer("/content/parts").and_then(|v| v.as_array()) {
            for p in parts {
                if let Some(t) = p.get("text").and_then(|x| x.as_str()) {
                    out.push_str(t);
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub fn extract_gemini_tool_call(value: &serde_json::Value) -> Option<(String, String)> {
    let cands = value.get("candidates")?.as_array()?;
    for c in cands {
        if let Some(parts) = c.pointer("/content/parts").and_then(|v| v.as_array()) {
            for p in parts {
                if let Some(fc) = p.get("functionCall") {
                    let name = fc.get("name").and_then(|v| v.as_str())?.to_string();
                    let args = fc.get("args").cloned().unwrap_or(json!({})).to_string();
                    return Some((name, args));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_double_v1beta() {
        let u = build_gemini_url("https://api.example.com/v1beta", "m", "generateContent");
        assert!(!u.contains("/v1beta/v1beta"), "{u}");
        assert!(u.contains("/v1beta/models/m:generateContent"), "{u}");
    }

    #[test]
    fn base_with_v1() {
        let u = build_gemini_url("https://api.example.com/v1", "m", "generateContent");
        assert!(
            u.ends_with("/v1beta/models/m:generateContent")
                || u.contains("/v1beta/models/m:generateContent"),
            "{u}"
        );
        assert!(!u.contains("/v1/v1beta"), "{u}");
    }

    #[test]
    fn extract_text() {
        let v = json!({"candidates":[{"content":{"parts":[{"text":"CCS_DOCTOR_OK"}]}}]});
        assert_eq!(extract_gemini_text(&v).as_deref(), Some("CCS_DOCTOR_OK"));
    }

    #[test]
    fn header_auth_puts_x_goog_api_key() {
        let r = build_gemini_request_with_auth(
            "https://api.example.com",
            "m",
            "secret-key-value",
            false,
            false,
            None,
            AuthScheme::XGoogApiKey,
        );
        assert!(r.headers.contains_key("x-goog-api-key"));
        assert!(!r.url.contains("key="));
    }

    #[test]
    fn query_auth_puts_key_param() {
        let r = build_gemini_request_with_auth(
            "https://api.example.com",
            "m",
            "secret-key-value",
            false,
            false,
            None,
            AuthScheme::QueryKey,
        );
        assert!(r.url.contains("key=secret-key-value"));
        assert!(!r.headers.contains_key("x-goog-api-key"));
    }

    #[test]
    fn stream_uses_alt_sse() {
        let r = build_gemini_request("https://api.example.com", "m", "k", true, false, None);
        assert!(r.url.contains("alt=sse"));
        assert!(r.stream);
    }
}
