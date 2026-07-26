import { Activity, Loader2, Square, RotateCcw } from "lucide-react";
import type { DiagnosisMode, RoutingStatusView, VerifyMode } from "@/types";
import { modeDescription, modeTooltip, multiRequestImpactNotice } from "@/lib/utils";

interface Props {
  mode: DiagnosisMode;
  concurrency: number;
  selectedCount: number;
  estimated: number;
  running: boolean;
  completed: number;
  total: number;
  sentRequests: number;
  currentName?: string | null;
  disabledStart: boolean;
  stopping?: boolean;
  verifyMode: VerifyMode;
  routing?: RoutingStatusView | null;
  onMode: (m: DiagnosisMode) => void;
  onConcurrency: (n: number) => void;
  onVerifyMode: (m: VerifyMode) => void;
  onStart: () => void;
  onCancel: () => void;
}

function routingChip(routing?: RoutingStatusView | null): { text: string; kind: string } {
  if (!routing || !routing.configDetected) {
    return { text: "CCS 路由：不可用", kind: "skip" };
  }
  if (routing.serverRunning || routing.healthReachable) {
    return { text: "CCS 路由：运行中", kind: "ok" };
  }
  if (routing.globalEnabled) {
    return { text: "CCS 路由：已配置但未运行", kind: "warn" };
  }
  return { text: "CCS 路由：未开启", kind: "skip" };
}

export function SessionControlBar({
  mode,
  concurrency,
  selectedCount,
  estimated,
  running,
  completed,
  total,
  sentRequests,
  currentName,
  disabledStart,
  stopping,
  verifyMode,
  routing,
  onMode,
  onConcurrency,
  onVerifyMode,
  onStart,
  onCancel,
}: Props) {
  const pct = total > 0 ? Math.round((completed / total) * 100) : 0;
  const finished = !running && completed > 0;
  const rchip = routingChip(routing);
  const claudeApp = routing?.apps?.find((a) => a.appType === "claude");

  return (
    <section className="panel session-bar" style={{ marginTop: 8, padding: "8px 12px" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          flexWrap: "wrap",
          marginBottom: 6,
        }}
      >
        <span className={`badge ${rchip.kind}`}>{rchip.text}</span>
        {claudeApp && (
          <span className={`badge ${claudeApp.enabled ? "ok" : "skip"}`}>
            Claude：{claudeApp.enabled ? "已接管" : "未接管"}
          </span>
        )}
        {claudeApp?.autoFailoverEnabled && <span className="badge warn">自动故障转移：开启</span>}
        {routing?.warning && (
          <span className="badge warn" title={routing.warning}>
            路由提示
          </span>
        )}
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          flexWrap: "wrap",
          justifyContent: "space-between",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
          <div className="segmented" role="radiogroup" aria-label="诊断模式">
            {(
              [
                ["quick", "快速验证"],
                ["smart", "智能诊断"],
                ["deep", "深度兼容"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                className={mode === id ? "active" : ""}
                disabled={running}
                onClick={() => onMode(id)}
                title={modeTooltip(id)}
              >
                {label}
              </button>
            ))}
          </div>

          <div
            className="segmented"
            role="radiogroup"
            aria-label="并发数"
            title={
              mode === "quick"
                ? "低扰动验证固定串行执行，避免同一时间对多个 Provider 或 Host 发起探测。"
                : "同时诊断的 Provider 数量。默认 1 最稳妥；2–3 更快，但更容易触发中转站限流。无论并发多少，同一 Host 每次会话仍最多发送 30 次真实请求。"
            }
          >
            {([1, 2, 3] as const).map((n) => (
              <button
                key={n}
                type="button"
                className={(mode === "quick" ? 1 : concurrency) === n ? "active" : ""}
                disabled={running || (mode === "quick" && n !== 1)}
                onClick={() => onConcurrency(n)}
                aria-label={`并发 ${n}`}
                title={
                  mode === "quick" && n !== 1
                    ? "低扰动验证固定串行执行，避免同一时间对多个 Provider 或 Host 发起探测。"
                    : undefined
                }
              >
                {n}
              </button>
            ))}
          </div>

          <div
            className="segmented"
            role="radiogroup"
            aria-label="验证方式"
            title="自动：App 路由关闭仅直连，开启且可达时直连+路由。仅直连不探测路由。直连+CCS 路由强制双通道。"
          >
            {(
              [
                ["auto", "自动"],
                ["direct_only", "仅直连"],
                ["direct_and_route", "直连+路由"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                className={verifyMode === id ? "active" : ""}
                disabled={running}
                onClick={() => onVerifyMode(id)}
              >
                {label}
              </button>
            ))}
          </div>

          <div className="muted" style={{ fontSize: 12 }}>
            {running ? (
              <>
                {stopping ? "正在停止… · " : ""}
                完成 {completed} / {total} · 请求 {sentRequests} / {estimated}
                {currentName ? ` · 当前：${currentName}` : ""}
              </>
            ) : (
              <>
                已选 <strong style={{ color: "var(--text)" }}>{selectedCount}</strong> · 预计最多{" "}
                <strong style={{ color: "var(--text)" }}>{estimated}</strong> 请求 · 并发{" "}
                {mode === "quick" ? 1 : concurrency}
                {mode === "quick" ? " · 低扰动 / 1 次当前配置请求" : ""}
              </>
            )}
          </div>
        </div>

        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          {running ? (
            <button className="btn btn-danger" type="button" onClick={onCancel} disabled={stopping}>
              <Square size={13} /> {stopping ? "正在停止" : "停止"}
            </button>
          ) : (
            <button
              className="btn btn-primary"
              type="button"
              disabled={disabledStart}
              onClick={onStart}
            >
              {finished ? <RotateCcw size={14} /> : <Activity size={14} />}
              {finished ? "重新诊断" : "开始诊断"}
            </button>
          )}
          {running && (
            <Loader2 size={14} className="muted" style={{ animation: "spin 1s linear infinite" }} />
          )}
        </div>
      </div>

      <div
        className="muted ellipsis"
        style={{ fontSize: 11.5, marginTop: 6 }}
        title={modeDescription(mode)}
      >
        {modeDescription(mode)}
      </div>

      {mode === "quick" && verifyMode === "direct_and_route" && (
        <div className="muted" style={{ fontSize: 11.5, marginTop: 4 }}>
          快速验证不会执行路由链业务请求；请切换智能诊断。
        </div>
      )}

      {!running && selectedCount > 0 && multiRequestImpactNotice(mode) && (
        <div
          className="muted"
          style={{ fontSize: 11.5, marginTop: 4 }}
          data-testid="multi-request-notice"
        >
          {multiRequestImpactNotice(mode)}
        </div>
      )}

      {running && (
        <div style={{ marginTop: 6 }}>
          <div className="progress">
            <span style={{ width: `${pct}%` }} />
          </div>
        </div>
      )}
    </section>
  );
}
