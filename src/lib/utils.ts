import type { ProviderListItem } from "@/types";

export function filterProviders(
  providers: ProviderListItem[],
  opts: {
    app: string;
    query: string;
    onlySelectable?: boolean;
  },
): ProviderListItem[] {
  const q = opts.query.trim().toLowerCase();
  return providers.filter((p) => {
    if (opts.app !== "all" && p.appType !== opts.app) return false;
    if (opts.onlySelectable && !p.selectable) return false;
    if (!q) return true;
    const hay = [
      p.displayName,
      p.appLabel,
      p.safeBaseUrl,
      p.configuredModel ?? "",
      p.sourceId,
      p.protocolLabel ?? "",
    ]
      .join(" ")
      .toLowerCase();
    return hay.includes(q);
  });
}

export function statusBadge(status: string): {
  label: string;
  kind: "ok" | "warn" | "danger" | "skip";
} {
  if (
    status === "CURRENT_CONFIG_OK" ||
    status === "CORRECTED_BASE_PATH_OK" ||
    status === "PROTOCOL_FALLBACK_OK" ||
    status === "AUTH_VARIANT_OK" ||
    status === "MODEL_VARIANT_OK"
  ) {
    return { label: status, kind: "ok" };
  }
  if (status === "MANAGED_AUTH_SKIPPED" || status === "LOCAL_ROUTING_REQUIRED") {
    return { label: status, kind: status === "MANAGED_AUTH_SKIPPED" ? "skip" : "warn" };
  }
  if (
    status === "KEY_INVALID" ||
    status === "QUOTA_EXHAUSTED" ||
    status === "NETWORK_UNREACHABLE" ||
    status === "TLS_ERROR"
  ) {
    return { label: status, kind: "danger" };
  }
  return { label: status, kind: "warn" };
}

export function estimateClientSide(count: number, mode: "quick" | "smart" | "deep"): number {
  const per = mode === "quick" ? 2 : mode === "smart" ? 12 : 16;
  return count * per;
}

export function assertNoFullKeyInDom(text: string): boolean {
  // Heuristic used in tests: long sk- keys should not appear
  return !/sk-[a-zA-Z0-9]{16,}/.test(text);
}
