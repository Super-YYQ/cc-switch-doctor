use crate::ccs_adapter::{DiscoveryInfo, ProviderScanView};
use crate::diagnostics::engine::{run_diagnosis, DiagnosisEvent};
use crate::diagnostics::route_planner::VerifyMode;
use crate::diagnostics::{estimate_attempts, DiagnosisMode, StartDiagnosisRequest};
use crate::error::{PublicError, PublicResult};
use crate::state::AppState;
use crate::updates::{check_updates_now, UpdateStatus};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub doctor_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunHandle {
    pub run_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateResult {
    pub estimated_attempts: usize,
    pub provider_count: usize,
    pub mode: DiagnosisMode,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "CC Switch Doctor".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        doctor_version: env!("CARGO_PKG_VERSION").into(),
    }
}

#[tauri::command]
pub fn discover_cc_switch(state: State<'_, AppState>) -> PublicResult<DiscoveryInfo> {
    state.discovery()
}

#[tauri::command]
pub fn scan_providers(state: State<'_, AppState>) -> PublicResult<ProviderScanView> {
    state.discover_and_scan()
}

#[tauri::command]
pub fn refresh_providers(state: State<'_, AppState>) -> PublicResult<ProviderScanView> {
    state.discover_and_scan()
}

#[tauri::command]
pub fn select_database(state: State<'_, AppState>, path: String) -> PublicResult<ProviderScanView> {
    let p = PathBuf::from(path);
    if !p.is_file() {
        return Err(PublicError::NotFound(format!(
            "文件不存在：{}",
            p.display()
        )));
    }
    // Only accept .db files by extension (soft check)
    if p.extension().and_then(|e| e.to_str()) != Some("db") {
        return Err(PublicError::InvalidRequest(
            "请选择 cc-switch.db SQLite 文件".into(),
        ));
    }
    state.set_db_path(p)
}

#[tauri::command]
pub fn estimate_diagnosis(
    state: State<'_, AppState>,
    request: StartDiagnosisRequest,
) -> PublicResult<EstimateResult> {
    let n = state.estimate(&request)?;
    Ok(EstimateResult {
        estimated_attempts: n,
        provider_count: request.opaque_ids.len(),
        mode: request.mode,
    })
}

#[tauri::command]
pub async fn start_diagnosis(
    app: AppHandle,
    state: State<'_, AppState>,
    request: StartDiagnosisRequest,
) -> PublicResult<RunHandle> {
    if request.opaque_ids.is_empty() {
        return Err(PublicError::InvalidRequest("请至少选择一个配置".into()));
    }
    if request.concurrency < 1 || request.concurrency > 3 {
        return Err(PublicError::InvalidRequest("并发数仅允许 1–3".into()));
    }

    // Ensure schema allows testing
    let scan = state
        .current_scan()
        .or_else(|_| state.discover_and_scan())?;
    if !scan.can_test {
        return Err(PublicError::UnsupportedSchema(
            scan.schema
                .map(|s| s.message)
                .unwrap_or_else(|| "当前 schema 不允许测试".into()),
        ));
    }

    let providers = state.take_providers_for(&request.opaque_ids)?;
    let run_id = Uuid::new_v4().to_string();
    let cancel = state.begin_run(run_id.clone())?;
    let mode = request.mode;
    let concurrency = request.concurrency;
    let verify_mode = VerifyMode::parse(request.verify_mode.as_deref().unwrap_or("auto"));
    let routing = scan.routing.clone();
    let app2 = app.clone();
    let app_for_complete = app.clone();
    let rid = run_id.clone();
    let rid_complete = run_id.clone();
    tauri::async_runtime::spawn(async move {
        run_diagnosis(
            rid,
            providers,
            mode,
            concurrency,
            cancel,
            move |event| {
                let _ = app2.emit("diagnosis_event", event);
            },
            routing,
            verify_mode,
        )
        .await;
        // best-effort clear active run; ignore if app shutting down
        use tauri::Manager;
        let st = app_for_complete.state::<AppState>();
        st.complete_run(&rid_complete);
    });

    Ok(RunHandle { run_id })
}

#[tauri::command]
pub fn cancel_diagnosis(state: State<'_, AppState>, run_id: String) -> PublicResult<()> {
    state.cancel_run(&run_id)
}

#[tauri::command]
pub async fn check_updates() -> PublicResult<UpdateStatus> {
    Ok(check_updates_now().await)
}

// silence unused import if estimate_attempts unused
#[allow(dead_code)]
fn _est() {
    let _ = estimate_attempts(1, DiagnosisMode::Quick);
}

// Ensure DiagnosisEvent is linked
#[allow(dead_code)]
fn _evt(_: DiagnosisEvent) {}
