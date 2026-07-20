import type { ProviderListItem } from "@/types";

export type StatusKind = "ok" | "info" | "warn" | "danger" | "skip" | "schema" | "primary";

export function filterProviders(
  providers: ProviderListItem[],
  opts: {
    app: string;
    query: string;
    onlySelectable?: boolean;
    onlySelected?: boolean;
    selected?: Set<string>;
  },
): ProviderListItem[] {
  const q = opts.query.trim().toLowerCase();
  return providers.filter((p) => {
    if (opts.app !== "all" && p.appType !== opts.app) return false;
    if (opts.onlySelectable && !p.selectable) return false;
    if (opts.onlySelected && opts.selected && !opts.selected.has(p.opaqueId)) return false;
    if (!q) return true;
    const hay = [
      p.displayName,
      p.appLabel,
      p.safeBaseUrl,
      p.configuredModel ?? "",
      p.sourceId,
      p.protocolLabel ?? "",
      p.maskedKey,
    ]
      .join(" ")
      .toLowerCase();
    return hay.includes(q);
  });
}

export function statusBadge(status: string): { label: string; kind: StatusKind; zh: string } {
  const map: Record<string, { label: string; kind: StatusKind; zh: string }> = {
    CURRENT_CONFIG_OK: {
      label: status,
      kind: "ok",
      zh: "当前配置可直接使用",
    },
    CORRECTED_BASE_PATH_OK: {
      label: status,
      kind: "info",
      zh: "修正 Base URL / 路径后可用",
    },
    PROTOCOL_FALLBACK_OK: {
      label: status,
      kind: "info",
      zh: "更换协议后可用",
    },
    AUTH_VARIANT_OK: {
      label: status,
      kind: "info",
      zh: "调整认证格式后可用",
    },
    MODEL_VARIANT_OK: {
      label: status,
      kind: "info",
      zh: "更换模型后可用",
    },
    LOCAL_ROUTING_REQUIRED: {
      label: status,
      kind: "warn",
      zh: "上游 API 可用，但可能需要 CC Switch 本地路由",
    },
    KEY_INVALID: {
      label: status,
      kind: "danger",
      zh: "API Key 无效或未授权",
    },
    PERMISSION_DENIED: {
      label: status,
      kind: "danger",
      zh: "权限不足",
    },
    QUOTA_EXHAUSTED: {
      label: status,
      kind: "warn",
      zh: "额度不足或配额耗尽",
    },
    RATE_LIMITED: {
      label: status,
      kind: "warn",
      zh: "触发限流，请稍后重试",
    },
    MODEL_NOT_FOUND: {
      label: status,
      kind: "warn",
      zh: "模型不存在或无权访问",
    },
    ENDPOINT_NOT_FOUND: {
      label: status,
      kind: "warn",
      zh: "端点不存在，可尝试修正 /v1 或协议",
    },
    NETWORK_UNREACHABLE: {
      label: status,
      kind: "danger",
      zh: "网络不可达",
    },
    TLS_ERROR: {
      label: status,
      kind: "danger",
      zh: "TLS / 证书错误",
    },
    TIMEOUT: {
      label: status,
      kind: "warn",
      zh: "请求超时",
    },
    STREAMING_UNSUPPORTED: {
      label: status,
      kind: "warn",
      zh: "流式调用不兼容",
    },
    TOOL_CALLING_UNSUPPORTED: {
      label: status,
      kind: "warn",
      zh: "Tool Calling 不兼容",
    },
    MANAGED_AUTH_SKIPPED: {
      label: status,
      kind: "skip",
      zh: "托管登录 / OAuth 已安全跳过",
    },
    UNKNOWN_SCHEMA: {
      label: status,
      kind: "schema",
      zh: "Schema 未知，已停止测试",
    },
    CROSS_ORIGIN_REDIRECT_BLOCKED: {
      label: status,
      kind: "danger",
      zh: "跨 Host 重定向已阻断",
    },
    CANCELLED: {
      label: status,
      kind: "skip",
      zh: "已取消",
    },
    UNKNOWN_ERROR: {
      label: status,
      kind: "warn",
      zh: "未知错误，请查看尝试链",
    },
  };
  return (
    map[status] ?? {
      label: status,
      kind: "warn",
      zh: status,
    }
  );
}

export function estimateClientSide(count: number, mode: "quick" | "smart" | "deep"): number {
  const per = mode === "quick" ? 2 : mode === "smart" ? 12 : 16;
  return count * per;
}

export function assertNoFullKeyInDom(text: string): boolean {
  return !/sk-[a-zA-Z0-9]{16,}/.test(text);
}

export function hostFromUrl(raw: string): string {
  try {
    const u = new URL(raw);
    return u.host || raw;
  } catch {
    return raw.replace(/^https?:\/\//, "").split("/")[0] || raw;
  }
}

export function confidenceLabel(c: string): string {
  if (c === "high") return "高";
  if (c === "medium") return "中";
  if (c === "low") return "低";
  return c;
}

export function schemaKind(status?: string | null): StatusKind {
  if (status === "verified") return "ok";
  if (status === "compatible") return "warn";
  if (status === "unknown" || status === "unsupported") return "schema";
  return "skip";
}

export function schemaLabel(status?: string | null): string {
  if (status === "verified") return "Schema 已验证";
  if (status === "compatible") return "Schema 兼容";
  if (status === "unknown") return "Schema 未知";
  if (status === "unsupported") return "Schema 不支持";
  return "Schema —";
}
