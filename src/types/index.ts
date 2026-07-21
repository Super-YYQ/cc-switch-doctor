export type AppType =
  | "claude"
  | "claude-desktop"
  | "codex"
  | "gemini"
  | "grokbuild"
  | "opencode"
  | "openclaw"
  | "hermes"
  | "unknown";

export type DiagnosisMode = "quick" | "smart" | "deep";

export interface ProviderListItem {
  opaqueId: string;
  sourceId: string;
  appType: AppType;
  appLabel: string;
  displayName: string;
  category?: string | null;
  authKind: string;
  providerKind: string;
  safeBaseUrl: string;
  maskedKey: string;
  configuredProtocol?: string | null;
  protocolLabel?: string | null;
  configuredModel?: string | null;
  isCurrent: boolean;
  selectable: boolean;
  skipReason?: string | null;
  needsLocalRouting?: boolean | null;
  websiteUrl?: string | null;
}

export interface SchemaInfoView {
  fingerprintId: string;
  userVersion: number;
  status: string;
  tables: string[];
  providersColumns: string[];
  message: string;
}

export interface DiscoveryInfo {
  found: boolean;
  databasePath?: string | null;
  dataDir?: string | null;
  source?: string | null;
  message: string;
}

export interface ProviderScanView {
  discovery: DiscoveryInfo;
  schema?: SchemaInfoView | null;
  providers: ProviderListItem[];
  canTest: boolean;
  scannedAt: string;
  ccSwitchVersionHint?: string | null;
}

export interface ErrorEvidence {
  source: string;
  code?: string | null;
  message?: string | null;
  matchedKeyword?: string | null;
}

export interface AttemptResult {
  ok: boolean;
  partial: boolean;
  statusCode?: number | null;
  latencyMs: number;
  ttftMs?: number | null;
  protocol: string;
  model: string;
  url: string;
  stream: boolean;
  purpose: string;
  extractedText?: string | null;
  toolCallOk?: boolean | null;
  errorKind?: string | null;
  errorMessage?: string | null;
  responseExcerpt?: string | null;
  classification: string;
  httpSent?: boolean;
  reusedFromCache?: boolean;
  suggestionNote?: string | null;
  tokenLimitField?: "max_completion_tokens" | "max_tokens" | null;
  errorEvidence?: ErrorEvidence[];
}

export interface ProviderDiagnosisSummary {
  opaqueId: string;
  sourceId: string;
  displayName: string;
  appLabel: string;
  status: string;
  currentConfigOk: boolean;
  anySuccess: boolean;
  safeBaseUrl: string;
  configuredProtocol?: string | null;
  configuredModel?: string | null;
  successUrl?: string | null;
  successProtocol?: string | null;
  successModel?: string | null;
  needsLocalRouting?: boolean | null;
  suggestion: string;
  evidence: string[];
  attempts: AttemptResult[];
  confidence: string;
}

export type DiagnosisEvent =
  | {
      type: "run_started";
      runId: string;
      providerCount: number;
      estimatedAttempts: number;
      mode: DiagnosisMode;
    }
  | {
      type: "provider_started";
      runId: string;
      opaqueId: string;
      displayName: string;
      attemptCount: number;
    }
  | {
      type: "attempt_started";
      runId: string;
      opaqueId: string;
      index: number;
      label: string;
      url: string;
      protocol: string;
      model: string;
    }
  | {
      type: "attempt_finished";
      runId: string;
      opaqueId: string;
      index: number;
      result: AttemptResult;
    }
  | {
      type: "provider_finished";
      runId: string;
      opaqueId: string;
      summary: ProviderDiagnosisSummary;
    }
  | { type: "run_cancelled"; runId: string }
  | {
      type: "run_finished";
      runId: string;
      summaries: ProviderDiagnosisSummary[];
    };

export interface UpdateStatus {
  doctorVersion: string;
  doctorLatest?: string | null;
  doctorUpdateAvailable: boolean;
  doctorReleaseUrl?: string | null;
  ccSwitchLatest?: string | null;
  ccSwitchReleaseUrl?: string | null;
  verifiedCcSwitch: string;
  message: string;
  checked: boolean;
  error?: string | null;
}

export interface AppInfo {
  name: string;
  version: string;
  doctorVersion: string;
}
