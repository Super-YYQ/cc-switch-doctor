import { Stethoscope } from "lucide-react";
import type { ProviderDiagnosisSummary, ProviderListItem } from "@/types";
import { ResultCard } from "./ResultCard";
import { useEffect, useMemo, useRef, useState } from "react";
import { isInteractiveTarget, statusBadge, primaryStatusCode } from "@/lib/utils";

interface Props {
  summaries: ProviderDiagnosisSummary[];
  activeId: string | null;
  providers: ProviderListItem[];
  running: boolean;
  liveLog: string[];
  onCopy: (text: string, label: string) => void;
  onActivateProvider: (id: string) => void;
}

type Filter = "all" | "needs" | "fail" | "ok" | "skip";

function priority(s: ProviderDiagnosisSummary): number {
  const b = statusBadge(primaryStatusCode(s)).kind;
  if (b === "info" || s.needsLocalRouting) return 0;
  if (b === "danger" || b === "warn") return 1;
  if (b === "ok") return 2;
  return 3;
}

function matchesFilter(s: ProviderDiagnosisSummary, filter: Filter): boolean {
  const k = statusBadge(primaryStatusCode(s)).kind;
  if (filter === "all") return true;
  if (filter === "ok") return k === "ok";
  if (filter === "skip") return k === "skip";
  if (filter === "needs") return k === "info" || !!s.needsLocalRouting;
  if (filter === "fail") return k === "danger" || (k === "warn" && !s.anySuccess);
  return true;
}

export function DiagnosisWorkspace({
  summaries,
  activeId,
  providers,
  running,
  liveLog,
  onCopy,
  onActivateProvider,
}: Props) {
  const [filter, setFilter] = useState<Filter>("all");
  const [showLog, setShowLog] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const userNavRef = useRef(false);
  const prevActiveIdRef = useRef<string | null>(null);

  // Keep left-provider order when possible
  const ordered = useMemo(() => {
    const byId = new Map(summaries.map((s) => [s.opaqueId, s]));
    const leftOrder = providers.map((p) => p.opaqueId);
    const out: ProviderDiagnosisSummary[] = [];
    for (const id of leftOrder) {
      const s = byId.get(id);
      if (s) out.push(s);
    }
    for (const s of summaries) {
      if (!out.some((x) => x.opaqueId === s.opaqueId)) out.push(s);
    }
    return out;
  }, [summaries, providers]);

  const sorted = useMemo(() => {
    // Default: provider list order; only re-prioritize when filter is not all
    if (filter === "all") return ordered;
    return [...ordered].sort((a, b) => priority(a) - priority(b));
  }, [ordered, filter]);

  // When left/right navigation activates a result hidden by the current filter, reveal it.
  // Only react to activeId transitions so manual filter chips still work.
  useEffect(() => {
    if (activeId === prevActiveIdRef.current) return;
    prevActiveIdRef.current = activeId;
    if (!activeId) return;
    const target = summaries.find((s) => s.opaqueId === activeId);
    if (!target) return;
    if (!matchesFilter(target, filter)) {
      setFilter("all");
    }
  }, [activeId, summaries, filter]);

  const filtered = sorted.filter((s) => matchesFilter(s, filter));

  const stats = useMemo(() => {
    let ok = 0;
    let needs = 0;
    let fail = 0;
    for (const s of summaries) {
      const k = statusBadge(primaryStatusCode(s)).kind;
      if (k === "ok" && s.currentConfigOk) ok++;
      else if (k === "info" || s.anySuccess) needs++;
      else if (k !== "skip") fail++;
    }
    return { ok, needs, fail };
  }, [summaries]);

  useEffect(() => {
    if (!activeId || !userNavRef.current) return;
    const el = document.getElementById(`result-${activeId}`);
    el?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    userNavRef.current = false;
  }, [activeId, filtered]);

  function jumpTo(id: string) {
    userNavRef.current = true;
    onActivateProvider(id);
  }

  function jumpRelative(delta: number) {
    if (!filtered.length) return;
    const idx = Math.max(
      0,
      filtered.findIndex((s) => s.opaqueId === activeId),
    );
    const next = filtered[(idx + delta + filtered.length) % filtered.length];
    jumpTo(next.opaqueId);
  }

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

      {!!summaries.length && (
        <div
          style={{
            display: "flex",
            gap: 6,
            alignItems: "center",
            marginBottom: 8,
            flexWrap: "wrap",
          }}
        >
          <label className="muted" style={{ fontSize: 12 }}>
            当前结果：
            <select
              value={activeId ?? ""}
              onChange={(e) => {
                if (e.target.value) jumpTo(e.target.value);
              }}
              style={{
                marginLeft: 4,
                height: 28,
                borderRadius: 8,
                border: "1px solid var(--border)",
                background: "var(--bg-elevated)",
                color: "var(--text)",
                fontSize: 12,
                maxWidth: 220,
              }}
            >
              <option value="">选择 Provider</option>
              {filtered.map((s) => (
                <option key={s.opaqueId} value={s.opaqueId}>
                  {statusBadge(primaryStatusCode(s)).zh} · {s.appLabel}/{s.displayName}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className="btn btn-sm" onClick={() => jumpRelative(-1)}>
            上一条
          </button>
          <button type="button" className="btn btn-sm" onClick={() => jumpRelative(1)}>
            下一条
          </button>
        </div>
      )}

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

      <div className="workspace-scroll" ref={scrollRef}>
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
          <div
            key={s.opaqueId}
            id={`result-${s.opaqueId}`}
            onClick={(event) => {
              if (isInteractiveTarget(event.target)) return;
              jumpTo(s.opaqueId);
            }}
            style={{
              outline: activeId === s.opaqueId ? "2px solid var(--primary)" : undefined,
              borderRadius: 12,
              marginBottom: 2,
            }}
          >
            <ResultCard summary={s} onCopy={onCopy} />
          </div>
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
