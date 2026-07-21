import { Activity, Loader2, Square, RotateCcw } from "lucide-react";
import type { DiagnosisMode } from "@/types";
import { modeDescription, modeTooltip } from "@/lib/utils";

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
  onMode: (m: DiagnosisMode) => void;
  onConcurrency: (n: number) => void;
  onStart: () => void;
  onCancel: () => void;
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
  onMode,
  onConcurrency,
  onStart,
  onCancel,
}: Props) {
  const pct = total > 0 ? Math.round((completed / total) * 100) : 0;
  const finished = !running && completed > 0;

  return (
    <section className="panel" style={{ marginTop: 12, padding: "10px 14px" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 14,
          flexWrap: "wrap",
          justifyContent: "space-between",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 14, flexWrap: "wrap" }}>
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
            title="同时诊断的 Provider 数量。默认 1 最稳妥；2–3 更快，但更容易触发中转站限流。无论并发多少，同一 Host 每次会话仍最多发送 30 次真实请求。"
          >
            {([1, 2, 3] as const).map((n) => (
              <button
                key={n}
                type="button"
                className={concurrency === n ? "active" : ""}
                disabled={running}
                onClick={() => onConcurrency(n)}
                aria-label={`并发 ${n}`}
              >
                {n}
              </button>
            ))}
          </div>

          <div className="muted" style={{ fontSize: 13 }}>
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
                {concurrency}
              </>
            )}
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          {running ? (
            <button className="btn btn-danger" type="button" onClick={onCancel} disabled={stopping}>
              <Square size={14} /> {stopping ? "正在停止" : "停止"}
            </button>
          ) : (
            <button
              className="btn btn-primary"
              type="button"
              disabled={disabledStart}
              onClick={onStart}
            >
              {finished ? <RotateCcw size={15} /> : <Activity size={15} />}
              {finished ? "重新诊断" : "开始诊断"}
            </button>
          )}
          {running && (
            <Loader2 size={16} className="muted" style={{ animation: "spin 1s linear infinite" }} />
          )}
        </div>
      </div>

      <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>
        {modeDescription(mode)}
      </div>

      {running && (
        <div style={{ marginTop: 10 }}>
          <div className="progress">
            <span style={{ width: `${pct}%` }} />
          </div>
        </div>
      )}
    </section>
  );
}
