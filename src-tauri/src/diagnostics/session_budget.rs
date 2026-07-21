//! Session-scoped host request budget and in-memory result cache.
//!
//! Budget is shared across all providers in a single diagnosis run for the same
//! scheme + host + effective port (max 30 real HTTP sends; two consecutive
//! RATE_LIMITED responses stop further requests to that host).

use crate::ccs_adapter::ProtocolKind;
use crate::protocols::types::{AttemptResult, RequestPurpose, TokenLimitField};
use crate::security::origin::SameOriginPolicy;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

pub const MAX_HOST_REQUESTS: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OriginKey {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl OriginKey {
    pub fn from_policy(policy: &SameOriginPolicy) -> Self {
        let port = policy.port.unwrap_or(match policy.scheme.as_str() {
            "https" => 443,
            "http" => 80,
            _ => 0,
        });
        Self {
            scheme: policy.scheme.clone(),
            host: policy.host.to_ascii_lowercase(),
            port,
        }
    }

    pub fn from_base_url(base_url: &str) -> Option<Self> {
        SameOriginPolicy::parse_url(base_url).map(|p| Self::from_policy(&p))
    }
}

#[derive(Debug, Clone, Default)]
pub struct HostBudget {
    pub sent: usize,
    pub consecutive_rate_limits: usize,
    pub stopped_reason: Option<BudgetStopReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetStopReason {
    MaxRequests,
    ConsecutiveRateLimits,
}

impl BudgetStopReason {
    pub fn message(self) -> &'static str {
        match self {
            Self::MaxRequests => "已停止继续请求：该 Host 在本次诊断会话中已达到 30 次请求上限。",
            Self::ConsecutiveRateLimits => {
                "已停止继续请求：该 Host 连续两次返回限流响应，避免进一步消耗配额或触发封禁。"
            }
        }
    }

    pub fn classification(self) -> &'static str {
        match self {
            Self::MaxRequests => "HOST_BUDGET_EXHAUSTED",
            Self::ConsecutiveRateLimits => "HOST_RATE_LIMIT_STOPPED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestCacheKey {
    pub origin: OriginKey,
    /// Irreversible short fingerprint of the API key (never the raw key).
    pub key_fingerprint: String,
    pub protocol: ProtocolKind,
    pub model: String,
    pub purpose: RequestPurpose,
    pub stream: bool,
    pub tool_call: bool,
    pub token_limit_field: TokenLimitField,
}

pub fn key_fingerprint(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

/// Shared across all providers in one diagnosis run.
pub struct SessionBudget {
    inner: Mutex<SessionBudgetInner>,
}

#[derive(Default)]
struct SessionBudgetInner {
    hosts: HashMap<OriginKey, HostBudget>,
    cache: HashMap<RequestCacheKey, AttemptResult>,
}

impl SessionBudget {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SessionBudgetInner::default()),
        }
    }

    /// Look up a previously completed identical request combination.
    pub fn get_cached(&self, key: &RequestCacheKey) -> Option<AttemptResult> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.cache.get(key).cloned().map(|mut r| {
            r.reused_from_cache = true;
            r.http_sent = false;
            r.suggestion_note = Some(
                "本次结果复用了同一会话内相同配置组合的已完成请求，未重复发送 HTTP 请求。".into(),
            );
            r
        })
    }

    /// Atomically reserve a send slot for `origin` if budget allows.
    pub fn try_reserve_send(&self, origin: &OriginKey) -> Result<(), BudgetStopReason> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let budget = guard.hosts.entry(origin.clone()).or_default();
        if let Some(reason) = budget.stopped_reason {
            return Err(reason);
        }
        if budget.sent >= MAX_HOST_REQUESTS {
            budget.stopped_reason = Some(BudgetStopReason::MaxRequests);
            return Err(BudgetStopReason::MaxRequests);
        }
        if budget.consecutive_rate_limits >= 2 {
            budget.stopped_reason = Some(BudgetStopReason::ConsecutiveRateLimits);
            return Err(BudgetStopReason::ConsecutiveRateLimits);
        }
        // Reserve before send so concurrent providers cannot overshoot.
        budget.sent += 1;
        Ok(())
    }

    /// Release a previously reserved slot when the HTTP request was never actually sent.
    pub fn release_unsent(&self, origin: &OriginKey) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(budget) = guard.hosts.get_mut(origin) {
            if budget.sent > 0 {
                budget.sent -= 1;
            }
            if budget.sent < MAX_HOST_REQUESTS
                && budget.stopped_reason == Some(BudgetStopReason::MaxRequests)
            {
                budget.stopped_reason = None;
            }
        }
    }

    pub fn record_result(&self, origin: &OriginKey, classification: &str) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let budget = guard.hosts.entry(origin.clone()).or_default();
        if classification == "RATE_LIMITED" {
            budget.consecutive_rate_limits += 1;
            if budget.consecutive_rate_limits >= 2 {
                budget.stopped_reason = Some(BudgetStopReason::ConsecutiveRateLimits);
            }
        } else {
            budget.consecutive_rate_limits = 0;
        }
    }

    pub fn store_cache(&self, key: RequestCacheKey, result: AttemptResult) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.cache.entry(key).or_insert(result);
    }

    pub fn sent_for(&self, origin: &OriginKey) -> usize {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.hosts.get(origin).map(|h| h.sent).unwrap_or(0)
    }

    pub fn stop_reason(&self, origin: &OriginKey) -> Option<BudgetStopReason> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.hosts.get(origin).and_then(|h| h.stopped_reason)
    }
}

impl Default for SessionBudget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn origin(host: &str) -> OriginKey {
        OriginKey {
            scheme: "https".into(),
            host: host.into(),
            port: 443,
        }
    }

    fn cache_key(host: &str, key_fp: &str, model: &str) -> RequestCacheKey {
        RequestCacheKey {
            origin: origin(host),
            key_fingerprint: key_fp.into(),
            protocol: ProtocolKind::OpenAiChat,
            model: model.into(),
            purpose: RequestPurpose::Generate,
            stream: false,
            tool_call: false,
            token_limit_field: TokenLimitField::MaxCompletionTokens,
        }
    }

    #[test]
    fn two_providers_share_thirty_budget() {
        let budget = SessionBudget::new();
        let o = origin("api.example.com");
        for _ in 0..30 {
            assert!(budget.try_reserve_send(&o).is_ok());
        }
        assert_eq!(budget.sent_for(&o), 30);
        assert_eq!(
            budget.try_reserve_send(&o),
            Err(BudgetStopReason::MaxRequests)
        );
    }

    #[test]
    fn concurrent_three_cannot_exceed_thirty() {
        let budget = Arc::new(SessionBudget::new());
        let o = origin("api.example.com");
        let mut handles = Vec::new();
        for _ in 0..3 {
            let b = Arc::clone(&budget);
            let o = o.clone();
            handles.push(thread::spawn(move || {
                let mut ok = 0usize;
                for _ in 0..20 {
                    if b.try_reserve_send(&o).is_ok() {
                        ok += 1;
                    }
                }
                ok
            }));
        }
        let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total, 30);
        assert_eq!(budget.sent_for(&o), 30);
    }

    #[test]
    fn different_hosts_have_independent_budgets() {
        let budget = SessionBudget::new();
        let a = origin("a.example.com");
        let b = origin("b.example.com");
        for _ in 0..30 {
            assert!(budget.try_reserve_send(&a).is_ok());
        }
        assert!(budget.try_reserve_send(&b).is_ok());
        assert_eq!(budget.sent_for(&a), 30);
        assert_eq!(budget.sent_for(&b), 1);
    }

    #[test]
    fn consecutive_two_429_stops_host() {
        let budget = SessionBudget::new();
        let o = origin("api.example.com");
        assert!(budget.try_reserve_send(&o).is_ok());
        budget.record_result(&o, "RATE_LIMITED");
        assert!(budget.try_reserve_send(&o).is_ok());
        budget.record_result(&o, "RATE_LIMITED");
        assert_eq!(
            budget.try_reserve_send(&o),
            Err(BudgetStopReason::ConsecutiveRateLimits)
        );
    }

    #[test]
    fn non_429_resets_consecutive_counter() {
        let budget = SessionBudget::new();
        let o = origin("api.example.com");
        assert!(budget.try_reserve_send(&o).is_ok());
        budget.record_result(&o, "RATE_LIMITED");
        assert!(budget.try_reserve_send(&o).is_ok());
        budget.record_result(&o, "GENERATE_OK");
        assert!(budget.try_reserve_send(&o).is_ok());
        budget.record_result(&o, "RATE_LIMITED");
        assert!(budget.try_reserve_send(&o).is_ok());
    }

    #[test]
    fn cache_hits_same_combo() {
        let budget = SessionBudget::new();
        let key = cache_key("api.example.com", "fp-a", "gpt-test");
        let mut result = AttemptResult::network_error(
            ProtocolKind::OpenAiChat,
            "gpt-test",
            "https://api.example.com/v1/chat/completions",
            "ok",
            10,
        );
        result.ok = true;
        result.classification = "GENERATE_OK".into();
        budget.store_cache(key.clone(), result);
        let hit = budget.get_cached(&key).expect("cache hit");
        assert!(hit.reused_from_cache);
        assert!(hit.ok);
        let mut other = AttemptResult::network_error(
            ProtocolKind::OpenAiChat,
            "gpt-test",
            "https://api.example.com/v1/chat/completions",
            "other",
            1,
        );
        other.ok = false;
        budget.store_cache(key.clone(), other);
        let hit2 = budget.get_cached(&key).unwrap();
        assert!(hit2.ok);
    }

    #[test]
    fn different_key_fingerprint_not_reused() {
        let budget = SessionBudget::new();
        let key_a = cache_key("api.example.com", "fp-a", "m");
        let key_b = cache_key("api.example.com", "fp-b", "m");
        let mut result = AttemptResult::network_error(
            ProtocolKind::OpenAiChat,
            "m",
            "https://api.example.com",
            "ok",
            1,
        );
        result.ok = true;
        result.classification = "GENERATE_OK".into();
        budget.store_cache(key_a, result);
        assert!(budget.get_cached(&key_b).is_none());
    }

    #[test]
    fn key_fingerprint_is_stable_and_not_raw_key() {
        let fp = key_fingerprint("sk-secret-value-12345");
        assert_eq!(fp.len(), 16);
        assert!(!fp.contains("sk-secret"));
        assert_eq!(fp, key_fingerprint("sk-secret-value-12345"));
        assert_ne!(fp, key_fingerprint("sk-other"));
    }

    #[test]
    fn release_unsent_returns_slot() {
        let budget = SessionBudget::new();
        let o = origin("api.example.com");
        for _ in 0..30 {
            assert!(budget.try_reserve_send(&o).is_ok());
        }
        assert!(budget.try_reserve_send(&o).is_err());
        budget.release_unsent(&o);
        assert!(budget.try_reserve_send(&o).is_ok());
    }
}
