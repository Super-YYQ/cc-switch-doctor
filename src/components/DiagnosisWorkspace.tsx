import { Stethoscope } from "lucide-react";
import type { ProviderDiagnosisSummary } from "@/types";
import { ResultCard } from "./ResultCard";
import { useMemo, useState } from "react";
import { statusBadge } from "@/lib/utils";

interface Props {
  summaries: ProviderDiagnosisSummary[];
  running: boolean;
  liveLog: string[];
  onCopy: (text: string, label: string) => void;
}

type Filter = "all" | "needs" | "fail" | "ok" | "skip";

function priority(s: ProviderDiagnosisSummary): number {
  const b = statusBadge(s.status).kind;
  if (b === "info" || s.needsLocalRouting) return 0;
  if (b === "danger" || b === "warn") return 1;
  if (b === "ok") return 2;
  return 3;
}

export function DiagnosisWorkspace({ summaries, running, liveLog, onCopy }: Props) {
  const [filter, setFilter] = useState<Filter>("all");
  const [showLog, setShowLog] = useState(false);

  const sorted = useMemo(
    () => [...summaries].sort((a, b) => priority(a) - priority(b)),
    [summaries],
  );

  const filtered = sorted.filter((s) => {
    const k = statusBadge(s.status).kind;
    if (filter === "all") return true;
    if (filter === "ok") return k === "ok";
    if (filter === "skip") return k === "skip";
    if (filter === "needs") return k === "info" || !!s.needsLocalRouting;
    if (filter === "fail") return k === "danger" || (k === "warn" && !s.anySuccess);
    return true;
  });

  const stats = useMemo(() => {
    let ok = 0;
    let needs = 0;
    let fail = 0;
    for (const s of summaries) {
      const k = statusBadge(s.status).kind;
      if (k === "ok" && s.currentConfigOk) ok++;
      else if (k === "info" || s.anySuccess) needs++;
      else if (k !== "skip") fail++;
    }
    return { ok, needs, fail };
  }, [summaries]);

  return (
    <section className="panel workspace-pane" style={{ padding: 12 }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 8,
          alignItems: "center",
          marginBottom: 10,
        }}
      >
        <div className="section-title">诊断结果</div>
        {!!summaries.length && (
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
            {(
              [
                ["all", "全部"],
                ["needs", "需调整"],
                ["fail", "失败"],
                ["ok", "正常"],
                ["skip", "跳过"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                className={`chip ${filter === id ? "active" : ""}`}
                style={{ height: 28, fontSize: 12, padding: "0 10px" }}
                onClick={() => setFilter(id)}
              >
                {label}
              </button>
            ))}
          </div>
        )}
      </div>

      {!!summaries.length && !running && (
        <div
          className="muted"
          style={{
            marginBottom: 10,
            fontSize: 13,
            padding: "8px 10px",
            background: "var(--bg-soft)",
            borderRadius: 10,
            border: "1px solid var(--border)",
          }}
        >
          本次诊断完成 · {stats.ok} 当前配置可用 · {stats.needs} 可通过变体使用 · {stats.fail}{" "}
          未发现可用组合
        </div>
      )}

      <div className="workspace-scroll">
        {!summaries.length && !running && (
          <div className="empty-state">
            <div className="icon">
              <Stethoscope size={22} />
            </div>
            <div style={{ fontWeight: 600, color: "var(--text-secondary)" }}>
              选择左侧配置并开始诊断
            </div>
            <div style={{ marginTop: 6, fontSize: 13 }}>
              这里将展示结论、建议和尝试链
              <br />
              原始调试日志默认折叠
            </div>
          </div>
        )}

        {filtered.map((s) => (
          <ResultCard key={s.opaqueId} summary={s} onCopy={onCopy} />
        ))}
      </div>

      <div style={{ borderTop: "1px solid var(--border)", marginTop: 8, paddingTop: 8 }}>
        <button
          type="button"
          className="btn btn-sm btn-ghost"
          onClick={() => setShowLog((v) => !v)}
        >
          {showLog ? "收起" : "展开"}调试日志（高级）
        </button>
        {showLog && (
          <pre
            className="mono muted"
            style={{
              marginTop: 8,
              maxHeight: 120,
              overflow: "auto",
              background: "var(--bg-soft)",
              borderRadius: 8,
              padding: 10,
              fontSize: 11,
              whiteSpace: "pre-wrap",
            }}
          >
            {liveLog.length ? liveLog.join("\n") : "暂无日志"}
          </pre>
        )}
      </div>
    </section>
  );
}
