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

export type CapabilityState = "supported" | "degraded" | "disabled";

export interface CapabilityStatus {
  state: CapabilityState;
  reason: string;
  missingTables: string[];
  missingColumns: string[];
  unverifiedColumns: string[];
}

export interface SchemaCapabilities {
  providerScan: CapabilityStatus;
  endpointScan: CapabilityStatus;
  directDiagnosis: CapabilityStatus;
  routingDiscovery: CapabilityStatus;
  routingDiagnosis: CapabilityStatus;
}

export interface SchemaInfoView {
  fingerprintId: string;
  userVersion: number;
  status: string;
  tables: string[];
  providersColumns: string[];
  message: string;
  /** Independent version verification (not a runtime gate). */
  versionVerification?: string | null;
  capabilities?: SchemaCapabilities | null;
  warnings?: string[] | null;
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
  routing?: RoutingStatusView | null;
}

export interface AppRoutingStatusView {
  appType: string;
  appLabel: string;
  enabled: boolean;
  autoFailoverEnabled: boolean;
  maxRetries?: number | null;
  streamingFirstByteTimeout?: number | null;
  streamingIdleTimeout?: number | null;
  nonStreamingTimeout?: number | null;
  activeProviderId?: string | null;
  activeProviderName?: string | null;
}

export interface RoutingStatusView {
  configDetected: boolean;
  globalEnabled: boolean;
  listenAddress?: string | null;
  listenPort?: number | null;
  healthReachable: boolean;
  serverRunning: boolean;
  failoverCount?: number | null;
  apps: AppRoutingStatusView[];
  warning?: string | null;
  connectHost?: string | null;
}

export type VerifyMode = "auto" | "direct_only" | "direct_and_route";

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
  channel?: "direct_upstream" | "ccs_local_route" | string;
  responseCompatibility?: "native" | "cross_protocol" | "loose_field" | string | null;
  requestedProtocol?: string | null;
  matchedProtocol?: string | null;
}

export type RouteDisposition =
  | "not_requested"
  | "not_configured"
  | "not_running"
  | "not_current_target"
  | "unsupported_app"
  | "blocked_non_loopback"
  | "attempted";

export interface CapabilityOutcome {
  attempted: boolean;
  success: boolean;
  status: string;
}

export interface DirectChannelSummary {
  attempted: boolean;
  status: string;
  success: boolean;
  nativeSuccess: boolean;
  bestAttemptIndex?: number | null;
}

export interface RouteChannelSummary {
  disposition: RouteDisposition;
  attempted: boolean;
  generate?: CapabilityOutcome | null;
  streaming?: CapabilityOutcome | null;
  overallStatus?: string | null;
  actualProviderId?: string | null;
  actualProviderName?: string | null;
  failoverCountBefore?: number | null;
  failoverCountAfter?: number | null;
  notice?: string | null;
}

export interface ProviderDiagnosisSummary {
  opaqueId: string;
  sourceId: string;
  displayName: string;
  appLabel: string;
  /** Primary outcome (compat alias of primaryOutcome). */
  status: string;
  /** Explicit primary outcome; equals status when present. */
  primaryOutcome?: string;
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
  /** Layered direct-channel summary. */
  direct?: DirectChannelSummary | null;
  /** Layered route-channel summary. Disposition is never primary. */
  route?: RouteChannelSummary | null;
  routeStatus?: string | null;
  directStatus?: string | null;
  routeSideEffectNotice?: string | null;
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
