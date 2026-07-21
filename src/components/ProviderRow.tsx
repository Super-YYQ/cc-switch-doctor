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
      id={`provider-${p.opaqueId}`}
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
      <div className="provider-card-inner">
        <input
          type="checkbox"
          checked={checked}
          disabled={!p.selectable || disabled}
          onClick={(e) => e.stopPropagation()}
          onChange={onToggle}
          aria-label={`选择 ${p.displayName}`}
        />
        <div className="provider-card-body">
          <div className="provider-card-title-row">
            <div className="provider-card-name">
              <strong className="ellipsis">{p.displayName}</strong>
              {p.isCurrent && <span className="badge primary">当前</span>}
            </div>
            <span className={`badge ${status.kind}`} title={p.skipReason ?? status.zh}>
              {!p.selectable && <Lock size={10} />}
              {status.kind === "ok" && !resultStatus ? "可诊断" : status.zh}
            </span>
          </div>
          <div className="muted mono ellipsis provider-meta">
            {p.appLabel}
            {p.maskedKey ? ` · ${p.maskedKey}` : ""}
          </div>
          <div className="provider-host-model">
            <div className="mono muted ellipsis" title={p.safeBaseUrl}>
              {host}
            </div>
            <div className="ellipsis secondary" title={p.configuredModel ?? ""}>
              {p.configuredModel || "—"}
            </div>
          </div>
          <div className="provider-footer">
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
          {p.skipReason && <div className="muted provider-skip">{p.skipReason}</div>}
        </div>
      </div>
    </article>
  );
}
