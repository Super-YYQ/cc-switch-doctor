import { Lock } from "lucide-react";
import type { KeyboardEvent } from "react";
import type { ProviderListItem } from "@/types";
import { hostFromUrl, statusBadge } from "@/lib/utils";

interface Props {
  provider: ProviderListItem;
  checked: boolean;
  active: boolean;
  running: boolean;
  resultStatus?: string;
  /** True only when a diagnosis summary exists for this provider (not selection state). */
  hasResult: boolean;
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
  hasResult,
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

  function activateIfPossible() {
    if (!hasResult) return;
    onActivate();
  }

  function onBodyKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (!hasResult) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onActivate();
    }
  }

  return (
    <article
      id={`provider-${p.opaqueId}`}
      className={[
        "provider-card",
        checked ? "selected" : "",
        active ? "active" : "",
        !p.selectable ? "disabled" : "",
        running ? "running" : "",
        hasResult ? "navigable" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="provider-card-inner">
        <input
          type="checkbox"
          checked={checked}
          disabled={!p.selectable || disabled}
          onChange={onToggle}
          aria-label={`选择 ${p.displayName}`}
        />
        <div
          className={["provider-card-body", hasResult ? "navigable" : ""].filter(Boolean).join(" ")}
          role={hasResult ? "button" : undefined}
          tabIndex={hasResult ? 0 : undefined}
          aria-label={hasResult ? `查看 ${p.displayName} 的诊断结果` : undefined}
          onClick={activateIfPossible}
          onKeyDown={onBodyKeyDown}
        >
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
            {` · ${p.protocolLabel || "未知协议"}`}
          </div>
          <div className="provider-host-model">
            <div className="mono muted ellipsis" title={p.safeBaseUrl}>
              {host}
            </div>
            <div className="ellipsis secondary" title={p.configuredModel ?? ""}>
              {p.configuredModel || "—"}
            </div>
          </div>
          {p.skipReason && (
            <div className="muted provider-skip truncate-2" title={p.skipReason}>
              {p.skipReason}
            </div>
          )}
        </div>
      </div>
    </article>
  );
}
