//! Layered diagnosis outcome model (v0.1.7).
//!
//! Primary provider status is never a route disposition. Route metadata is
//! always carried separately so UI can show Direct vs CCS route independently.

use serde::{Deserialize, Serialize};

/// Why a CCS route business request was or was not executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RouteDisposition {
    /// Verify mode DirectOnly, or no routing view / not asked to verify route.
    #[default]
    NotRequested,
    /// CCS routing config not detected / app not taken over / no profile.
    NotConfigured,
    /// Config present but proxy not running / health unreachable.
    NotRunning,
    /// Provider is not the current CCS route target for its app.
    NotCurrentTarget,
    /// App type has no client-protocol route probe in this Doctor build.
    UnsupportedApp,
    /// Listen address is non-loopback; route verify blocked by security policy.
    BlockedNonLoopback,
    /// At least one real CCS route HTTP request was sent for this provider/app.
    Attempted,
}

impl RouteDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::NotConfigured => "not_configured",
            Self::NotRunning => "not_running",
            Self::NotCurrentTarget => "not_current_target",
            Self::UnsupportedApp => "unsupported_app",
            Self::BlockedNonLoopback => "blocked_non_loopback",
            Self::Attempted => "attempted",
        }
    }

    /// True when disposition alone must never become Provider primary status.
    pub fn is_auxiliary(self) -> bool {
        !matches!(self, Self::Attempted)
    }
}

/// Outcome of one capability (generate / streaming / tool-call).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityOutcome {
    pub attempted: bool,
    pub success: bool,
    pub status: String,
}

impl CapabilityOutcome {
    pub fn skipped(status: impl Into<String>) -> Self {
        Self {
            attempted: false,
            success: false,
            status: status.into(),
        }
    }

    pub fn from_ok(ok: bool, status: impl Into<String>) -> Self {
        Self {
            attempted: true,
            success: ok,
            status: status.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectChannelSummary {
    pub attempted: bool,
    pub status: String,
    pub success: bool,
    pub native_success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_attempt_index: Option<usize>,
}

impl DirectChannelSummary {
    pub fn not_attempted() -> Self {
        Self {
            attempted: false,
            status: "NOT_ATTEMPTED".into(),
            success: false,
            native_success: false,
            best_attempt_index: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteChannelSummary {
    pub disposition: RouteDisposition,
    pub attempted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate: Option<CapabilityOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streaming: Option<CapabilityOutcome>,
    /// Combined route status code when attempted; otherwise mirrors disposition label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_provider_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failover_count_before: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failover_count_after: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

impl RouteChannelSummary {
    pub fn not_requested() -> Self {
        Self {
            disposition: RouteDisposition::NotRequested,
            attempted: false,
            generate: None,
            streaming: None,
            overall_status: None,
            actual_provider_id: None,
            actual_provider_name: None,
            failover_count_before: None,
            failover_count_after: None,
            notice: None,
        }
    }

    pub fn with_disposition(disposition: RouteDisposition, overall_status: Option<String>) -> Self {
        Self {
            disposition,
            attempted: matches!(disposition, RouteDisposition::Attempted),
            generate: None,
            streaming: None,
            overall_status,
            actual_provider_id: None,
            actual_provider_name: None,
            failover_count_before: None,
            failover_count_after: None,
            notice: None,
        }
    }

    /// Legacy `route_status` string for UI chips (compat with v0.1.6 frontend).
    pub fn legacy_route_status_code(&self) -> Option<String> {
        if let Some(s) = &self.overall_status {
            return Some(s.clone());
        }
        match self.disposition {
            RouteDisposition::NotRequested => None,
            RouteDisposition::NotConfigured => Some("CCS_ROUTE_NOT_APPLICABLE".into()),
            RouteDisposition::NotRunning => Some("CCS_ROUTE_NOT_RUNNING".into()),
            RouteDisposition::NotCurrentTarget => Some("CCS_ROUTE_NOT_APPLICABLE".into()),
            RouteDisposition::UnsupportedApp => Some("CCS_ROUTE_NOT_APPLICABLE".into()),
            RouteDisposition::BlockedNonLoopback => Some("CCS_ROUTE_NOT_APPLICABLE".into()),
            RouteDisposition::Attempted => None,
        }
    }
}

/// Map Skip reason text from `route_applicable` into a precise disposition.
pub fn disposition_from_skip_message(msg: &str) -> RouteDisposition {
    let m = msg.to_ascii_lowercase();
    if m.contains("仅直连") || m.contains("direct") {
        return RouteDisposition::NotRequested;
    }
    if m.contains("loopback") || m.contains("非 loopback") || m.contains("非loopback") {
        return RouteDisposition::BlockedNonLoopback;
    }
    if m.contains("未运行") {
        return RouteDisposition::NotRunning;
    }
    if m.contains("无 ccs")
        || m.contains("路由状态不可用")
        || m.contains("未开启")
        || m.contains("未接管")
    {
        return RouteDisposition::NotConfigured;
    }
    RouteDisposition::NotConfigured
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auxiliary_dispositions_never_attempted() {
        for d in [
            RouteDisposition::NotRequested,
            RouteDisposition::NotConfigured,
            RouteDisposition::NotRunning,
            RouteDisposition::NotCurrentTarget,
            RouteDisposition::UnsupportedApp,
            RouteDisposition::BlockedNonLoopback,
        ] {
            assert!(d.is_auxiliary());
            assert!(!matches!(d, RouteDisposition::Attempted));
        }
        assert!(!RouteDisposition::Attempted.is_auxiliary());
    }

    #[test]
    fn skip_direct_only_is_not_requested() {
        assert_eq!(
            disposition_from_skip_message("验证方式：仅直连"),
            RouteDisposition::NotRequested
        );
    }

    #[test]
    fn skip_non_loopback_is_blocked() {
        assert_eq!(
            disposition_from_skip_message("监听地址非 loopback 或端口未知"),
            RouteDisposition::BlockedNonLoopback
        );
    }

    #[test]
    fn legacy_codes_for_not_running() {
        let r = RouteChannelSummary::with_disposition(
            RouteDisposition::NotRunning,
            Some("CCS_ROUTE_NOT_RUNNING".into()),
        );
        assert_eq!(
            r.legacy_route_status_code().as_deref(),
            Some("CCS_ROUTE_NOT_RUNNING")
        );
        assert!(!r.attempted);
    }
}
