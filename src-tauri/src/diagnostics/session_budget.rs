//! Session-scoped host request budget and in-memory result cache.
//!
//! Budget is shared across all providers in a single diagnosis run for the same
//! scheme + host + effective port (max 30 real HTTP sends; two consecutive
//! RATE_LIMITED responses stop further requests to that host).
//!
//! Cache keys are derived from the fully built HTTP request (URL, method, auth
//! scheme, body fingerprint) so URL/auth/body variants never cross-reuse.

use crate::protocols::types::{
    AttemptResult, AuthScheme, BuiltRequest, RequestPurpose, TokenLimitField,
};
use crate::security::origin::SameOriginPolicy;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

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
    pub method: String,
    /// Final request URL without credentials (path + non-sensitive query shape).
    pub canonical_url: String,
    pub protocol: String,
    pub model: String,
    pub purpose: RequestPurpose,
    pub stream: bool,
    pub tool_call: bool,
    pub token_limit_field: Option<TokenLimitField>,
    pub auth_scheme: AuthScheme,
    pub user_agent_fingerprint: Option<String>,
    pub relevant_headers_fingerprint: String,
    pub request_body_fingerprint: String,
    /// Irreversible short fingerprint of the API key (never the raw key).
    pub key_fingerprint: String,
}

pub fn key_fingerprint(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Build a cache key from a fully constructed request. Never embeds secret values.
pub fn cache_key_from_built(
    origin: &OriginKey,
    built: &BuiltRequest,
    key_fp: &str,
    token_limit_field: Option<TokenLimitField>,
    auth_scheme: AuthScheme,
) -> RequestCacheKey {
    let canonical_url = canonicalize_url_for_cache(&built.url);
    let ua = built
        .headers
        .get("User-Agent")
        .or_else(|| built.headers.get("user-agent"))
        .map(|s| sha256_hex(s.as_bytes())[..16].to_string());

    // Non-sensitive headers only (sorted). Skip Authorization / api-key variants.
    let mut header_parts: Vec<String> = built
        .headers
        .iter()
        .filter(|(k, _)| {
            let kl = k.to_ascii_lowercase();
            !kl.contains("authorization")
                && !kl.contains("api-key")
                && !kl.contains("apikey")
                && !kl.contains("x-api-key")
                && !kl.contains("x-goog-api-key")
                && kl != "cookie"
        })
        .map(|(k, v)| {
            if k.eq_ignore_ascii_case("user-agent") {
                format!("{k}=<fp>")
            } else {
                format!("{k}={v}")
            }
        })
        .collect();
    header_parts.sort();
    let relevant_headers_fingerprint = sha256_hex(header_parts.join("\n").as_bytes());

    let body_fp = match &built.body {
        Some(v) => {
            // Stable JSON: serde_json Value Display is not sorted; use to_string which is deterministic for our builds.
            sha256_hex(v.to_string().as_bytes())
        }
        None => sha256_hex(b""),
    };

    RequestCacheKey {
        origin: origin.clone(),
        method: built.method.to_ascii_uppercase(),
        canonical_url,
        protocol: built.protocol.as_str().to_string(),
        model: built.model.clone(),
        purpose: built.purpose,
        stream: built.stream,
        tool_call: built.purpose == RequestPurpose::ToolCall,
        token_limit_field,
        auth_scheme,
        user_agent_fingerprint: ua,
        relevant_headers_fingerprint,
        request_body_fingerprint: body_fp,
        key_fingerprint: key_fp.to_string(),
    }
}

fn canonicalize_url_for_cache(raw: &str) -> String {
    let Ok(mut u) = url::Url::parse(raw) else {
        return raw.trim().to_string();
    };
    let _ = u.set_username("");
    let _ = u.set_password(None);
    // Keep path; scrub query values but keep keys (structure matters)
    let pairs: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, _)| (k.to_string(), "*".into()))
        .collect();
    if pairs.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut().clear();
        for (k, v) in pairs {
            u.query_pairs_mut().append_pair(&k, &v);
        }
    }
    u.set_fragment(None);
    u.to_string()
}

/// Shared across all providers in one diagnosis run.
pub struct SessionBudget {
    inner: Mutex<SessionBudgetInner>,
    /// Single-flight waiters for in-flight identical keys.
    flight: Mutex<HashMap<RequestCacheKey, Arc<FlightSlot>>>,
}

struct FlightSlot {
    done: Mutex<Option<AttemptResult>>,
    cv: Condvar,
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
            flight: Mutex::new(HashMap::new()),
        }
    }

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

    /// Single-flight: if another task is already sending this key, wait and reuse.
    /// Returns Ok(None) when caller should send; Ok(Some) when reused from peer.
    pub fn begin_or_wait_flight(&self, key: &RequestCacheKey) -> Option<AttemptResult> {
        // Fast path: already cached
        if let Some(c) = self.get_cached(key) {
            return Some(c);
        }

        let slot = {
            let mut flights = self.flight.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = flights.get(key) {
                Arc::clone(existing)
            } else {
                let s = Arc::new(FlightSlot {
                    done: Mutex::new(None),
                    cv: Condvar::new(),
                });
                flights.insert(key.clone(), Arc::clone(&s));
                return None; // we are the leader
            }
        };

        // Waiter path
        let mut done = slot.done.lock().unwrap_or_else(|e| e.into_inner());
        while done.is_none() {
            done = slot.cv.wait(done).unwrap_or_else(|e| e.into_inner());
        }
        done.clone().map(|mut r| {
            r.reused_from_cache = true;
            r.http_sent = false;
            r.suggestion_note = Some(
                "本次结果复用了同一会话内相同配置组合的已完成请求，未重复发送 HTTP 请求。".into(),
            );
            r
        })
    }

    pub fn finish_flight(&self, key: &RequestCacheKey, result: AttemptResult) {
        self.store_cache(key.clone(), result.clone());
        let mut flights = self.flight.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = flights.remove(key) {
            {
                let mut done = slot.done.lock().unwrap_or_else(|e| e.into_inner());
                *done = Some(result);
            }
            slot.cv.notify_all();
        }
    }

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
        budget.sent += 1;
        Ok(())
    }

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
    use crate::ccs_adapter::ProtocolKind;
    use serde_json::json;
    use std::collections::HashMap as StdMap;
    use std::thread;

    fn origin(host: &str) -> OriginKey {
        OriginKey {
            scheme: "https".into(),
            host: host.into(),
            port: 443,
        }
    }

    fn built(url: &str, method: &str, auth: AuthScheme) -> BuiltRequest {
        let mut headers = StdMap::new();
        headers.insert("Content-Type".into(), "application/json".into());
        match auth {
            AuthScheme::Bearer => {
                headers.insert("Authorization".into(), "Bearer sk-secret-key-value".into());
            }
            AuthScheme::XApiKey => {
                headers.insert("x-api-key".into(), "sk-secret-key-value".into());
            }
            AuthScheme::XGoogApiKey => {
                headers.insert("x-goog-api-key".into(), "sk-secret-key-value".into());
            }
            AuthScheme::QueryKey => {}
        }
        BuiltRequest {
            method: method.into(),
            url: url.into(),
            headers,
            body: Some(json!({"model":"m","stream":false})),
            stream: false,
            protocol: ProtocolKind::AnthropicMessages,
            model: "m".into(),
            purpose: RequestPurpose::Generate,
        }
    }

    #[test]
    fn different_paths_do_not_share_cache_key() {
        let o = origin("api.example.com");
        let a = cache_key_from_built(
            &o,
            &built(
                "https://api.example.com/messages",
                "POST",
                AuthScheme::Bearer,
            ),
            "fp",
            None,
            AuthScheme::Bearer,
        );
        let b = cache_key_from_built(
            &o,
            &built(
                "https://api.example.com/v1/messages",
                "POST",
                AuthScheme::Bearer,
            ),
            "fp",
            None,
            AuthScheme::Bearer,
        );
        assert_ne!(a.canonical_url, b.canonical_url);
        assert_ne!(a, b);
    }

    #[test]
    fn bearer_and_x_api_key_do_not_share() {
        let o = origin("api.example.com");
        let url = "https://api.example.com/v1/messages";
        let a = cache_key_from_built(
            &o,
            &built(url, "POST", AuthScheme::Bearer),
            "fp",
            None,
            AuthScheme::Bearer,
        );
        let b = cache_key_from_built(
            &o,
            &built(url, "POST", AuthScheme::XApiKey),
            "fp",
            None,
            AuthScheme::XApiKey,
        );
        assert_ne!(a.auth_scheme, b.auth_scheme);
        assert_ne!(a, b);
    }

    #[test]
    fn different_user_agents_do_not_share() {
        let o = origin("api.example.com");
        let mut b1 = built("https://api.example.com/v1/x", "POST", AuthScheme::Bearer);
        let mut b2 = b1.clone();
        b1.headers.insert("User-Agent".into(), "ua-a".into());
        b2.headers.insert("User-Agent".into(), "ua-b".into());
        let k1 = cache_key_from_built(&o, &b1, "fp", None, AuthScheme::Bearer);
        let k2 = cache_key_from_built(&o, &b2, "fp", None, AuthScheme::Bearer);
        assert_ne!(k1.user_agent_fingerprint, k2.user_agent_fingerprint);
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_token_fields_do_not_share() {
        let o = origin("api.example.com");
        let b = built(
            "https://api.example.com/v1/chat/completions",
            "POST",
            AuthScheme::Bearer,
        );
        let k1 = cache_key_from_built(
            &o,
            &b,
            "fp",
            Some(TokenLimitField::MaxCompletionTokens),
            AuthScheme::Bearer,
        );
        let k2 = cache_key_from_built(
            &o,
            &b,
            "fp",
            Some(TokenLimitField::MaxTokens),
            AuthScheme::Bearer,
        );
        assert_ne!(k1, k2);
    }

    #[test]
    fn identical_requests_reuse() {
        let budget = SessionBudget::new();
        let o = origin("api.example.com");
        let b = built("https://api.example.com/v1/x", "POST", AuthScheme::Bearer);
        let key = cache_key_from_built(&o, &b, "fp", None, AuthScheme::Bearer);
        let mut r = AttemptResult::network_error(
            ProtocolKind::OpenAiChat,
            "m",
            "https://api.example.com/v1/x",
            "ok",
            1,
        );
        r.ok = true;
        r.classification = "GENERATE_OK".into();
        budget.store_cache(key.clone(), r);
        let hit = budget.get_cached(&key).unwrap();
        assert!(hit.reused_from_cache);
        assert!(hit.ok);
        let s = format!("{key:?}");
        assert!(!s.contains("sk-secret"));
    }

    #[test]
    fn cache_key_never_contains_raw_secret() {
        let o = origin("api.example.com");
        let b = built("https://api.example.com/v1/x", "POST", AuthScheme::Bearer);
        let key = cache_key_from_built(&o, &b, "deadbeef", None, AuthScheme::Bearer);
        let dbg = format!("{key:?}");
        assert!(!dbg.contains("sk-secret-key-value"));
        assert!(!dbg.contains("Bearer sk-"));
    }

    #[test]
    fn concurrent_identical_requests_single_flight() {
        let budget = Arc::new(SessionBudget::new());
        let o = origin("api.example.com");
        let b = built("https://api.example.com/v1/x", "POST", AuthScheme::Bearer);
        let key = cache_key_from_built(&o, &b, "fp", None, AuthScheme::Bearer);

        let leader = budget.begin_or_wait_flight(&key);
        assert!(leader.is_none());

        let budget2 = Arc::clone(&budget);
        let key2 = key.clone();
        let waiter = thread::spawn(move || budget2.begin_or_wait_flight(&key2));

        // Give waiter time to park
        thread::sleep(std::time::Duration::from_millis(30));
        let mut r = AttemptResult::network_error(
            ProtocolKind::OpenAiChat,
            "m",
            "https://api.example.com/v1/x",
            "ok",
            1,
        );
        r.ok = true;
        r.classification = "GENERATE_OK".into();
        budget.finish_flight(&key, r);

        let got = waiter.join().unwrap().expect("waiter should reuse");
        assert!(got.reused_from_cache);
        assert!(got.ok);
    }

    #[test]
    fn two_providers_share_thirty_budget() {
        let budget = SessionBudget::new();
        let o = origin("api.example.com");
        for _ in 0..30 {
            assert!(budget.try_reserve_send(&o).is_ok());
        }
        assert_eq!(
            budget.try_reserve_send(&o),
            Err(BudgetStopReason::MaxRequests)
        );
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
}
