//! Layered response text extraction:
//! 1. Target protocol native parse
//! 2. Cross-protocol compatible parse
//! 3. Known wrapper unwrap (whitelist)
//! 4. Known text-field scan (bounded)

use super::anthropic::extract_anthropic_text;
use super::gemini::extract_gemini_text;
use super::openai_chat::extract_chat_text;
use super::openai_responses::extract_responses_text;
use crate::ccs_adapter::ProtocolKind;
use serde_json::Value;

const MAX_WRAPPER_DEPTH: usize = 3;
const MAX_TEXT_SCAN_NODES: usize = 64;
const MAX_TEXT_LEN: usize = 8 * 1024;

const WRAPPER_KEYS: &[&str] = &["data", "result", "response", "message", "payload"];
const TEXT_KEYS: &[&str] = &[
    "text",
    "content",
    "message",
    "output_text",
    "response",
    "answer",
    "result",
    "data",
];
const SENSITIVE_KEYS: &[&str] = &[
    "key",
    "api_key",
    "apikey",
    "token",
    "access_token",
    "authorization",
    "password",
    "secret",
    "header",
    "headers",
    "credential",
    "credentials",
];

#[derive(Debug, Clone)]
pub struct ParsedText {
    pub text: String,
    /// Protocol whose extractor produced the text (may differ from request target).
    pub matched_protocol: ProtocolKind,
    /// True when text came from a non-native protocol extractor.
    pub cross_protocol: bool,
    /// True when text came from wrapper / known-field fallback (not a full protocol shape).
    pub loose_field: bool,
}

fn try_protocol(kind: ProtocolKind, value: &Value) -> Option<String> {
    match kind {
        ProtocolKind::AnthropicMessages => extract_anthropic_text(value),
        ProtocolKind::OpenAiChat => extract_chat_text(value),
        ProtocolKind::OpenAiResponses => extract_responses_text(value),
        ProtocolKind::GeminiNative => extract_gemini_text(value),
        ProtocolKind::Unknown => None,
    }
}

fn fallback_order(target: ProtocolKind) -> &'static [ProtocolKind] {
    match target {
        ProtocolKind::AnthropicMessages => &[
            ProtocolKind::OpenAiChat,
            ProtocolKind::OpenAiResponses,
            ProtocolKind::GeminiNative,
        ],
        ProtocolKind::OpenAiChat => &[
            ProtocolKind::OpenAiResponses,
            ProtocolKind::AnthropicMessages,
            ProtocolKind::GeminiNative,
        ],
        ProtocolKind::OpenAiResponses => &[
            ProtocolKind::OpenAiChat,
            ProtocolKind::AnthropicMessages,
            ProtocolKind::GeminiNative,
        ],
        ProtocolKind::GeminiNative => &[
            ProtocolKind::OpenAiChat,
            ProtocolKind::AnthropicMessages,
            ProtocolKind::OpenAiResponses,
        ],
        ProtocolKind::Unknown => &[
            ProtocolKind::AnthropicMessages,
            ProtocolKind::OpenAiChat,
            ProtocolKind::OpenAiResponses,
            ProtocolKind::GeminiNative,
        ],
    }
}

/// Unwrap one level of known wrapper keys if present and object-shaped.
fn unwrap_wrappers(value: &Value, depth: usize) -> Vec<Value> {
    let mut out = vec![value.clone()];
    if depth == 0 {
        return out;
    }
    if let Some(obj) = value.as_object() {
        for key in WRAPPER_KEYS {
            if let Some(inner) = obj.get(*key) {
                if inner.is_object() || inner.is_array() {
                    out.extend(unwrap_wrappers(inner, depth - 1));
                }
            }
        }
    }
    out
}

fn is_sensitive_key(k: &str) -> bool {
    let lower = k.to_ascii_lowercase();
    SENSITIVE_KEYS.iter().any(|s| lower == *s || lower.contains(s))
}

fn collect_known_text(value: &Value, budget: &mut usize, depth: usize, out: &mut String) {
    if *budget == 0 || depth > MAX_WRAPPER_DEPTH || out.len() >= MAX_TEXT_LEN {
        return;
    }
    match value {
        Value::String(s) => {
            if !s.is_empty() && s.len() <= MAX_TEXT_LEN {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(s);
                *budget = budget.saturating_sub(1);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_known_text(item, budget, depth + 1, out);
                if *budget == 0 || out.len() >= MAX_TEXT_LEN {
                    break;
                }
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                if is_sensitive_key(k) {
                    continue;
                }
                if TEXT_KEYS.iter().any(|t| k.eq_ignore_ascii_case(t)) {
                    collect_known_text(v, budget, depth + 1, out);
                } else if v.is_object() || v.is_array() {
                    // Only descend one more level into non-text containers for known keys
                    if depth < 2 {
                        collect_known_text(v, budget, depth + 1, out);
                    }
                }
                if *budget == 0 || out.len() >= MAX_TEXT_LEN {
                    break;
                }
            }
        }
        _ => {}
    }
}

/// Extract model text with layered fallbacks. Returns None only when no usable text found.
pub fn extract_response_text(target: ProtocolKind, value: &Value) -> Option<ParsedText> {
    if value.is_null() {
        return None;
    }
    // Layer 1: native
    if let Some(text) = try_protocol(target, value) {
        return Some(ParsedText {
            text,
            matched_protocol: target,
            cross_protocol: false,
            loose_field: false,
        });
    }

    // Candidates: root + wrappers
    let candidates = unwrap_wrappers(value, MAX_WRAPPER_DEPTH);

    // Layer 1 again on wrappers with native protocol
    for c in &candidates {
        if let Some(text) = try_protocol(target, c) {
            return Some(ParsedText {
                text,
                matched_protocol: target,
                cross_protocol: false,
                loose_field: false,
            });
        }
    }

    // Layer 2: cross-protocol on root + wrappers
    for alt in fallback_order(target) {
        for c in &candidates {
            if let Some(text) = try_protocol(*alt, c) {
                return Some(ParsedText {
                    text,
                    matched_protocol: *alt,
                    cross_protocol: true,
                    loose_field: false,
                });
            }
        }
    }

    // Layer 3/4: known text fields (bounded)
    for c in &candidates {
        let mut budget = MAX_TEXT_SCAN_NODES;
        let mut text = String::new();
        collect_known_text(c, &mut budget, 0, &mut text);
        let text = text.trim().to_string();
        if !text.is_empty() {
            return Some(ParsedText {
                text,
                matched_protocol: target,
                cross_protocol: false,
                loose_field: true,
            });
        }
    }

    None
}

/// Protocol label for UI / suggestion notes.
pub fn protocol_label(kind: ProtocolKind) -> &'static str {
    match kind {
        ProtocolKind::AnthropicMessages => "Anthropic Messages",
        ProtocolKind::OpenAiChat => "OpenAI Chat Completions",
        ProtocolKind::OpenAiResponses => "OpenAI Responses",
        ProtocolKind::GeminiNative => "Gemini Native",
        ProtocolKind::Unknown => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn anthropic_native_success() {
        let v = json!({
            "content":[{"type":"text","text":"CCS_DOCTOR_OK"}],
            "usage":{"billing_usage":{"source":"oai_chat"}}
        });
        let p = extract_response_text(ProtocolKind::AnthropicMessages, &v).unwrap();
        assert_eq!(p.text, "CCS_DOCTOR_OK");
        assert!(!p.cross_protocol);
    }

    #[test]
    fn anthropic_endpoint_returns_openai() {
        let v = json!({"choices":[{"message":{"content":"CCS_DOCTOR_OK"}}]});
        let p = extract_response_text(ProtocolKind::AnthropicMessages, &v).unwrap();
        assert_eq!(p.text, "CCS_DOCTOR_OK");
        assert!(p.cross_protocol);
        assert_eq!(p.matched_protocol, ProtocolKind::OpenAiChat);
    }

    #[test]
    fn openai_endpoint_returns_anthropic() {
        let v = json!({"content":[{"type":"text","text":"CCS_DOCTOR_OK"}]});
        let p = extract_response_text(ProtocolKind::OpenAiChat, &v).unwrap();
        assert_eq!(p.text, "CCS_DOCTOR_OK");
        assert!(p.cross_protocol);
        assert_eq!(p.matched_protocol, ProtocolKind::AnthropicMessages);
    }

    #[test]
    fn wrapper_data_content() {
        let v = json!({"data":{"choices":[{"message":{"content":"CCS_DOCTOR_OK"}}]}});
        let p = extract_response_text(ProtocolKind::OpenAiChat, &v).unwrap();
        assert_eq!(p.text, "CCS_DOCTOR_OK");
    }

    #[test]
    fn loose_text_field() {
        let v = json!({"answer":"hello world from proxy"});
        let p = extract_response_text(ProtocolKind::OpenAiChat, &v).unwrap();
        assert!(p.loose_field);
        assert!(p.text.contains("hello world"));
    }

    #[test]
    fn does_not_scan_api_key_fields() {
        let v = json!({"api_key":"sk-should-not-appear","foo":1});
        assert!(extract_response_text(ProtocolKind::OpenAiChat, &v).is_none());
    }
}
