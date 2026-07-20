import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  DiagnosisEvent,
  DiagnosisMode,
  ProviderScanView,
  UpdateStatus,
} from "@/types";

export async function getAppInfo(): Promise<AppInfo> {
  return invoke("get_app_info");
}

export async function discoverCcSwitch() {
  return invoke("discover_cc_switch");
}

export async function scanProviders(): Promise<ProviderScanView> {
  return invoke("scan_providers");
}

export async function refreshProviders(): Promise<ProviderScanView> {
  return invoke("refresh_providers");
}

export async function selectDatabase(path: string): Promise<ProviderScanView> {
  return invoke("select_database", { path });
}

export async function estimateDiagnosis(input: {
  opaqueIds: string[];
  mode: DiagnosisMode;
  concurrency: number;
}): Promise<{ estimatedAttempts: number; providerCount: number; mode: DiagnosisMode }> {
  return invoke("estimate_diagnosis", {
    request: {
      opaqueIds: input.opaqueIds,
      mode: input.mode,
      concurrency: input.concurrency,
    },
  });
}

export async function startDiagnosis(input: {
  opaqueIds: string[];
  mode: DiagnosisMode;
  concurrency: number;
}): Promise<{ runId: string }> {
  return invoke("start_diagnosis", {
    request: {
      opaqueIds: input.opaqueIds,
      mode: input.mode,
      concurrency: input.concurrency,
    },
  });
}

export async function cancelDiagnosis(runId: string): Promise<void> {
  return invoke("cancel_diagnosis", { runId });
}

export async function checkUpdates(): Promise<UpdateStatus> {
  return invoke("check_updates");
}

export async function onDiagnosisEvent(
  handler: (event: DiagnosisEvent) => void,
): Promise<UnlistenFn> {
  return listen<DiagnosisEvent>("diagnosis_event", (e) => handler(e.payload));
}

/** Browser/dev mock when not running inside Tauri */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
