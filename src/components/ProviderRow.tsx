import { Lock } from "lucide-react";
import type { ProviderListItem } from "@/types";
import { hostFromUrl, statusBadge } from "@/lib/utils";

interface Props {
  provider: ProviderListItem;
  checked: boolean;
  active: boolean;
  running: boolean;
  resultStatus?: string;
  disabled: boolean;
  onToggle: () => void;
  onActivate: () => void;
}

export function ProviderRow({
  provider: p,
  checked,
  active,
  running,
  resultStatus,
  disabled,
  onToggle,
  onActivate,
}: Props) {
  const host = p.safeBaseUrl && p.safeBaseUrl !== "—" ? hostFromUrl(p.safeBaseUrl) : "—";
  const status = resultStatus
    ? statusBadge(resultStatus)
    : p.selectable
      ? { label: "可诊断", kind: "ok" as const, zh: "可诊断" }
      : { label: "已跳过", kind: "skip" as const, zh: "官方登录，已跳过" };

  return (
    <article
      className={[
        "provider-card",
        checked ? "selected" : "",
        active ? "active" : "",
        !p.selectable ? "disabled" : "",
        running ? "running" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      onClick={onActivate}
    >
      <div style={{ display: "flex", gap: 10, alignItems: "flex-start" }}>
        <input
          type="checkbox"
          checked={checked}
          disabled={!p.selectable || disabled}
          onClick={(e) => e.stopPropagation()}
          onChange={onToggle}
          aria-label={`选择 ${p.displayName}`}
          style={{ marginTop: 3 }}
        />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 8,
              alignItems: "center",
            }}
          >
            <div style={{ display: "flex", gap: 8, alignItems: "center", minWidth: 0 }}>
              <strong className="ellipsis" style={{ fontSize: 14 }}>
                {p.displayName}
              </strong>
              {p.isCurrent && <span className="badge primary">当前</span>}
            </div>
            <span className={`badge ${status.kind}`} title={p.skipReason ?? status.zh}>
              {!p.selectable && <Lock size={11} />}
              {status.kind === "ok" && !resultStatus ? "可诊断" : status.zh}
            </span>
          </div>
          <div className="muted mono ellipsis" style={{ marginTop: 4 }}>
            {p.appLabel}
            {p.maskedKey ? ` · ${p.maskedKey}` : ""}
          </div>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1.2fr 1fr",
              gap: 8,
              marginTop: 8,
            }}
          >
            <div className="mono muted ellipsis" title={p.safeBaseUrl}>
              {host}
            </div>
            <div className="ellipsis secondary" title={p.configuredModel ?? ""}>
              {p.configuredModel || "—"}
            </div>
          </div>
          <div
            style={{
              marginTop: 6,
              display: "flex",
              gap: 8,
              alignItems: "center",
              flexWrap: "wrap",
            }}
          >
            <span className="badge">{p.protocolLabel || "未知协议"}</span>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={(e) => {
                e.stopPropagation();
                onActivate();
              }}
              aria-label={`查看详情 ${p.displayName}`}
            >
              查看详情
            </button>
          </div>
          {p.skipReason && (
            <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>
              {p.skipReason}
            </div>
          )}
        </div>
      </div>
    </article>
  );
}
