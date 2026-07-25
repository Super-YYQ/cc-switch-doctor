use crate::ccs_adapter::{scan_database, DiscoveryInfo, NormalizedProvider, ProviderScanView};
use crate::diagnostics::StartDiagnosisRequest;
use crate::error::{PublicError, PublicResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

pub struct AppState {
    inner: Mutex<InnerState>,
}

struct InnerState {
    db_path: Option<PathBuf>,
    scan: Option<ProviderScanView>,
    /// opaque_id -> provider (secrets in memory only)
    providers: HashMap<String, NormalizedProvider>,
    active_cancel: Option<CancellationToken>,
    active_run_id: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(InnerState {
                db_path: None,
                scan: None,
                providers: HashMap::new(),
                active_cancel: None,
                active_run_id: None,
            }),
        }
    }

    pub fn discover_and_scan(&self) -> PublicResult<ProviderScanView> {
        let manual = {
            let g = self
                .inner
                .lock()
                .map_err(|_| PublicError::internal("lock"))?;
            g.db_path.clone()
        };
        let (view, normalized) = scan_database(manual.as_deref())?;
        let mut g = self
            .inner
            .lock()
            .map_err(|_| PublicError::internal("lock"))?;
        if let Some(p) = view.discovery.database_path.clone() {
            g.db_path = Some(p);
        }
        g.providers.clear();
        for n in normalized {
            g.providers.insert(n.opaque_id.clone(), n);
        }
        g.scan = Some(view.clone());
        Ok(view)
    }

    pub fn set_db_path(&self, path: PathBuf) -> PublicResult<ProviderScanView> {
        {
            let mut g = self
                .inner
                .lock()
                .map_err(|_| PublicError::internal("lock"))?;
            g.db_path = Some(path);
        }
        self.discover_and_scan()
    }

    pub fn current_scan(&self) -> PublicResult<ProviderScanView> {
        let g = self
            .inner
            .lock()
            .map_err(|_| PublicError::internal("lock"))?;
        g.scan
            .clone()
            .ok_or_else(|| PublicError::NotFound("尚未扫描，请先刷新配置".into()))
    }

    pub fn discovery(&self) -> PublicResult<DiscoveryInfo> {
        let g = self
            .inner
            .lock()
            .map_err(|_| PublicError::internal("lock"))?;
        if let Some(s) = &g.scan {
            return Ok(s.discovery.clone());
        }
        Ok(crate::ccs_adapter::discover_database_paths())
    }

    pub fn take_providers_for(&self, ids: &[String]) -> PublicResult<Vec<NormalizedProvider>> {
        let g = self
            .inner
            .lock()
            .map_err(|_| PublicError::internal("lock"))?;
        let mut out = Vec::new();
        for id in ids {
            if let Some(p) = g.providers.get(id) {
                if !p.is_selectable() {
                    return Err(PublicError::InvalidRequest(format!(
                        "配置不可测试：{}",
                        p.display_name
                    )));
                }
                // clone secrets into new SecretString
                out.push(NormalizedProvider {
                    opaque_id: p.opaque_id.clone(),
                    source_id: p.source_id.clone(),
                    app_type: p.app_type,
                    display_name: p.display_name.clone(),
                    category: p.category.clone(),
                    auth_kind: p.auth_kind,
                    provider_kind: p.provider_kind,
                    base_url: p.base_url.clone(),
                    api_key: secrecy::SecretString::from(
                        secrecy::ExposeSecret::expose_secret(&p.api_key).to_string(),
                    ),
                    configured_protocol: p.configured_protocol,
                    configured_model: p.configured_model.clone(),
                    model_candidates: p.model_candidates.clone(), // ModelCandidate list
                    endpoint_candidates: p.endpoint_candidates.clone(),
                    custom_user_agent: p.custom_user_agent.clone(),
                    needs_local_routing: p.needs_local_routing,
                    is_current: p.is_current,
                    skip_reason: p.skip_reason.clone(),
                    masked_key: p.masked_key.clone(),
                    safe_base_url: p.safe_base_url.clone(),
                    website_url: p.website_url.clone(),
                    api_format_hint: p.api_format_hint.clone(),
                    preferred_auth: p.preferred_auth,
                    credential_source: p.credential_source.clone(),
                });
            } else {
                return Err(PublicError::NotFound(format!("未知配置 ID：{id}")));
            }
        }
        if out.is_empty() {
            return Err(PublicError::InvalidRequest("未选择任何配置".into()));
        }
        Ok(out)
    }

    pub fn begin_run(&self, run_id: String) -> PublicResult<CancellationToken> {
        let mut g = self
            .inner
            .lock()
            .map_err(|_| PublicError::internal("lock"))?;
        if let Some(prev) = g.active_cancel.take() {
            prev.cancel();
        }
        let token = CancellationToken::new();
        g.active_cancel = Some(token.clone());
        g.active_run_id = Some(run_id);
        Ok(token)
    }

    pub fn cancel_run(&self, run_id: &str) -> PublicResult<()> {
        let mut g = self
            .inner
            .lock()
            .map_err(|_| PublicError::internal("lock"))?;
        match &g.active_run_id {
            Some(id) if id == run_id => {
                if let Some(t) = g.active_cancel.take() {
                    t.cancel();
                }
                // keep run id until complete_run so late events can still match? Spec: cancel matching only.
                // Keep id until finished so frontend can wait for events; don't clear yet.
                Ok(())
            }
            Some(_) => Err(PublicError::InvalidRequest(
                "runId 不匹配当前活动诊断".into(),
            )),
            None => Ok(()), // no-op
        }
    }

    pub fn complete_run(&self, run_id: &str) {
        if let Ok(mut g) = self.inner.lock() {
            if g.active_run_id.as_deref() == Some(run_id) {
                g.active_cancel = None;
                g.active_run_id = None;
            }
        }
    }

    pub fn cancel_all(&self) {
        if let Ok(mut g) = self.inner.lock() {
            if let Some(t) = g.active_cancel.take() {
                t.cancel();
            }
            g.active_run_id = None;
        }
    }

    pub fn estimate(&self, req: &StartDiagnosisRequest) -> PublicResult<usize> {
        let providers = self.take_providers_for(&req.opaque_ids)?;
        let n: usize = providers
            .iter()
            .map(|p| crate::diagnostics::plan_attempts(p, req.mode).len())
            .sum();
        Ok(n)
    }
}

pub type SharedState = Arc<AppState>;
