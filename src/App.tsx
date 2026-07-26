import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  cancelDiagnosis,
  checkUpdates,
  estimateDiagnosis,
  getAppInfo,
  isTauri,
  onDiagnosisEvent,
  refreshProviders,
  scanProviders,
  selectDatabase,
  startDiagnosis,
} from "@/lib/api";
import { defaultSelectedIds, estimateClientSide, filterProviders } from "@/lib/utils";
import type {
  AppInfo,
  DiagnosisEvent,
  DiagnosisMode,
  ProviderDiagnosisSummary,
  ProviderListItem,
  ProviderScanView,
  UpdateStatus,
  VerifyMode,
} from "@/types";
import { AppHeader } from "@/components/AppHeader";
import { SessionControlBar } from "@/components/SessionControlBar";
import { ProviderWorkspace } from "@/components/ProviderWorkspace";
import { DiagnosisWorkspace } from "@/components/DiagnosisWorkspace";
import { SafetyDrawer } from "@/components/SafetyDrawer";

const DEMO_SCAN: ProviderScanView = {
  discovery: {
    found: true,
    message: "开发预览：synthetic fixture（未连接 Tauri）",
    databasePath: "fixture://synthetic",
    source: "demo",
  },
  schema: {
    fingerprintId: "demo-v16",
    userVersion: 16,
    status: "verified",
    tables: ["providers", "provider_endpoints", "settings"],
    providersColumns: ["id", "app_type", "name", "settings_config", "meta", "is_current"],
    message: "Schema 与 CC Switch v3.18.0（user_version=16）已验证指纹匹配。",
    versionVerification: "verified",
    capabilities: {
      providerScan: {
        state: "supported",
        reason: "providers 核心与推荐字段完整。",
        missingTables: [],
        missingColumns: [],
        unverifiedColumns: [],
      },
      endpointScan: {
        state: "supported",
        reason: "provider_endpoints 结构完整。",
        missingTables: [],
        missingColumns: [],
        unverifiedColumns: [],
      },
      directDiagnosis: {
        state: "supported",
        reason: "可执行上游直连诊断。",
        missingTables: [],
        missingColumns: [],
        unverifiedColumns: [],
      },
      routingDiscovery: {
        state: "supported",
        reason: "proxy_config 结构完整，可读取路由状态。",
        missingTables: [],
        missingColumns: [],
        unverifiedColumns: [],
      },
      routingDiagnosis: {
        state: "supported",
        reason: "路由结构可读取。",
        missingTables: [],
        missingColumns: [],
        unverifiedColumns: [],
      },
    },
  },
  providers: [
    {
      opaqueId: "demo-1",
      sourceId: "glm-claude-1",
      appType: "claude",
      appLabel: "Claude Code",
      displayName: "GLM Relay",
      safeBaseUrl: "https://api.example-relay.test/v1",
      maskedKey: "sk-tes…aude",
      configuredProtocol: "anthropic_messages",
      protocolLabel: "Anthropic Messages",
      configuredModel: "glm-4.5",
      isCurrent: true,
      selectable: true,
      authKind: "anthropic_key",
      providerKind: "third_party_api",
    },
    {
      opaqueId: "demo-3",
      sourceId: "minimax-codex-1",
      appType: "codex",
      appLabel: "Codex",
      displayName: "MiniMax Codex",
      safeBaseUrl: "https://api.minimax-relay.test/v1",
      maskedKey: "sk-tes…odex",
      configuredProtocol: "openai_chat",
      protocolLabel: "OpenAI Chat Completions",
      configuredModel: "MiniMax-M2.5",
      isCurrent: true,
      selectable: true,
      needsLocalRouting: true,
      authKind: "bearer_token",
      providerKind: "third_party_api",
    },
    {
      opaqueId: "demo-2",
      sourceId: "codex-official-oauth",
      appType: "codex",
      appLabel: "Codex",
      displayName: "Codex Official OAuth",
      safeBaseUrl: "—",
      maskedKey: "",
      isCurrent: false,
      selectable: false,
      skipReason: "安全跳过：Codex OAuth / 官方登录（不读取登录缓存，不提供绕过）",
      authKind: "codex_oauth",
      providerKind: "managed_account",
    },
  ],
  canTest: true,
  scannedAt: new Date().toISOString(),
  ccSwitchVersionHint: "3.18.0",
};

export default function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [scan, setScan] = useState<ProviderScanView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [appFilter, setAppFilter] = useState("claude");
  const [query, setQuery] = useState("");
  const [onlySelected, setOnlySelected] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [activeId, setActiveId] = useState<string | null>(null);
  const [mode, setMode] = useState<DiagnosisMode>("quick");
  const [concurrency, setConcurrency] = useState(1);
  const [verifyMode, setVerifyMode] = useState<VerifyMode>("auto");
  const [stopping, setStopping] = useState(false);
  const activeRunIdRef = useRef<string | null>(null);
  const [running, setRunning] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [liveLog, setLiveLog] = useState<string[]>([]);
  const [summaries, setSummaries] = useState<ProviderDiagnosisSummary[]>([]);
  const [updates, setUpdates] = useState<UpdateStatus | null>(null);
  const [safetyOpen, setSafetyOpen] = useState(false);
  const [hideSafetySession, setHideSafetySession] = useState(false);
  const [estimated, setEstimated] = useState(0);
  const [sentRequests, setSentRequests] = useState(0);
  const [runningIds, setRunningIds] = useState<Set<string>>(new Set());
  const [currentName, setCurrentName] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [completedCount, setCompletedCount] = useState(0);

  /** Replace scan data and wipe all session/result state bound to the previous DB view. */
  const applyFreshScan = useCallback((view: ProviderScanView) => {
    setScan(view);
    setSelected(new Set());
    setAppFilter("claude");
    setActiveId(null);
    setSummaries([]);
    setLiveLog([]);
    setRunningIds(new Set());
    setCompletedCount(0);
    setSentRequests(0);
    setCurrentName(null);
    setRunId(null);
    activeRunIdRef.current = null;
    setRunning(false);
    setStopping(false);
    setError(null);
  }, []);

  const load = useCallback(async () => {
    setError(null);
    try {
      if (!isTauri()) {
        applyFreshScan(DEMO_SCAN);
        setAppInfo({ name: "CC Switch Doctor", version: "0.1.6", doctorVersion: "0.1.6" });
        if (!hideSafetySession) setSafetyOpen(true);
        return;
      }
      const [info, view] = await Promise.all([getAppInfo(), scanProviders()]);
      setAppInfo(info);
      applyFreshScan(view);
      if (!hideSafetySession) setSafetyOpen(true);
    } catch (e) {
      setError(String(e));
    }
  }, [hideSafetySession, applyFreshScan]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!isTauri()) return;
    let un: (() => void) | undefined;
    void onDiagnosisEvent((ev) => handleEvent(ev)).then((fn) => {
      un = fn;
    });
    return () => un?.();
  }, []);

  const providers = useMemo(() => scan?.providers ?? [], [scan?.providers]);
  const filtered = useMemo(
    () =>
      filterProviders(providers, {
        app: appFilter,
        query,
        onlySelected,
        selected,
      }),
    [providers, appFilter, query, onlySelected, selected],
  );

  useEffect(() => {
    const count = [...selected].filter((id) =>
      providers.find((p) => p.opaqueId === id && p.selectable),
    ).length;
    if (!isTauri() || count === 0) {
      setEstimated(estimateClientSide(count, mode));
      return;
    }
    void estimateDiagnosis({ opaqueIds: [...selected], mode, concurrency })
      .then((r) => setEstimated(r.estimatedAttempts))
      .catch(() => setEstimated(estimateClientSide(count, mode)));
  }, [selected, mode, concurrency, providers]);

  function handleEvent(ev: DiagnosisEvent) {
    if ("runId" in ev && activeRunIdRef.current && ev.runId !== activeRunIdRef.current) return;
    if (ev.type === "run_started") {
      setSentRequests(0);
      setCompletedCount(0);
      setLiveLog((l) => [
        `开始：${ev.providerCount} 个配置，预估 ${ev.estimatedAttempts} 次请求（${ev.mode}）`,
        ...l,
      ]);
    } else if (ev.type === "provider_started") {
      setRunningIds((s) => new Set(s).add(ev.opaqueId));
      setCurrentName(ev.displayName);
    } else if (ev.type === "attempt_started") {
      setLiveLog((l) => [`→ ${ev.label} | ${ev.protocol} | ${ev.model} | ${ev.url}`, ...l]);
    } else if (ev.type === "attempt_finished") {
      if (ev.result.httpSent !== false && !ev.result.reusedFromCache) {
        setSentRequests((n) => n + 1);
      }
      const note = ev.result.reusedFromCache
        ? " [复用]"
        : ev.result.suggestionNote
          ? ` — ${ev.result.suggestionNote}`
          : "";
      setLiveLog((l) => [
        `← ${ev.result.classification} ${ev.result.statusCode ?? ""} ${ev.result.latencyMs}ms${note}`,
        ...l,
      ]);
    } else if (ev.type === "provider_finished") {
      setRunningIds((s) => {
        const n = new Set(s);
        n.delete(ev.opaqueId);
        return n;
      });
      setCompletedCount((c) => c + 1);
      setSummaries((s) => {
        const rest = s.filter((x) => x.opaqueId !== ev.summary.opaqueId);
        return [ev.summary, ...rest];
      });
    } else if (ev.type === "run_finished") {
      setSummaries(ev.summaries);
      setRunning(false);
      setStopping(false);
      setRunId(null);
      activeRunIdRef.current = null;
      setCurrentName(null);
      setRunningIds(new Set());
      setLiveLog((l) => [`完成：${ev.summaries.length} 个结果`, ...l]);
    } else if (ev.type === "run_cancelled") {
      // Keep running=true until matching run_finished finishes cleanup (P1-4).
      setStopping(true);
      setCurrentName(null);
      setRunningIds(new Set());
      setLiveLog((l) => ["正在收尾…", ...l]);
    }
  }

  function toggle(p: ProviderListItem) {
    if (!p.selectable || running) return;
    setSelected((prev) => {
      const n = new Set(prev);
      if (n.has(p.opaqueId)) n.delete(p.opaqueId);
      else n.add(p.opaqueId);
      return n;
    });
  }

  function selectFiltered() {
    setSelected((prev) => {
      const n = new Set(prev);
      for (const p of filtered) if (p.selectable) n.add(p.opaqueId);
      return n;
    });
  }

  async function onStart() {
    setError(null);
    setSummaries([]);
    setLiveLog([]);
    setSentRequests(0);
    setCompletedCount(0);
    try {
      if (!isTauri()) {
        setRunning(true);
        await new Promise((r) => setTimeout(r, 400));
        const demo: ProviderDiagnosisSummary[] = [
          {
            opaqueId: "demo-1",
            sourceId: "glm-claude-1",
            displayName: "GLM Relay",
            appLabel: "Claude Code",
            status: "CURRENT_CONFIG_OK",
            currentConfigOk: true,
            anySuccess: true,
            safeBaseUrl: "https://api.example-relay.test/v1",
            configuredProtocol: "Anthropic Messages",
            configuredModel: "glm-4.5",
            successUrl: "https://api.example-relay.test/v1/messages",
            successProtocol: "Anthropic Messages",
            successModel: "glm-4.5",
            suggestion: "当前配置可用。Base URL 与协议正确。本工具未修改任何配置。",
            evidence: ["尝试 1：POST /v1/messages -> 200（GENERATE_OK）"],
            attempts: [],
            confidence: "high",
          },
          {
            opaqueId: "demo-3",
            sourceId: "minimax-codex-1",
            displayName: "MiniMax Codex",
            appLabel: "Codex",
            status: "LOCAL_ROUTING_REQUIRED",
            currentConfigOk: false,
            anySuccess: true,
            safeBaseUrl: "https://api.minimax-relay.test/v1",
            configuredProtocol: "OpenAI Responses",
            configuredModel: "MiniMax-M2.5",
            successUrl: "https://api.minimax-relay.test/v1/chat/completions",
            successProtocol: "OpenAI Chat Completions",
            successModel: "MiniMax-M2.5",
            needsLocalRouting: true,
            suggestion:
              "上游 Chat Completions 可用，但 Responses 不可用。Codex 场景建议在 CC Switch 启用本地路由/协议转换。本工具未修改配置。",
            evidence: [
              "尝试 1：POST /v1/responses -> 404",
              "尝试 2：POST /v1/chat/completions -> 200",
            ],
            attempts: [],
            confidence: "medium",
          },
        ];
        setSummaries(demo);
        setCompletedCount(2);
        setSentRequests(3);
        setRunning(false);
        showToast("预览模式：已生成 synthetic 结果");
        return;
      }
      if (!scan?.canTest) {
        setError(
          scan?.schema?.capabilities?.directDiagnosis?.reason ||
            "当前数据库结构无法安全执行上游直连诊断。",
        );
        return;
      }
      const ids = [...selected];
      if (!ids.length) {
        setError("请先勾选要测试的第三方配置。");
        return;
      }
      setRunning(true);
      const effectiveConcurrency = mode === "quick" ? 1 : concurrency;
      const { runId: id } = await startDiagnosis({
        opaqueIds: ids,
        mode,
        concurrency: effectiveConcurrency,
        verifyMode,
      });
      activeRunIdRef.current = id;
      setRunId(id);
      setStopping(false);
    } catch (e) {
      setRunning(false);
      setError(String(e));
    }
  }

  async function onCancel() {
    if (runId) {
      setStopping(true);
      try {
        await cancelDiagnosis(runId);
      } catch {
        setStopping(false);
      }
    }
  }

  async function onPickDb() {
    if (running) return;
    try {
      if (!isTauri()) return;
      const { open } = await import("@tauri-apps/plugin-dialog");
      const file = await open({
        multiple: false,
        filters: [{ name: "CC Switch DB", extensions: ["db"] }],
      });
      if (typeof file === "string") {
        const view = await selectDatabase(file);
        applyFreshScan(view);
      }
    } catch (e) {
      setError(String(e));
    }
  }

  async function onRefresh() {
    if (running) return;
    try {
      if (!isTauri()) {
        applyFreshScan(DEMO_SCAN);
        return;
      }
      const view = await refreshProviders();
      applyFreshScan(view);
    } catch (e) {
      setError(String(e));
    }
  }

  async function onCheckUpdates() {
    try {
      if (!isTauri()) {
        setUpdates({
          doctorVersion: "0.1.6",
          doctorUpdateAvailable: false,
          verifiedCcSwitch: "3.17.0",
          message: "开发预览：更新检查需在应用内执行",
          checked: false,
        });
        return;
      }
      setUpdates(await checkUpdates());
    } catch (e) {
      setError(String(e));
    }
  }

  function showToast(msg: string) {
    setToast(msg);
    window.setTimeout(() => setToast(null), 2200);
  }

  function onCopy(text: string, label: string) {
    void navigator.clipboard.writeText(text).then(() => showToast(label));
  }

  const statusById = useMemo(() => {
    const m = new Map<string, string>();
    for (const s of summaries) m.set(s.opaqueId, s.primaryOutcome || s.status);
    return m;
  }, [summaries]);

  const selectedCount = [...selected].filter((id) =>
    providers.find((p) => p.opaqueId === id && p.selectable),
  ).length;

  return (
    <div className="app-shell">
      <AppHeader
        appInfo={appInfo}
        scan={scan}
        updates={updates}
        running={running}
        onRefresh={() => void onRefresh()}
        onPickDb={() => void onPickDb()}
        onCheckUpdates={() => void onCheckUpdates()}
        onOpenSafety={() => setSafetyOpen(true)}
      />

      <SessionControlBar
        mode={mode}
        concurrency={concurrency}
        selectedCount={selectedCount}
        estimated={estimated}
        running={running}
        completed={completedCount}
        total={selectedCount || summaries.length}
        sentRequests={sentRequests}
        currentName={currentName}
        disabledStart={running || selectedCount === 0 || !scan?.canTest}
        stopping={stopping}
        verifyMode={verifyMode}
        routing={scan?.routing}
        onMode={(m) => {
          setMode(m);
          if (m === "quick") setConcurrency(1);
        }}
        onConcurrency={(n) => {
          if (mode === "quick") {
            setConcurrency(1);
            return;
          }
          setConcurrency(n);
        }}
        onVerifyMode={setVerifyMode}
        onStart={() => void onStart()}
        onCancel={() => void onCancel()}
      />

      {error && (
        <div
          style={{
            marginTop: 10,
            color: "var(--danger)",
            background: "var(--danger-soft)",
            border: "1px solid var(--danger-border)",
            borderRadius: 10,
            padding: "8px 12px",
            fontSize: 13,
          }}
        >
          {error}
        </div>
      )}

      <div className="workspace">
        <ProviderWorkspace
          providers={providers}
          filtered={filtered}
          appFilter={appFilter}
          query={query}
          onlySelected={onlySelected}
          selected={selected}
          activeId={activeId}
          runningIds={runningIds}
          statusById={statusById}
          running={running}
          schemaStatus={scan?.schema?.status}
          canTest={scan?.canTest}
          onAppFilter={setAppFilter}
          onQuery={setQuery}
          onOnlySelected={setOnlySelected}
          onToggle={toggle}
          onActivate={(id) => {
            setActiveId(id);
            // Scroll result into view without changing checkbox selection
            window.requestAnimationFrame(() => {
              document
                .getElementById(`result-${id}`)
                ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
            });
          }}
          onSelectFiltered={selectFiltered}
          onClearSelection={() => setSelected(new Set())}
          onSelectCurrent={() => setSelected(defaultSelectedIds(providers))}
        />
        <DiagnosisWorkspace
          summaries={summaries}
          activeId={activeId}
          providers={providers}
          running={running}
          liveLog={liveLog}
          onCopy={onCopy}
          onActivateProvider={(id) => {
            setActiveId(id);
            window.requestAnimationFrame(() => {
              document
                .getElementById(`provider-${id}`)
                ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
            });
          }}
        />
      </div>

      <SafetyDrawer
        open={safetyOpen}
        onClose={() => setSafetyOpen(false)}
        hideThisSession={hideSafetySession}
        onHideThisSession={setHideSafetySession}
      />

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

// Exported for unit tests
export { DEMO_SCAN };
