import { Copy, CheckCircle2 } from "lucide-react";
import type { ErrorEvidence, ProviderDiagnosisSummary } from "@/types";
import { confidenceLabel, hostFromUrl, possibleCauses, statusBadge } from "@/lib/utils";
import { useState } from "react";

interface Props {
  summary: ProviderDiagnosisSummary;
  onCopy: (text: string, label: string) => void;
}

function collectEvidenceLines(s: ProviderDiagnosisSummary): string[] {
  const lines: string[] = [];
  for (const a of s.attempts) {
    if (!a.errorEvidence?.length) continue;
    for (const e of a.errorEvidence) {
      lines.push(formatEvidence(a.statusCode, e));
    }
  }
  return lines.slice(0, 6);
}

function formatEvidence(status: number | null | undefined, e: ErrorEvidence): string {
  const parts: string[] = [];
  if (status != null) parts.push(`HTTP ${status}`);
  if (e.source) parts.push(e.source);
  if (e.code) parts.push(`code=${e.code}`);
  if (e.matchedKeyword) parts.push(`关键词：${e.matchedKeyword}`);
  if (e.message) parts.push(e.message.slice(0, 120));
  return parts.join(" · ");
}

export function ResultCard({ summary: s, onCopy }: Props) {
  const b = statusBadge(s.status);
  const [openAttempts, setOpenAttempts] = useState(false);
  const [openEvidence, setOpenEvidence] = useState(false);
  const [openDebug, setOpenDebug] = useState(false);

  const evidenceTag =
    s.currentConfigOk || s.anySuccess
      ? s.needsLocalRouting
        ? "上游 API 已验证 · 端到端需本地路由（推断）"
        : "上游 API 已验证"
      : "未发现可用组合";

  const conclusion = b.zh;
  const evidenceLines = collectEvidenceLines(s);
  const keyEvidence = s.evidence.slice(0, 2);
  const protocolVariantNote = s.attempts.find(
    (a) => a.classification === "RESPONSE_PROTOCOL_VARIANT_OK" && a.suggestionNote,
  )?.suggestionNote;

  return (
    <article className={`result-card ${b.kind}`}>
      <div className="result-card-head">
        <div style={{ minWidth: 0 }}>
          <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
            <strong className="result-title">
              {s.appLabel} / {s.displayName}
            </strong>
            <span className={`badge ${b.kind}`}>{b.zh}</span>
          </div>
          <div className="mono muted ellipsis result-host" title={s.safeBaseUrl}>
            {s.safeBaseUrl && s.safeBaseUrl !== "—" ? hostFromUrl(s.safeBaseUrl) : "—"}
          </div>
        </div>
        <span className="badge">可信度 {confidenceLabel(s.confidence)}</span>
      </div>

      <p className="result-conclusion">{conclusion}</p>

      <div className="badge info result-tag">
        <CheckCircle2 size={11} /> {evidenceTag}
      </div>

      {protocolVariantNote && <div className="result-variant muted">{protocolVariantNote}</div>}

      {keyEvidence.length > 0 && (
        <ul className="result-key-evidence">
          {keyEvidence.map((e) => (
            <li key={e} className="mono muted">
              {e}
            </li>
          ))}
        </ul>
      )}

      {possibleCauses(s.status) && (
        <div className="result-causes">
          <div className="section-title">可能原因</div>
          <ul>
            {possibleCauses(s.status)!.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </div>
      )}

      <div className="result-suggestion">
        <div className="section-title">建议</div>
        <p>{s.suggestion}</p>
      </div>

      {(s.successProtocol || s.successUrl) && (
        <div className="result-success-combo secondary">
          <div className="section-title">成功组合</div>
          {s.successProtocol && <div>协议：{s.successProtocol}</div>}
          {s.successModel && <div>模型：{s.successModel}</div>}
          {s.successUrl && (
            <div className="mono ellipsis" title={s.successUrl}>
              URL：{s.successUrl}
            </div>
          )}
        </div>
      )}

      {evidenceLines.length > 0 && (
        <details
          className="accordion"
          open={openEvidence}
          onToggle={(e) => setOpenEvidence((e.target as HTMLDetailsElement).open)}
        >
          <summary>判定依据（{evidenceLines.length}）</summary>
          <ul className="result-evidence-list">
            {evidenceLines.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </details>
      )}

      <details
        className="accordion"
        open={openAttempts}
        onToggle={(e) => setOpenAttempts((e.target as HTMLDetailsElement).open)}
      >
        <summary>尝试链（{s.attempts.length}）</summary>
        <ul className="result-evidence-list">
          {s.evidence.map((e) => (
            <li key={e} className="mono muted">
              {e}
            </li>
          ))}
        </ul>
      </details>

      <details
        className="accordion"
        open={openDebug}
        onToggle={(e) => setOpenDebug((e.target as HTMLDetailsElement).open)}
      >
        <summary>调试日志（高级）</summary>
        <pre className="debug-log mono muted">
          {s.attempts
            .map(
              (a, i) =>
                `#${i + 1} ${a.classification} ${a.statusCode ?? "—"} ${a.latencyMs}ms ${a.url}${
                  a.reusedFromCache ? " [复用缓存]" : a.httpSent ? " [真实发送]" : ""
                }${a.errorMessage ? `\n  ${a.errorMessage}` : ""}${
                  a.errorEvidence?.length
                    ? `\n  依据: ${a.errorEvidence
                        .map((ev) => formatEvidence(a.statusCode, ev))
                        .join("; ")}`
                    : ""
                }`,
            )
            .join("\n")}
        </pre>
      </details>

      <div className="result-actions">
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
          <Copy size={12} /> 复制摘要
        </button>
        <button
          className="btn btn-sm"
          type="button"
          onClick={() => onCopy(s.suggestion, "已复制建议")}
        >
          <Copy size={12} /> 复制建议
        </button>
      </div>
    </article>
  );
}
