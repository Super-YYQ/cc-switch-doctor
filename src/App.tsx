import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Loader2,
  RefreshCw,
  ShieldCheck,
  Square,
  Stethoscope,
} from "lucide-react";
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
import { estimateClientSide, filterProviders, statusBadge } from "@/lib/utils";
import type {
  AppInfo,
  DiagnosisEvent,
  DiagnosisMode,
  ProviderDiagnosisSummary,
  ProviderListItem,
  ProviderScanView,
  UpdateStatus,
} from "@/types";

const APP_FILTERS: { id: string; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "claude", label: "Claude" },
  { id: "claude-desktop", label: "Claude Desktop" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "opencode", label: "OpenCode" },
  { id: "openclaw", label: "OpenClaw" },
  { id: "hermes", label: "Hermes" },
  { id: "grokbuild", label: "Grok" },
];

const DEMO_SCAN: ProviderScanView = {
  discovery: {
    found: false,
    message: "开发预览模式：未连接 Tauri 后端。打包后将实时读取 CC Switch 数据库。",
  },
  schema: null,
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
  canTest: false,
  scannedAt: new Date().toISOString(),
};

export default function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [scan, setScan] = useState<ProviderScanView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [appFilter, setAppFilter] = useState("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [mode, setMode] = useState<DiagnosisMode>("smart");
  const [concurrency, setConcurrency] = useState(1);
  const [running, setRunning] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [liveLog, setLiveLog] = useState<string[]>([]);
  const [summaries, setSummaries] = useState<ProviderDiagnosisSummary[]>([]);
  const [updates, setUpdates] = useState<UpdateStatus | null>(null);
  const [showSafety, setShowSafety] = useState(true);
  const [estimated, setEstimated] = useState(0);
  const [busyMsg, setBusyMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    setBusyMsg("正在扫描 CC Switch…");
    setError(null);
    try {
      if (!isTauri()) {
        setScan(DEMO_SCAN);
        setAppInfo({ name: "CC Switch Doctor", version: "0.1.0", doctorVersion: "0.1.0" });
        return;
      }
      const [info, view] = await Promise.all([getAppInfo(), scanProviders()]);
      setAppInfo(info);
      setScan(view);
      setSelected(new Set());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyMsg(null);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!isTauri()) return;
    let un: (() => void) | undefined;
    void onDiagnosisEvent((ev) => {
      handleEvent(ev);
    }).then((fn) => {
      un = fn;
    });
    return () => {
      un?.();
    };
  }, []);

  const providers = useMemo(() => scan?.providers ?? [], [scan?.providers]);
  const filtered = useMemo(
    () => filterProviders(providers, { app: appFilter, query }),
    [providers, appFilter, query],
  );

  useEffect(() => {
    const count = [...selected].filter((id) =>
      providers.find((p) => p.opaqueId === id && p.selectable),
    ).length;
    if (!isTauri() || count === 0) {
      setEstimated(estimateClientSide(count, mode));
      return;
    }
    void estimateDiagnosis({
      opaqueIds: [...selected],
      mode,
      concurrency,
    })
      .then((r) => setEstimated(r.estimatedAttempts))
      .catch(() => setEstimated(estimateClientSide(count, mode)));
  }, [selected, mode, concurrency, providers]);

  function handleEvent(ev: DiagnosisEvent) {
    if (ev.type === "run_started") {
      setLiveLog((l) => [
        `开始：${ev.providerCount} 个配置，预估 ${ev.estimatedAttempts} 次请求（${ev.mode}）`,
        ...l,
      ]);
    } else if (ev.type === "attempt_started") {
      setLiveLog((l) => [`→ ${ev.label} | ${ev.protocol} | ${ev.model} | ${ev.url}`, ...l]);
    } else if (ev.type === "attempt_finished") {
      setLiveLog((l) => [
        `← ${ev.result.classification} ${ev.result.statusCode ?? ""} ${ev.result.latencyMs}ms ${ev.result.url}`,
        ...l,
      ]);
    } else if (ev.type === "provider_finished") {
      setSummaries((s) => {
        const rest = s.filter((x) => x.opaqueId !== ev.summary.opaqueId);
        return [ev.summary, ...rest];
      });
    } else if (ev.type === "run_finished") {
      setSummaries(ev.summaries);
      setRunning(false);
      setRunId(null);
      setLiveLog((l) => [`完成：${ev.summaries.length} 个结果`, ...l]);
    } else if (ev.type === "run_cancelled") {
      setRunning(false);
      setLiveLog((l) => ["已取消", ...l]);
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

  function clearSelection() {
    setSelected(new Set());
  }

  async function onStart() {
    if (showSafety) {
      // require dismiss safety first click path still allows explicit continue
    }
    setError(null);
    setSummaries([]);
    setLiveLog([]);
    try {
      if (!isTauri()) {
        setError("请在 Tauri 应用中运行真实诊断。");
        return;
      }
      if (!scan?.canTest) {
        setError("当前 schema 未通过兼容检查，已安全停止测试。");
        return;
      }
      const ids = [...selected];
      if (!ids.length) {
        setError("请先勾选要测试的第三方配置。");
        return;
      }
      setRunning(true);
      const { runId: id } = await startDiagnosis({
        opaqueIds: ids,
        mode,
        concurrency,
      });
      setRunId(id);
    } catch (e) {
      setRunning(false);
      setError(String(e));
    }
  }

  async function onCancel() {
    if (runId) {
      try {
        await cancelDiagnosis(runId);
      } catch {
        /* ignore */
      }
    }
    setRunning(false);
  }

  async function onPickDb() {
    try {
      if (!isTauri()) return;
      const { open } = await import("@tauri-apps/plugin-dialog");
      const file = await open({
        multiple: false,
        filters: [{ name: "CC Switch DB", extensions: ["db"] }],
      });
      if (typeof file === "string") {
        setBusyMsg("加载所选数据库…");
        const view = await selectDatabase(file);
        setScan(view);
        setSelected(new Set());
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyMsg(null);
    }
  }

  async function onCheckUpdates() {
    try {
      if (!isTauri()) {
        setUpdates({
          doctorVersion: "0.1.0",
          doctorUpdateAvailable: false,
          verifiedCcSwitch: "3.17.0",
          message: "开发预览：更新检查需在应用内执行",
          checked: false,
        });
        return;
      }
      const u = await checkUpdates();
      setUpdates(u);
    } catch (e) {
      setError(String(e));
    }
  }

  const schemaStatus = scan?.schema?.status ?? "—";
  const schemaKind =
    schemaStatus === "verified"
      ? "ok"
      : schemaStatus === "compatible"
        ? "warn"
        : schemaStatus === "unknown" || schemaStatus === "unsupported"
          ? "danger"
          : "skip";

  return (
    <div className="app-shell">
      <header className="panel" style={{ margin: "12px 12px 0", padding: "14px 16px" }}>
        <div
          style={{ display: "flex", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}
        >
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <Stethoscope size={20} />
              <strong style={{ fontSize: "1.15rem" }}>CC Switch Doctor</strong>
              <span className="badge">v{appInfo?.doctorVersion ?? "0.1.0"}</span>
            </div>
            <div className="muted" style={{ marginTop: 6, fontSize: "0.9rem" }}>
              只读 · 无状态 · 纯 HTTP 诊断 · 不启动任何 AI CLI
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
            <span className={`badge ${scan?.discovery.found ? "ok" : "warn"}`}>
              DB: {scan?.discovery.found ? "已连接" : "未找到"}
            </span>
            <span className={`badge ${schemaKind}`}>兼容: {schemaStatus}</span>
            <span className="badge">
              CC Switch: {scan?.ccSwitchVersionHint ?? updates?.verifiedCcSwitch ?? "—"}
            </span>
            <button className="btn" onClick={() => void onCheckUpdates()} type="button">
              检查更新
            </button>
            <button
              className="btn"
              onClick={() => void (isTauri() ? refreshProviders().then(setScan) : load())}
              type="button"
              disabled={running}
            >
              <RefreshCw size={14} style={{ display: "inline", marginRight: 4 }} />
              刷新配置
            </button>
            <button
              className="btn"
              onClick={() => void onPickDb()}
              type="button"
              disabled={running}
            >
              选择 DB
            </button>
          </div>
        </div>
        {updates && (
          <div className="muted" style={{ marginTop: 10, fontSize: "0.88rem" }}>
            {updates.message}
            {updates.doctorReleaseUrl && updates.doctorUpdateAvailable && (
              <>
                {" "}
                <a href={updates.doctorReleaseUrl} target="_blank" rel="noreferrer">
                  打开 Doctor Release
                </a>
              </>
            )}
          </div>
        )}
        {scan?.schema && (
          <div className="muted" style={{ marginTop: 8, fontSize: "0.85rem" }}>
            {scan.schema.message}
          </div>
        )}
        {scan && !scan.discovery.found && (
          <div style={{ marginTop: 8 }} className="muted">
            {scan.discovery.message}
          </div>
        )}
      </header>

      {showSafety && (
        <div
          className="panel"
          style={{
            margin: "12px 12px 0",
            padding: 14,
            borderColor: "color-mix(in srgb, var(--accent) 40%, var(--border))",
          }}
        >
          <div style={{ display: "flex", gap: 10, alignItems: "flex-start" }}>
            <ShieldCheck />
            <div style={{ flex: 1 }}>
              <strong>安全说明（每次启动显示，不保存“已读”）</strong>
              <ul
                className="muted"
                style={{ margin: "8px 0 0", paddingLeft: 18, lineHeight: 1.55 }}
              >
                <li>
                  仅通过 Rust HTTP 客户端测试 API，绝不启动 Codex / Claude / OpenCode / Gemini CLI /
                  CC Switch。
                </li>
                <li>不读取 `.codex` / `.claude` / OpenCode / Gemini 登录目录。</li>
                <li>CC Switch 数据库只读；完整 Key 只在内存，不会进入界面、日志或磁盘。</li>
                <li>测试会消耗极少量上游 token；自动变体仅限同一 Host，跨 Host 重定向会阻断。</li>
              </ul>
            </div>
            <button className="btn btn-primary" type="button" onClick={() => setShowSafety(false)}>
              知道了
            </button>
          </div>
        </div>
      )}

      <main
        style={{
          display: "grid",
          gridTemplateColumns: "1.4fr 1fr",
          gap: 12,
          padding: 12,
          flex: 1,
          minHeight: 0,
        }}
      >
        <section
          className="panel"
          style={{ padding: 12, display: "flex", flexDirection: "column", minHeight: 0 }}
        >
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 10 }}>
            {APP_FILTERS.map((f) => (
              <button
                key={f.id}
                type="button"
                className={`chip ${appFilter === f.id ? "active" : ""}`}
                onClick={() => setAppFilter(f.id)}
              >
                {f.label}
              </button>
            ))}
          </div>
          <div style={{ display: "flex", gap: 8, marginBottom: 10, flexWrap: "wrap" }}>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="搜索供应商 / 模型 / 主机…"
              style={{
                flex: 1,
                minWidth: 180,
                borderRadius: 10,
                border: "1px solid var(--border)",
                background: "var(--bg-soft)",
                color: "var(--text)",
                padding: "0.45rem 0.7rem",
              }}
            />
            <button className="btn" type="button" onClick={selectFiltered} disabled={running}>
              全选当前筛选
            </button>
            <button className="btn" type="button" onClick={clearSelection} disabled={running}>
              取消全选
            </button>
          </div>

          <div className="scroll-y" style={{ flex: 1, minHeight: 240 }}>
            <table className="table">
              <thead>
                <tr>
                  <th style={{ width: 36 }}></th>
                  <th>应用</th>
                  <th>供应商</th>
                  <th>地址</th>
                  <th>模型</th>
                  <th>状态</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((p) => {
                  const checked = selected.has(p.opaqueId);
                  return (
                    <tr key={p.opaqueId} style={{ opacity: p.selectable ? 1 : 0.72 }}>
                      <td>
                        <input
                          type="checkbox"
                          checked={checked}
                          disabled={!p.selectable || running}
                          onChange={() => toggle(p)}
                          aria-label={`选择 ${p.displayName}`}
                        />
                      </td>
                      <td>
                        {p.appLabel}
                        {p.isCurrent && <div className="badge ok">当前</div>}
                      </td>
                      <td>
                        <div>{p.displayName}</div>
                        <div className="muted mono">{p.maskedKey || "—"}</div>
                      </td>
                      <td className="mono muted" style={{ maxWidth: 220, wordBreak: "break-all" }}>
                        {p.safeBaseUrl || "—"}
                      </td>
                      <td>
                        <div>{p.configuredModel || "—"}</div>
                        <div className="muted" style={{ fontSize: "0.8rem" }}>
                          {p.protocolLabel || "—"}
                        </div>
                      </td>
                      <td>
                        {p.selectable ? (
                          <span className="badge ok">可测试</span>
                        ) : (
                          <span className="badge skip" title={p.skipReason ?? ""}>
                            安全跳过
                          </span>
                        )}
                        {p.skipReason && (
                          <div
                            className="muted"
                            style={{ fontSize: "0.75rem", marginTop: 4, maxWidth: 160 }}
                          >
                            {p.skipReason}
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })}
                {!filtered.length && (
                  <tr>
                    <td colSpan={6} className="muted" style={{ padding: 24, textAlign: "center" }}>
                      没有匹配的配置
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          <div
            style={{
              borderTop: "1px solid var(--border)",
              marginTop: 10,
              paddingTop: 12,
              display: "flex",
              flexWrap: "wrap",
              gap: 12,
              alignItems: "center",
            }}
          >
            <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
              <span className="muted">模式：</span>
              {(
                [
                  ["quick", "快速验证"],
                  ["smart", "智能诊断"],
                  ["deep", "深度兼容性"],
                ] as const
              ).map(([id, label]) => (
                <label
                  key={id}
                  htmlFor={`mode-${id}`}
                  style={{ display: "inline-flex", gap: 4, alignItems: "center" }}
                >
                  <input
                    id={`mode-${id}`}
                    type="radio"
                    name="mode"
                    checked={mode === id}
                    disabled={running}
                    onChange={() => setMode(id)}
                  />
                  {label}
                </label>
              ))}
            </div>
            <label
              className="muted"
              style={{ display: "inline-flex", gap: 6, alignItems: "center" }}
            >
              并发
              <select
                value={concurrency}
                disabled={running}
                onChange={(e) => setConcurrency(Number(e.target.value))}
                style={{
                  borderRadius: 8,
                  border: "1px solid var(--border)",
                  background: "var(--bg-soft)",
                  color: "var(--text)",
                  padding: "0.25rem 0.4rem",
                }}
              >
                <option value={1}>1</option>
                <option value={2}>2</option>
                <option value={3}>3</option>
              </select>
            </label>
            <span className="badge">
              已选 {selected.size} · 预估请求 {estimated}
            </span>
            <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
              <button
                className="btn btn-primary"
                type="button"
                disabled={running || !selected.size || showSafety}
                onClick={() => void onStart()}
                title={showSafety ? "请先确认安全说明" : undefined}
              >
                {running ? <Loader2 className="spin" size={14} /> : <Activity size={14} />} 开始测试
              </button>
              <button
                className="btn btn-danger"
                type="button"
                disabled={!running}
                onClick={() => void onCancel()}
              >
                <Square size={14} /> 取消
              </button>
            </div>
          </div>
          {error && (
            <div style={{ marginTop: 10, color: "var(--danger)", display: "flex", gap: 6 }}>
              <AlertTriangle size={16} /> {error}
            </div>
          )}
          {busyMsg && (
            <div className="muted" style={{ marginTop: 8 }}>
              {busyMsg}
            </div>
          )}
        </section>

        <section
          className="panel"
          style={{ padding: 12, display: "flex", flexDirection: "column", minHeight: 0 }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
            <CheckCircle2 size={16} />
            <strong>实时结果 / 诊断建议</strong>
          </div>
          <div
            className="scroll-y mono muted"
            style={{
              maxHeight: 160,
              background: "var(--bg-soft)",
              borderRadius: 10,
              padding: 10,
              border: "1px solid var(--border)",
              marginBottom: 10,
              whiteSpace: "pre-wrap",
            }}
          >
            {liveLog.length ? liveLog.join("\n") : "等待测试…"}
          </div>
          <div className="scroll-y" style={{ flex: 1 }}>
            {summaries.map((s) => {
              const b = statusBadge(s.status);
              return (
                <article
                  key={s.opaqueId}
                  style={{
                    border: "1px solid var(--border)",
                    borderRadius: 12,
                    padding: 12,
                    marginBottom: 10,
                    background: "var(--bg-soft)",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      gap: 8,
                      flexWrap: "wrap",
                    }}
                  >
                    <div>
                      <strong>
                        {s.appLabel} / {s.displayName}
                      </strong>
                      <div className="muted mono" style={{ fontSize: "0.85rem" }}>
                        {s.safeBaseUrl}
                      </div>
                    </div>
                    <span className={`badge ${b.kind}`}>{b.label}</span>
                  </div>
                  <p style={{ margin: "10px 0 6px", lineHeight: 1.5 }}>{s.suggestion}</p>
                  <div className="muted" style={{ fontSize: "0.85rem" }}>
                    可信度：{s.confidence}
                    {s.successProtocol && (
                      <>
                        {" "}
                        · 成功：{s.successProtocol} / {s.successModel} / {s.successUrl}
                      </>
                    )}
                  </div>
                  <details style={{ marginTop: 8 }}>
                    <summary className="muted">尝试链（{s.attempts.length}）</summary>
                    <ul style={{ paddingLeft: 18, margin: "8px 0" }}>
                      {s.evidence.map((e) => (
                        <li
                          key={e}
                          className="mono"
                          style={{ fontSize: "0.8rem", marginBottom: 4 }}
                        >
                          {e}
                        </li>
                      ))}
                    </ul>
                  </details>
                  <button
                    className="btn"
                    type="button"
                    style={{ marginTop: 8 }}
                    onClick={() => {
                      const text = [
                        `CC Switch Doctor 诊断摘要`,
                        `${s.appLabel} / ${s.displayName}`,
                        `状态: ${s.status}`,
                        s.suggestion,
                        ...s.evidence,
                        "（完整 Key 从未包含在此摘要中）",
                      ].join("\n");
                      void navigator.clipboard.writeText(text);
                    }}
                  >
                    复制诊断摘要（写入系统剪贴板）
                  </button>
                </article>
              );
            })}
            {!summaries.length && (
              <div className="muted" style={{ textAlign: "center", padding: 28 }}>
                选择配置并开始测试后，这里会显示尝试链与建议。
              </div>
            )}
          </div>
        </section>
      </main>
    </div>
  );
}
