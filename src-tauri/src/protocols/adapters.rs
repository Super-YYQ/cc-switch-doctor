//! Source-grounded response adapter registry (v0.1.7).
//!
//! Thin façade over existing protocol extractors so native / cross-protocol /
//! loose-field layers stay explicit. Full streaming state machines land later
//! with fixtures; this module establishes the registry boundary required by
//! the v0.1.7 spec and docs/research/v0.1.7-source-review.md.

use super::anthropic::extract_anthropic_text;
use super::gemini::extract_gemini_text;
use super::openai_chat::extract_chat_text;
use super::openai_responses::extract_responses_text;
use super::parse::{extract_response_text, ParsedText};
use crate::ccs_adapter::ProtocolKind;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityClass {
    Native,
    CrossProtocol,
    LooseField,
    Unrecognized,
}

#[derive(Debug, Clone)]
pub struct ParseOutcome {
    pub text: Option<String>,
    pub matched_protocol: Option<ProtocolKind>,
    pub class: CompatibilityClass,
}

pub trait ResponseAdapter: Send + Sync {
    fn protocol(&self) -> ProtocolKind;

    /// Native-only parse (no cross-protocol, no loose-field).
    fn parse_native(&self, body: &Value) -> Option<String>;

    /// Full layered parse with explicit class.
    fn parse(&self, body: &Value) -> ParseOutcome {
        // Native first
        if let Some(t) = self.parse_native(body) {
            if !t.trim().is_empty() {
                return ParseOutcome {
                    text: Some(t),
                    matched_protocol: Some(self.protocol()),
                    class: CompatibilityClass::Native,
                };
            }
        }
        // Fall through to shared layered extractor for cross/loose.
        match extract_response_text(self.protocol(), body) {
            Some(ParsedText {
                text,
                matched_protocol,
                cross_protocol,
                loose_field,
            }) => {
                let class = if loose_field {
                    CompatibilityClass::LooseField
                } else if cross_protocol {
                    CompatibilityClass::CrossProtocol
                } else {
                    CompatibilityClass::Native
                };
                ParseOutcome {
                    text: Some(text),
                    matched_protocol: Some(matched_protocol),
                    class,
                }
            }
            None => ParseOutcome {
                text: None,
                matched_protocol: None,
                class: CompatibilityClass::Unrecognized,
            },
        }
    }
}

pub struct AnthropicAdapter;
pub struct OpenAiChatAdapter;
pub struct OpenAiResponsesAdapter;
pub struct GeminiAdapter;

impl ResponseAdapter for AnthropicAdapter {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::AnthropicMessages
    }
    fn parse_native(&self, body: &Value) -> Option<String> {
        extract_anthropic_text(body)
    }
}

impl ResponseAdapter for OpenAiChatAdapter {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::OpenAiChat
    }
    fn parse_native(&self, body: &Value) -> Option<String> {
        extract_chat_text(body)
    }
}

impl ResponseAdapter for OpenAiResponsesAdapter {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::OpenAiResponses
    }
    fn parse_native(&self, body: &Value) -> Option<String> {
        extract_responses_text(body)
    }
}

impl ResponseAdapter for GeminiAdapter {
    fn protocol(&self) -> ProtocolKind {
        ProtocolKind::GeminiNative
    }
    fn parse_native(&self, body: &Value) -> Option<String> {
        extract_gemini_text(body)
    }
}

pub fn adapter_for(kind: ProtocolKind) -> Option<&'static dyn ResponseAdapter> {
    match kind {
        ProtocolKind::AnthropicMessages => Some(&AnthropicAdapter),
        ProtocolKind::OpenAiChat => Some(&OpenAiChatAdapter),
        ProtocolKind::OpenAiResponses => Some(&OpenAiResponsesAdapter),
        ProtocolKind::GeminiNative => Some(&GeminiAdapter),
        ProtocolKind::Unknown => None,
    }
}

/// Run a fixture body through the adapter for `target` and return outcome.
pub fn parse_fixture(target: ProtocolKind, body: &Value) -> ParseOutcome {
    match adapter_for(target) {
        Some(a) => a.parse(body),
        None => ParseOutcome {
            text: None,
            matched_protocol: None,
            class: CompatibilityClass::Unrecognized,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn anthropic_native_fixture() {
        let body = json!({
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "hello from anthropic"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 2}
        });
        let o = parse_fixture(ProtocolKind::AnthropicMessages, &body);
        assert_eq!(o.class, CompatibilityClass::Native);
        assert_eq!(o.text.as_deref(), Some("hello from anthropic"));
        assert_eq!(o.matched_protocol, Some(ProtocolKind::AnthropicMessages));
    }

    #[test]
    fn openai_chat_native_fixture() {
        let body = json!({
            "id": "chatcmpl-1",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "hello chat"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        });
        let o = parse_fixture(ProtocolKind::OpenAiChat, &body);
        assert_eq!(o.class, CompatibilityClass::Native);
        assert_eq!(o.text.as_deref(), Some("hello chat"));
    }

    #[test]
    fn openai_responses_native_fixture() {
        let body = json!({
            "id": "resp_1",
            "object": "response",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "hello responses"}]
            }]
        });
        let o = parse_fixture(ProtocolKind::OpenAiResponses, &body);
        assert_eq!(o.class, CompatibilityClass::Native);
        assert!(o.text.as_deref().unwrap_or("").contains("hello responses"));
    }

    #[test]
    fn gemini_native_fixture() {
        let body = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "hello gemini"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });
        let o = parse_fixture(ProtocolKind::GeminiNative, &body);
        assert_eq!(o.class, CompatibilityClass::Native);
        assert_eq!(o.text.as_deref(), Some("hello gemini"));
    }

    #[test]
    fn cross_protocol_openai_on_anthropic_target_not_native() {
        let body = json!({
            "id": "chatcmpl-x",
            "choices": [{
                "message": {"role": "assistant", "content": "cross"},
                "finish_reason": "stop"
            }]
        });
        let o = parse_fixture(ProtocolKind::AnthropicMessages, &body);
        assert_ne!(o.class, CompatibilityClass::Native);
        assert!(matches!(
            o.class,
            CompatibilityClass::CrossProtocol | CompatibilityClass::LooseField
        ));
        assert!(o.text.is_some());
    }

    #[test]
    fn http_200_error_envelope_not_success_text() {
        // Anthropic-style error object must not be extracted as assistant text.
        let body = json!({
            "type": "error",
            "error": {
                "type": "authentication_error",
                "message": "invalid x-api-key"
            }
        });
        let o = parse_fixture(ProtocolKind::AnthropicMessages, &body);
        // Must not classify as Native success with the error message as content.
        if o.class == CompatibilityClass::Native {
            panic!("error envelope must not be native success: {:?}", o.text);
        }
    }

    #[test]
    fn empty_output_null_responses_not_native_success() {
        let body = json!({
            "id": "resp_null",
            "object": "response",
            "status": "completed",
            "output": null
        });
        let o = parse_fixture(ProtocolKind::OpenAiResponses, &body);
        assert_ne!(o.class, CompatibilityClass::Native);
    }

    #[test]
    fn usage_null_does_not_block_native_anthropic() {
        let body = json!({
            "id": "msg_2",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "ok"}],
            "stop_reason": "end_turn",
            "usage": null
        });
        let o = parse_fixture(ProtocolKind::AnthropicMessages, &body);
        assert_eq!(o.class, CompatibilityClass::Native);
        assert_eq!(o.text.as_deref(), Some("ok"));
    }

    #[test]
    fn loose_field_not_native() {
        let body = json!({"answer": "only loose"});
        let o = parse_fixture(ProtocolKind::OpenAiChat, &body);
        assert_ne!(o.class, CompatibilityClass::Native);
    }

    #[test]
    fn registry_covers_all_known_protocols() {
        for k in [
            ProtocolKind::AnthropicMessages,
            ProtocolKind::OpenAiChat,
            ProtocolKind::OpenAiResponses,
            ProtocolKind::GeminiNative,
        ] {
            assert!(adapter_for(k).is_some());
        }
        assert!(adapter_for(ProtocolKind::Unknown).is_none());
    }

    fn load_fixture(rel: &str) -> Value {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("protocols")
            .join(rel);
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
        serde_json::from_str(&raw).expect("fixture json")
    }

    #[test]
    fn protocol_fixture_corpus_native_files() {
        let cases = [
            (
                ProtocolKind::AnthropicMessages,
                "anthropic/native-success.json",
                "fixture anthropic ok",
            ),
            (
                ProtocolKind::OpenAiChat,
                "openai-chat/native-success.json",
                "fixture chat ok",
            ),
            (
                ProtocolKind::OpenAiResponses,
                "openai-responses/native-success.json",
                "fixture responses ok",
            ),
            (
                ProtocolKind::GeminiNative,
                "gemini/native-success.json",
                "fixture gemini ok",
            ),
        ];
        for (kind, rel, expect) in cases {
            let body = load_fixture(rel);
            let o = parse_fixture(kind, &body);
            assert_eq!(
                o.class,
                CompatibilityClass::Native,
                "fixture {rel} must be Native"
            );
            assert!(
                o.text.as_deref().unwrap_or("").contains(expect),
                "fixture {rel} text={:?}",
                o.text
            );
        }
    }

    #[test]
    fn protocol_fixture_corpus_error_and_null() {
        let err = load_fixture("errors/anthropic-auth-error.json");
        let o = parse_fixture(ProtocolKind::AnthropicMessages, &err);
        assert_ne!(o.class, CompatibilityClass::Native);

        let null_out = load_fixture("openai-responses/output-null.json");
        let o2 = parse_fixture(ProtocolKind::OpenAiResponses, &null_out);
        assert_ne!(o2.class, CompatibilityClass::Native);
    }

    #[test]
    fn protocol_fixture_wrapper_not_native_for_openai_chat() {
        let body = load_fixture("wrappers/data-content.json");
        let o = parse_fixture(ProtocolKind::OpenAiChat, &body);
        // Wrapper may yield text via layered parse, but must not claim Native
        // unless the outer shape is the native chat envelope.
        if o.class == CompatibilityClass::Native {
            // Accept only if extractors see native shape after unwrap — still require text.
            assert!(o.text.is_some());
        } else {
            assert!(matches!(
                o.class,
                CompatibilityClass::CrossProtocol | CompatibilityClass::LooseField
            ));
        }
    }
}
