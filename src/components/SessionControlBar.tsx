import { Activity, Loader2, Square, RotateCcw } from "lucide-react";
import type { DiagnosisMode } from "@/types";

interface Props {
  mode: DiagnosisMode;
  selectedCount: number;
  estimated: number;
  running: boolean;
  completed: number;
  total: number;
  sentRequests: number;
  currentName?: string | null;
  disabledStart: boolean;
  onMode: (m: DiagnosisMode) => void;
  onStart: () => void;
  onCancel: () => void;
}

export function SessionControlBar({
  mode,
  selectedCount,
  estimated,
  running,
  completed,
  total,
  sentRequests,
  currentName,
  disabledStart,
  onMode,
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
                title={
                  id === "quick"
                    ? "只测当前配置"
                    : id === "smart"
                      ? "失败时尝试受控变体"
                      : "额外测试流式与 Tool Calling"
                }
              >
                {label}
              </button>
            ))}
          </div>

          <div className="muted" style={{ fontSize: 13 }}>
            {running ? (
              <>
                完成 {completed} / {total} · 请求 {sentRequests} / {estimated}
                {currentName ? ` · 当前：${currentName}` : ""}
              </>
            ) : (
              <>
                已选 <strong style={{ color: "var(--text)" }}>{selectedCount}</strong> · 预计最多{" "}
                <strong style={{ color: "var(--text)" }}>{estimated}</strong> 请求 · 并发 1
              </>
            )}
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          {running ? (
            <button className="btn btn-danger" type="button" onClick={onCancel}>
              <Square size={14} /> 停止
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
