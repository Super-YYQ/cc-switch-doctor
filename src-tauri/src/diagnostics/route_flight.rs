//! Per-app CCS route probe single-flight (async, non-blocking).
//!
//! Concurrent providers for the same App in one diagnosis run share at most one
//! real route leader. Waiters receive a clone of the leader's `RouteChannelSummary`.
//! The summary is App-level route evidence — never a Provider success conclusion.

use super::outcome::RouteChannelSummary;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::{oneshot, watch};

#[derive(Debug)]
pub enum RouteReservation {
    /// This task owns the real route probe for the App.
    Leader,
    /// Another task is probing; wait for summary via the oneshot receiver.
    Waiter(oneshot::Receiver<RouteChannelSummary>),
    /// A previous leader already finished; reuse its summary.
    AlreadyCompleted(RouteChannelSummary),
}

enum AppFlightState {
    InFlight {
        waiters: Vec<oneshot::Sender<RouteChannelSummary>>,
    },
    Completed(RouteChannelSummary),
}

/// Session-scoped single-flight coordinator for CCS route probes, keyed by app type.
pub struct RouteFlight {
    inner: Mutex<HashMap<String, AppFlightState>>,
    /// Optional cancellation broadcast (run cancelled).
    cancel_tx: watch::Sender<bool>,
}

impl RouteFlight {
    pub fn new() -> Self {
        let (cancel_tx, _) = watch::channel(false);
        Self {
            inner: Mutex::new(HashMap::new()),
            cancel_tx,
        }
    }

    pub fn cancel_all(&self) {
        let _ = self.cancel_tx.send(true);
    }

    /// Atomically reserve leadership for `app_key` or join as waiter / reuse result.
    pub fn reserve(&self, app_key: &str) -> RouteReservation {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match g.get_mut(app_key) {
            Some(AppFlightState::Completed(summary)) => {
                RouteReservation::AlreadyCompleted(summary.clone())
            }
            Some(AppFlightState::InFlight { waiters }) => {
                let (tx, rx) = oneshot::channel();
                waiters.push(tx);
                RouteReservation::Waiter(rx)
            }
            None => {
                g.insert(
                    app_key.to_string(),
                    AppFlightState::InFlight {
                        waiters: Vec::new(),
                    },
                );
                RouteReservation::Leader
            }
        }
    }

    /// Leader finished: store summary and wake all waiters.
    pub fn complete(&self, app_key: &str, summary: RouteChannelSummary) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let waiters = match g.remove(app_key) {
            Some(AppFlightState::InFlight { waiters }) => waiters,
            Some(AppFlightState::Completed(_)) => {
                // Idempotent: keep latest
                g.insert(app_key.to_string(), AppFlightState::Completed(summary));
                return;
            }
            None => {
                g.insert(app_key.to_string(), AppFlightState::Completed(summary));
                return;
            }
        };
        for tx in waiters {
            let _ = tx.send(summary.clone());
        }
        g.insert(app_key.to_string(), AppFlightState::Completed(summary));
    }

    /// Leader aborted without a usable result (cancel / build failure).
    /// Waiters receive a disposition-only summary so they do not hang.
    pub fn abandon(&self, app_key: &str, fallback: RouteChannelSummary) {
        self.complete(app_key, fallback);
    }
}

impl Default for RouteFlight {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::outcome::RouteDisposition;
    use std::sync::Arc;

    fn sample_summary(ok: bool) -> RouteChannelSummary {
        let mut s = RouteChannelSummary::with_disposition(
            RouteDisposition::Attempted,
            Some(if ok {
                "CCS_ROUTE_OK".into()
            } else {
                "CCS_ROUTE_FAILED".into()
            }),
        );
        s.attempted = true;
        s
    }

    #[tokio::test]
    async fn first_caller_is_leader_others_wait() {
        let flight = Arc::new(RouteFlight::new());
        let r1 = flight.reserve("claude");
        assert!(matches!(r1, RouteReservation::Leader));

        let f2 = Arc::clone(&flight);
        let waiter = tokio::spawn(async move {
            match f2.reserve("claude") {
                RouteReservation::Waiter(rx) => rx.await.ok(),
                other => panic!("expected waiter, got {other:?}"),
            }
        });

        // Give waiter a moment to register
        tokio::task::yield_now().await;
        flight.complete("claude", sample_summary(true));
        let got = waiter.await.unwrap().unwrap();
        assert!(got.attempted);
        assert_eq!(got.overall_status.as_deref(), Some("CCS_ROUTE_OK"));
    }

    #[tokio::test]
    async fn third_caller_reuses_completed() {
        let flight = RouteFlight::new();
        assert!(matches!(flight.reserve("codex"), RouteReservation::Leader));
        flight.complete("codex", sample_summary(false));
        match flight.reserve("codex") {
            RouteReservation::AlreadyCompleted(s) => {
                assert_eq!(s.overall_status.as_deref(), Some("CCS_ROUTE_FAILED"));
            }
            other => panic!("expected completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn concurrent_three_only_one_leader() {
        let flight = Arc::new(RouteFlight::new());
        let mut handles = Vec::new();
        for _ in 0..3 {
            let f = Arc::clone(&flight);
            handles.push(tokio::spawn(async move {
                match f.reserve("gemini") {
                    RouteReservation::Leader => {
                        // Simulate work
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        f.complete("gemini", sample_summary(true));
                        "leader"
                    }
                    RouteReservation::Waiter(rx) => {
                        let _ = rx.await;
                        "waiter"
                    }
                    RouteReservation::AlreadyCompleted(_) => "reuse",
                }
            }));
        }
        let mut roles = Vec::new();
        for h in handles {
            roles.push(h.await.unwrap());
        }
        let leaders = roles.iter().filter(|r| **r == "leader").count();
        assert_eq!(leaders, 1, "roles={roles:?}");
        assert_eq!(roles.len(), 3);
    }
}
