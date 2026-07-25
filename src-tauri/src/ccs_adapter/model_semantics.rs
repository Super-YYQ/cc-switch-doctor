//! Minimal model-semantics helpers aligned with CC Switch upstream.
//!
//! Source of truth (do not invent extra markers):
//! - farion1231/cc-switch @ 878c26f31e012ba32b9772bd080bd4fa9e7d495e
//! - `src-tauri/src/proxy/model_mapper.rs` (`strip_one_m_suffix_for_upstream`)
//! - `src/components/providers/forms/hooks/useModelState.ts` (`stripClaudeOneMMarker`)
//! - `src-tauri/src/claude_desktop_config.rs` (`ONE_M_CONTEXT_MARKER = "[1m]"`)

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Claude Code / CC Switch local 1M-context capability marker.
/// Matching is case-insensitive; the constant itself is lowercase.
pub const ONE_M_CONTEXT_MARKER: &str = "[1m]";

/// Where a model candidate came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelCandidateSource {
    /// Current configured model, no local-marker rewrite needed.
    ConfiguredModel,
    /// Current configured model with only the local `[1M]` marker stripped.
    LocalMarkerNormalized,
    /// Explicit role mapping from the current Provider settings.
    ConfiguredRoleMapping,
    /// Model discovered from the same host `/models` (or equivalent).
    DiscoveredModel,
    /// Conservative built-in Doctor guess (no configured model).
    DoctorGuess,
}

impl ModelCandidateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConfiguredModel => "configured_model",
            Self::LocalMarkerNormalized => "local_marker_normalized",
            Self::ConfiguredRoleMapping => "configured_role_mapping",
            Self::DiscoveredModel => "discovered_model",
            Self::DoctorGuess => "doctor_guess",
        }
    }

    /// True when success of this candidate means the current CC Switch config works.
    pub fn is_current_config_equivalent(self) -> bool {
        matches!(self, Self::ConfiguredModel | Self::LocalMarkerNormalized)
    }
}

/// Small structured model candidate (replaces bare `String` lists).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCandidate {
    /// Value shown / stored as the configured model (may keep `[1M]`).
    pub display_model: String,
    /// Value placed into the upstream HTTP request body.
    pub wire_model: String,
    pub source: ModelCandidateSource,
    /// Runtime-equivalent to the current CC Switch configuration.
    pub equivalent_to_current: bool,
}

impl ModelCandidate {
    pub fn new(
        display_model: impl Into<String>,
        wire_model: impl Into<String>,
        source: ModelCandidateSource,
        equivalent_to_current: bool,
    ) -> Self {
        Self {
            display_model: display_model.into(),
            wire_model: wire_model.into(),
            source,
            equivalent_to_current,
        }
    }

    /// Build the current-config candidate for Claude-family apps.
    /// Strips a trailing `[1M]` marker (case-insensitive) for the wire model.
    pub fn from_configured_claude(display: &str) -> Self {
        let display = display.trim();
        let stripped = strip_claude_one_m_marker(display);
        if stripped.as_ref() == display {
            Self::new(
                display.to_string(),
                display.to_string(),
                ModelCandidateSource::ConfiguredModel,
                true,
            )
        } else {
            Self::new(
                display.to_string(),
                stripped.into_owned(),
                ModelCandidateSource::LocalMarkerNormalized,
                true,
            )
        }
    }

    /// Build a current-config candidate for non-Claude apps (no marker rewrite).
    pub fn from_configured_plain(display: &str) -> Self {
        let display = display.trim();
        Self::new(
            display.to_string(),
            display.to_string(),
            ModelCandidateSource::ConfiguredModel,
            true,
        )
    }

    pub fn role_mapping(display: &str, wire: &str) -> Self {
        Self::new(
            display.trim().to_string(),
            wire.trim().to_string(),
            ModelCandidateSource::ConfiguredRoleMapping,
            false,
        )
    }

    pub fn discovered(model: &str) -> Self {
        let m = model.trim();
        Self::new(
            m.to_string(),
            m.to_string(),
            ModelCandidateSource::DiscoveredModel,
            false,
        )
    }

    pub fn doctor_guess(model: &str) -> Self {
        let m = model.trim();
        Self::new(
            m.to_string(),
            m.to_string(),
            ModelCandidateSource::DoctorGuess,
            false,
        )
    }

    pub fn transform_label(&self) -> Option<&'static str> {
        match self.source {
            ModelCandidateSource::LocalMarkerNormalized => Some("CCS 本地 [1M] 上下文标记归一化"),
            ModelCandidateSource::ConfiguredRoleMapping => {
                Some("当前 Provider 配置中的角色模型映射")
            }
            ModelCandidateSource::DiscoveredModel => Some("从 /models 发现的模型"),
            ModelCandidateSource::DoctorGuess => Some("Doctor 推测模型"),
            ModelCandidateSource::ConfiguredModel => None,
        }
    }
}

/// Strip a trailing Claude/CCS `[1M]` local capability marker (case-insensitive).
///
/// Matches upstream `strip_one_m_suffix_for_upstream`:
/// - only the **suffix** marker is removed
/// - trailing whitespace after the base name is trimmed
/// - mid-string `[1M]` is left untouched
/// - other bracket suffixes (e.g. `[128K]`) are left untouched
pub fn strip_claude_one_m_marker(model: &str) -> Cow<'_, str> {
    let trimmed = model.trim_end();
    let marker = ONE_M_CONTEXT_MARKER.as_bytes();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= marker.len()
        && bytes[bytes.len() - marker.len()..].eq_ignore_ascii_case(marker)
    {
        let base = trimmed[..trimmed.len() - marker.len()].trim_end();
        return Cow::Owned(base.to_string());
    }
    if trimmed == model {
        Cow::Borrowed(model)
    } else {
        // Only trailing whitespace differed; return borrowed original when equal
        // to the untrimmed form's base... keep simple: return trimmed borrowed slice
        // only if it is a subslice of model (always true for trim_end).
        if let Some(stripped_ws) = model.strip_suffix(|c: char| c.is_whitespace()) {
            if stripped_ws == trimmed {
                return Cow::Borrowed(stripped_ws);
            }
        }
        Cow::Owned(trimmed.to_string())
    }
}

/// True when `wire` is the same as `configured` after local-marker normalization
/// and case-insensitive compare.
pub fn models_equivalent_after_local_normalize(configured: &str, wire: &str) -> bool {
    let a = strip_claude_one_m_marker(configured.trim());
    let b = strip_claude_one_m_marker(wire.trim());
    a.eq_ignore_ascii_case(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_uppercase_one_m() {
        assert_eq!(strip_claude_one_m_marker("GLM-5.2[1M]").as_ref(), "GLM-5.2");
    }

    #[test]
    fn strips_lowercase_one_m() {
        assert_eq!(strip_claude_one_m_marker("GLM-5.2[1m]").as_ref(), "GLM-5.2");
    }

    #[test]
    fn strips_trailing_space_after_marker() {
        assert_eq!(
            strip_claude_one_m_marker("GLM-5.2[1M]  ").as_ref(),
            "GLM-5.2"
        );
    }

    #[test]
    fn strips_space_before_marker() {
        // Upstream trims only the end of the whole string, then strips marker;
        // "GLM-5.2 [1M]" ends with [1M], base becomes "GLM-5.2 " → trim_end → "GLM-5.2"
        assert_eq!(
            strip_claude_one_m_marker("GLM-5.2 [1M]").as_ref(),
            "GLM-5.2"
        );
    }

    #[test]
    fn leaves_plain_model() {
        assert_eq!(strip_claude_one_m_marker("GLM-5.2").as_ref(), "GLM-5.2");
    }

    #[test]
    fn does_not_strip_mid_marker() {
        assert_eq!(
            strip_claude_one_m_marker("GLM-[1M]-TEST").as_ref(),
            "GLM-[1M]-TEST"
        );
    }

    #[test]
    fn does_not_strip_other_brackets() {
        assert_eq!(
            strip_claude_one_m_marker("GLM-5.2[128K]").as_ref(),
            "GLM-5.2[128K]"
        );
    }

    #[test]
    fn configured_claude_marks_local_normalized() {
        let c = ModelCandidate::from_configured_claude("GLM-5.2[1M]");
        assert_eq!(c.display_model, "GLM-5.2[1M]");
        assert_eq!(c.wire_model, "GLM-5.2");
        assert_eq!(c.source, ModelCandidateSource::LocalMarkerNormalized);
        assert!(c.equivalent_to_current);
    }

    #[test]
    fn configured_claude_plain_is_configured() {
        let c = ModelCandidate::from_configured_claude("GLM-5.2");
        assert_eq!(c.source, ModelCandidateSource::ConfiguredModel);
        assert_eq!(c.wire_model, "GLM-5.2");
        assert!(c.equivalent_to_current);
    }

    #[test]
    fn equivalence_ignores_case_and_marker() {
        assert!(models_equivalent_after_local_normalize(
            "GLM-5.2[1M]",
            "glm-5.2"
        ));
        assert!(!models_equivalent_after_local_normalize(
            "model-a", "model-b"
        ));
    }
}
