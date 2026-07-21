import { Copy, CheckCircle2 } from "lucide-react";
import type { ErrorEvidence, ProviderDiagnosisSummary } from "@/types";
import {
  confidenceLabel,
  groupAttemptsByCanonical,
  hostFromUrl,
  possibleCauses,
  primaryStatusCode,
  routeDispositionLabel,
  statusBadge,
} from "@/lib/utils";
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
  // Primary badge = primaryOutcome only (never route disposition alone).
  const primaryCode = primaryStatusCode(s);
  const b = statusBadge(primaryCode);
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

  const routeAttempts = s.attempts.filter((a) => a.channel === "ccs_local_route");
  const directAttempts = s.attempts.filter((a) => !a.channel || a.channel === "direct_upstream");
  const hasRouteMeta = !!s.route || !!s.routeStatus || routeAttempts.length > 0;
  const hasDirectMeta = !!s.direct || !!s.directStatus || directAttempts.length > 0;
  const showChannels = hasRouteMeta || hasDirectMeta;

  const directStatusCode = s.direct?.status || s.directStatus || null;
  const routeDisp = routeDispositionLabel(s.route?.disposition, s.routeStatus);
  const routeAttempted = s.route?.attempted === true || routeAttempts.some((a) => a.httpSent);

  const realSendCount = s.attempts.filter((a) => a.httpSent).length;
  const grouped = groupAttemptsByCanonical(s.attempts);

  return (
    <article className={`result-card ${b.kind}`}>
      <div className="result-card-head">
        <div style={{ minWidth: 0 }}>
          <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
            <strong className="result-title">
              {s.appLabel} / {s.displayName}
            </strong>
            <span className={`badge ${b.kind}`} title={primaryCode}>
              {b.zh}
            </span>
            {hasRouteMeta && !routeAttempted && (
              <span className="badge skip" title={routeDisp.detail}>
                路由未验证
              </span>
            )}
          </div>
          <div className="mono muted ellipsis result-host" title={s.safeBaseUrl}>
            {s.safeBaseUrl && s.safeBaseUrl !== "—" ? hostFromUrl(s.safeBaseUrl) : "—"}
          </div>
        </div>
        <span className="badge">可信度 {confidenceLabel(s.confidence)}</span>
      </div>

      <div className="section-title" style={{ marginTop: 4 }}>
        诊断结论
      </div>
      <p className="result-conclusion">{conclusion}</p>

      {showChannels && (
        <div style={{ display: "grid", gap: 6, marginBottom: 8 }}>
          <div
            style={{
              border: "1px solid var(--border)",
              borderRadius: 8,
              padding: "6px 8px",
              fontSize: 12,
            }}
          >
            <div className="section-title">上游直连</div>
            <div className="secondary">
              {directStatusCode
                ? statusBadge(directStatusCode).zh
                : directAttempts.some((a) => a.ok)
                  ? "直连成功"
                  : directAttempts.length
                    ? "直连未成功"
                    : "未执行直连"}
            </div>
          </div>
          {(hasRouteMeta || s.routeSideEffectNotice) && (
            <div
              style={{
                border: "1px solid var(--border)",
                borderRadius: 8,
                padding: "6px 8px",
                background: "var(--bg-soft)",
                fontSize: 12,
              }}
            >
              <div className="section-title">CCS 路由</div>
              <div className="secondary">
                {routeAttempted ? (
                  <>
                    {s.route?.generate && (
                      <div>
                        基础推理：{s.route.generate.success ? "成功" : "失败"}
                        {s.route.generate.status ? `（${s.route.generate.status}）` : ""}
                      </div>
                    )}
                    {s.route?.streaming && (
                      <div>
                        流式输出：{s.route.streaming.success ? "成功" : "不支持或失败"}
                        {s.route.streaming.status ? `（${s.route.streaming.status}）` : ""}
                      </div>
                    )}
                    {!s.route?.generate && (
                      <div>
                        {s.routeStatus
                          ? statusBadge(s.routeStatus).zh
                          : routeAttempts.some((a) => a.ok)
                            ? "路由请求成功"
                            : "路由未成功"}
                      </div>
                    )}
                  </>
                ) : (
                  <>
                    <div>
                      <span className={`badge ${routeDisp.kind}`} style={{ marginRight: 6 }}>
                        {routeDisp.title}
                      </span>
                      {routeDisp.detail}
                    </div>
                  </>
                )}
              </div>
              {s.route?.actualProviderName && (
                <div className="muted" style={{ marginTop: 4, fontSize: 11 }}>
                  实际处理 Provider：{s.route.actualProviderName}
                  {s.route.actualProviderId ? `（${s.route.actualProviderId}）` : ""}
                </div>
              )}
              {(s.route?.failoverCountBefore != null || s.route?.failoverCountAfter != null) && (
                <div className="muted" style={{ marginTop: 2, fontSize: 11 }}>
                  故障转移次数：{s.route.failoverCountBefore ?? "—"} →{" "}
                  {s.route.failoverCountAfter ?? "—"}
                </div>
              )}
              {(s.route?.notice || s.routeSideEffectNotice) && (
                <div className="muted" style={{ marginTop: 4, fontSize: 11 }}>
                  {s.route?.notice || s.routeSideEffectNotice}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      <div className="badge info result-tag">
        <CheckCircle2 size={11} /> {evidenceTag}
        {realSendCount > 0 ? ` · 真实请求 ${realSendCount}` : ""}
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

      {possibleCauses(primaryCode) && (
        <div className="result-causes">
          <div className="section-title">可能原因</div>
          <ul>
            {possibleCauses(primaryCode)!.map((c) => (
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
        <summary>
          尝试链（{s.attempts.length} · 真实发送 {realSendCount}）
        </summary>
        <ul className="result-evidence-list">
          {grouped.map((g) => (
            <li key={g.key} className="mono muted">
              {g.label}
              <br />
              真实发送 {g.realSends} 次{g.cacheHits > 0 ? ` · 缓存复用 ${g.cacheHits} 次` : ""}
              <br />
              最终状态：{g.finalStatus}
              {statusBadge(g.finalStatus).zh ? `（${statusBadge(g.finalStatus).zh}）` : ""}
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
          {`primary=${primaryCode}\ndirect=${directStatusCode ?? "—"}\nroute.disposition=${s.route?.disposition ?? "—"}\nrouteStatus=${s.routeStatus ?? "—"}\n`}
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
                `主状态: ${primaryCode}（${b.zh}）`,
                directStatusCode
                  ? `上游直连: ${directStatusCode}（${statusBadge(directStatusCode).zh}）`
                  : null,
                hasRouteMeta ? `CCS 路由: ${routeDisp.title} — ${routeDisp.detail}` : null,
                s.suggestion,
                evidenceTag,
                ...s.evidence,
                "（完整 Key 从未包含在此摘要中）",
              ]
                .filter(Boolean)
                .join("\n"),
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
