import { Copy, CheckCircle2 } from "lucide-react";
import type { ProviderDiagnosisSummary } from "@/types";
import { confidenceLabel, hostFromUrl, possibleCauses, statusBadge } from "@/lib/utils";
import { useState } from "react";

interface Props {
  summary: ProviderDiagnosisSummary;
  onCopy: (text: string, label: string) => void;
}

export function ResultCard({ summary: s, onCopy }: Props) {
  const b = statusBadge(s.status);
  const [openAttempts, setOpenAttempts] = useState(false);
  const [openDebug, setOpenDebug] = useState(false);

  const evidenceTag =
    s.currentConfigOk || s.anySuccess
      ? s.needsLocalRouting
        ? "上游 API 已验证 · 端到端需本地路由（推断）"
        : "上游 API 已验证"
      : "未发现可用组合";

  const conclusion = b.zh;

  return (
    <article className={`result-card ${b.kind}`}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 10,
          alignItems: "flex-start",
          flexWrap: "wrap",
        }}
      >
        <div style={{ minWidth: 0 }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <strong style={{ fontSize: 15 }}>
              {s.appLabel} / {s.displayName}
            </strong>
            <span className={`badge ${b.kind}`}>{b.zh}</span>
          </div>
          <div className="mono muted ellipsis" style={{ marginTop: 4 }} title={s.safeBaseUrl}>
            {s.safeBaseUrl && s.safeBaseUrl !== "—" ? hostFromUrl(s.safeBaseUrl) : "—"}
          </div>
        </div>
        <span className="badge">可信度 {confidenceLabel(s.confidence)}</span>
      </div>

      <p style={{ margin: "12px 0 8px", lineHeight: 1.55, color: "var(--text-secondary)" }}>
        {conclusion}
      </p>

      <div className="badge info" style={{ marginBottom: 10 }}>
        <CheckCircle2 size={12} /> {evidenceTag}
      </div>

      {possibleCauses(s.status) && (
        <div style={{ marginBottom: 10, fontSize: 13 }}>
          <div className="section-title" style={{ marginBottom: 4 }}>
            可能原因
          </div>
          <ul className="secondary" style={{ margin: 0, paddingLeft: 18, lineHeight: 1.5 }}>
            {possibleCauses(s.status)!.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </div>
      )}

      <div className="muted mono" style={{ fontSize: 11, marginBottom: 8 }}>
        技术状态：{s.status}
      </div>

      <div
        style={{
          background: "var(--bg-soft)",
          borderRadius: 10,
          padding: "10px 12px",
          border: "1px solid var(--border)",
          marginBottom: 10,
        }}
      >
        <div className="section-title" style={{ marginBottom: 6 }}>
          建议
        </div>
        <p style={{ margin: 0, lineHeight: 1.55, fontSize: 13 }}>{s.suggestion}</p>
      </div>

      {(s.successProtocol || s.successUrl) && (
        <div style={{ marginBottom: 10, fontSize: 13 }}>
          <div className="section-title" style={{ marginBottom: 4 }}>
            成功组合
          </div>
          <div className="secondary">
            {s.successProtocol && <div>协议：{s.successProtocol}</div>}
            {s.successModel && <div>模型：{s.successModel}</div>}
            {s.successUrl && (
              <div className="mono ellipsis" title={s.successUrl}>
                URL：{s.successUrl}
              </div>
            )}
          </div>
        </div>
      )}

      <details
        className="accordion"
        open={openAttempts}
        onToggle={(e) => setOpenAttempts((e.target as HTMLDetailsElement).open)}
      >
        <summary>尝试链（{s.attempts.length}）</summary>
        <ul style={{ paddingLeft: 18, margin: "8px 0" }}>
          {s.evidence.map((e) => (
            <li key={e} className="mono muted" style={{ fontSize: 12, marginBottom: 4 }}>
              {e}
            </li>
          ))}
        </ul>
      </details>

      <details
        className="accordion"
        style={{ marginTop: 8 }}
        open={openDebug}
        onToggle={(e) => setOpenDebug((e.target as HTMLDetailsElement).open)}
      >
        <summary>调试日志（高级）</summary>
        <pre
          className="mono muted"
          style={{
            margin: "8px 0 0",
            maxHeight: 160,
            overflow: "auto",
            background: "var(--bg-soft)",
            borderRadius: 8,
            padding: 10,
            fontSize: 11,
            whiteSpace: "pre-wrap",
          }}
        >
          {s.attempts
            .map(
              (a, i) =>
                `#${i + 1} ${a.classification} ${a.statusCode ?? "—"} ${a.latencyMs}ms ${a.url}${
                  a.errorMessage ? `\n  ${a.errorMessage}` : ""
                }`,
            )
            .join("\n")}
        </pre>
      </details>

      <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
        <button
          className="btn btn-sm"
          type="button"
          onClick={() =>
            onCopy(
              [
                `CC Switch Doctor 诊断摘要`,
                `${s.appLabel} / ${s.displayName}`,
                `状态: ${s.status}（${b.zh}）`,
                s.suggestion,
                evidenceTag,
                ...s.evidence,
                "（完整 Key 从未包含在此摘要中）",
              ].join("\n"),
              "已复制诊断摘要",
            )
          }
        >
          <Copy size={13} /> 复制摘要
        </button>
        <button
          className="btn btn-sm"
          type="button"
          onClick={() => onCopy(s.suggestion, "已复制建议")}
        >
          <Copy size={13} /> 复制建议
        </button>
      </div>
    </article>
  );
}
