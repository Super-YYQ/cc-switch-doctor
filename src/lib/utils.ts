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
    CURRENT_CONFIG_OK: { label: status, kind: "ok", zh: "当前配置可直接使用" },
    CORRECTED_BASE_PATH_OK: { label: status, kind: "info", zh: "修正接口路径后可用" },
    PROTOCOL_FALLBACK_OK: { label: status, kind: "info", zh: "切换协议后可用" },
    AUTH_VARIANT_OK: { label: status, kind: "info", zh: "切换认证方式后可用" },
    MODEL_VARIANT_OK: {
      label: status,
      kind: "info",
      zh: "当前模型不可用，其他模型可用",
    },
    CONFIGURED_MODEL_MAPPING_OK: {
      label: status,
      kind: "info",
      zh: "当前 Provider 配置中的模型映射可用",
    },
    MODEL_GUESS_OK: {
      label: status,
      kind: "info",
      zh: "使用推测模型测试成功，不能代表当前配置已完整验证",
    },
    LOCAL_ROUTING_REQUIRED: {
      label: status,
      kind: "warn",
      zh: "需要 CC Switch 本地路由转换",
    },
    CCS_ROUTE_OK: { label: status, kind: "ok", zh: "当前 CCS 路由链可用" },
    CCS_ROUTE_OK_DIRECT_NATIVE_OK: {
      label: status,
      kind: "ok",
      zh: "CCS 路由链与上游直连均可用",
    },
    CCS_ROUTE_OK_DIRECT_VARIANT: {
      label: status,
      kind: "ok",
      zh: "当前 CCS 路由链可用（上游为跨协议变体）",
    },
    CCS_ROUTE_OK_DIRECT_PARSE_FAILED: {
      label: status,
      kind: "ok",
      zh: "当前 CCS 路由链可用（上游直连解析失败）",
    },
    CCS_ROUTE_NOT_RUNNING: {
      label: status,
      kind: "warn",
      zh: "CCS 路由已配置但未运行",
    },
    CCS_ROUTE_NOT_APPLICABLE: {
      label: status,
      kind: "skip",
      zh: "CCS 路由不适用",
    },
    CCS_ROUTE_TARGET_MISMATCH: {
      label: status,
      kind: "warn",
      zh: "路由实际目标与所选 Provider 不一致",
    },
    CCS_ROUTE_FAILED_DIRECT_OK: {
      label: status,
      kind: "warn",
      zh: "路由失败，但上游直连可用",
    },
    CCS_ROUTE_AND_DIRECT_FAILED: {
      label: status,
      kind: "danger",
      zh: "路由与上游直连均失败",
    },
    RESPONSE_BODY_TOO_LARGE: {
      label: status,
      kind: "warn",
      zh: "响应体超过 2MB 限制",
    },
    RESPONSE_PROTOCOL_VARIANT_OK: {
      label: status,
      kind: "info",
      zh: "目标协议与实际返回结构不同，但解析成功",
    },
    DIRECT_PROTOCOL_VARIANT_OK: {
      label: status,
      kind: "info",
      zh: "上游直连跨协议解析成功，不能证明当前协议配置完整可用",
    },
    DIRECT_NATIVE_OK: { label: status, kind: "ok", zh: "上游直连原生协议成功" },
    DIRECT_LOOSE_TEXT_OK: {
      label: status,
      kind: "info",
      zh: "宽松字段解析到文本，不能证明协议兼容",
    },
    LOOSE_RESPONSE_TEXT_OK: {
      label: status,
      kind: "info",
      zh: "宽松字段解析到文本，不能证明协议兼容",
    },
    GENERATE_OK: { label: status, kind: "ok", zh: "生成请求成功" },
    STREAM_OK: { label: status, kind: "ok", zh: "流式请求成功" },
    STREAM_PROTOCOL_VARIANT_OK: {
      label: status,
      kind: "info",
      zh: "流式跨协议解析成功",
    },
    PARTIAL_TEXT: { label: status, kind: "info", zh: "返回了文本但缺少成功标记" },
    PROVIDER_BUDGET_EXHAUSTED: {
      label: status,
      kind: "warn",
      zh: "已达到本 Provider 真实请求上限",
    },
    TOOL_CALLING_OK: { label: status, kind: "ok", zh: "Tool Calling 可用" },
    KEY_INVALID: { label: status, kind: "danger", zh: "API Key 无效或已失效" },
    AUTH_INVALID: { label: status, kind: "danger", zh: "API Key 无效或已失效" },
    PERMISSION_DENIED: { label: status, kind: "danger", zh: "Key 有效性或权限不足" },
    AUTH_PERMISSION_DENIED: { label: status, kind: "danger", zh: "Key 有效性或权限不足" },
    QUOTA_EXHAUSTED: { label: status, kind: "warn", zh: "余额或额度不足" },
    RATE_LIMITED: { label: status, kind: "warn", zh: "请求被限流" },
    MODEL_NOT_FOUND: {
      label: status,
      kind: "warn",
      zh: "当前模型名或当前分组没有可用渠道",
    },
    ENDPOINT_NOT_FOUND: { label: status, kind: "warn", zh: "接口路径不存在" },
    GATEWAY_OR_WAF: { label: status, kind: "danger", zh: "网关或安全验证页面阻断" },
    RESPONSE_FORMAT_MISMATCH: {
      label: status,
      kind: "warn",
      zh: "返回格式与预期不一致",
    },
    UNSUPPORTED_PROTOCOL: {
      label: status,
      kind: "warn",
      zh: "未发现兼容的协议组合",
    },
    NETWORK_UNREACHABLE: { label: status, kind: "danger", zh: "网络不可达" },
    TLS_ERROR: { label: status, kind: "danger", zh: "TLS 或证书错误" },
    TIMEOUT: { label: status, kind: "warn", zh: "请求超时" },
    HOST_BUDGET_EXHAUSTED: {
      label: status,
      kind: "warn",
      zh: "已达到本次 Host 请求上限",
    },
    HOST_RATE_LIMIT_STOPPED: {
      label: status,
      kind: "warn",
      zh: "连续限流，已停止继续请求",
    },
    STREAMING_UNSUPPORTED: { label: status, kind: "warn", zh: "流式调用不兼容" },
    TOOL_CALLING_UNSUPPORTED: { label: status, kind: "warn", zh: "Tool Calling 不兼容" },
    MANAGED_AUTH_SKIPPED: {
      label: status,
      kind: "skip",
      zh: "托管登录 / OAuth 已安全跳过",
    },
    UNKNOWN_SCHEMA: { label: status, kind: "schema", zh: "Schema 未知，已停止测试" },
    CROSS_ORIGIN_REDIRECT_BLOCKED: {
      label: status,
      kind: "danger",
      zh: "跨 Host 重定向已阻断",
    },
    CANCELLED: { label: status, kind: "skip", zh: "已取消" },
    INVALID_REQUEST_PARAMETER: {
      label: status,
      kind: "warn",
      zh: "请求参数不被接口支持",
    },
    UNKNOWN_ERROR: { label: status, kind: "warn", zh: "未知错误，请查看尝试链" },
  };
  return (
    map[status] ?? {
      label: status,
      kind: "warn",
      zh: "诊断未给出明确结论，请查看尝试链",
    }
  );
}

export function possibleCauses(status: string): string[] | null {
  if (
    status === "UNSUPPORTED_PROTOCOL" ||
    status === "RESPONSE_FORMAT_MISMATCH" ||
    status === "UNKNOWN_ERROR"
  ) {
    return [
      "当前接口路径或协议格式不匹配",
      "上游返回了非标准错误结构",
      "Key、权限或余额错误未使用标准 HTTP 状态",
      "中转站返回了网关/WAF 页面",
    ];
  }
  return null;
}

export function estimateClientSide(count: number, mode: "quick" | "smart" | "deep"): number {
  const per = mode === "quick" ? 2 : mode === "smart" ? 12 : 16;
  return count * per;
}

export function modeDescription(mode: "quick" | "smart" | "deep"): string {
  if (mode === "quick") {
    return "快速验证：只优先测试当前配置，速度最快、Token 最低。";
  }
  if (mode === "smart") {
    return "智能诊断：失败后自动尝试同 Host 的 URL、协议、认证和模型变体（最多约 12 次/配置）。";
  }
  return "深度兼容：在智能诊断基础上测试 Streaming、Tool Calling 与稳定性（最多约 16 次/配置）。";
}

export function modeTooltip(mode: "quick" | "smart" | "deep"): string {
  if (mode === "quick") {
    return "只优先测试当前配置的 URL、协议、认证方式和模型。速度最快、Token 消耗最低，适合日常确认。不进行大范围变体、Streaming 或 Tool Calling。";
  }
  if (mode === "smart") {
    return "先测当前配置；失败后按错误类型尝试同 Host 的安全 URL、协议、认证和模型变体。适合排查 /v1、协议格式、认证 Header、模型名。每配置最多约 12 次真实请求。";
  }
  return "在智能诊断基础上继续测试 Streaming、Tool Calling 和稳定性复测。耗时和 Token 最高。每配置最多约 16 次真实请求，仍遵守 Host 30 次会话上限。";
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

/** True when a click target is itself an interactive control that should not trigger pane navigation. */
export function isInteractiveTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    !!target.closest("button, a, input, select, textarea, summary, details, [role='button']")
  );
}

const SIMPLE_ROUTE_DISPOSITIONS = new Set([
  "not_requested",
  "not_configured",
  "not_running",
  "not_current_target",
  "unsupported_app",
  "blocked_non_loopback",
]);

/** Whether the route channel needs the expanded detail panel (real attempt / complex evidence). */
export function shouldShowRouteDetail(summary: {
  route?: {
    disposition?: string | null;
    attempted?: boolean | null;
    generate?: unknown;
    streaming?: unknown;
    actualProviderName?: string | null;
    actualProviderId?: string | null;
    failoverCountBefore?: number | null;
    failoverCountAfter?: number | null;
    notice?: string | null;
  } | null;
  routeStatus?: string | null;
  routeSideEffectNotice?: string | null;
  attempts?: { channel?: string | null; httpSent?: boolean }[];
}): boolean {
  const routeAttempts = (summary.attempts ?? []).filter((a) => a.channel === "ccs_local_route");
  if (summary.route?.attempted === true || routeAttempts.some((a) => a.httpSent)) return true;
  if (summary.route?.actualProviderName || summary.route?.actualProviderId) return true;
  if (summary.route?.failoverCountBefore != null || summary.route?.failoverCountAfter != null) {
    return true;
  }
  if (summary.route?.notice || summary.routeSideEffectNotice) return true;
  if (summary.route?.generate || summary.route?.streaming) return true;
  if ((summary.route?.disposition || "").toLowerCase() === "attempted") return true;
  if (summary.routeStatus === "CCS_ROUTE_TARGET_MISMATCH") return true;
  return false;
}

export function directChannelLabel(summary: {
  direct?: { status?: string | null; success?: boolean } | null;
  directStatus?: string | null;
  attempts?: { channel?: string | null; ok?: boolean }[];
}): string {
  const directStatusCode = summary.direct?.status || summary.directStatus || null;
  if (directStatusCode) return statusBadge(directStatusCode).zh;
  const directAttempts = (summary.attempts ?? []).filter(
    (a) => !a.channel || a.channel === "direct_upstream",
  );
  if (directAttempts.some((a) => a.ok) || summary.direct?.success) return "直连成功";
  if (directAttempts.length) return "直连未成功";
  return "未执行直连";
}

/** Compact one-line route status for simple (non-attempted) dispositions. */
export function routeChannelSummaryText(
  disposition?: string | null,
  routeStatus?: string | null,
): string {
  const d = (disposition || "").toLowerCase();
  if (d === "not_running" || routeStatus === "CCS_ROUTE_NOT_RUNNING") {
    return "未验证（CCS 未运行）";
  }
  if (d === "not_configured" || routeStatus === "CCS_ROUTE_NOT_APPLICABLE") {
    return "未验证（未配置或不适用）";
  }
  if (d === "not_current_target") return "未验证（非当前目标）";
  if (d === "blocked_non_loopback") return "未验证（非 loopback）";
  if (d === "unsupported_app") return "未验证（应用不支持）";
  if (d === "not_requested") return "未请求";
  if (SIMPLE_ROUTE_DISPOSITIONS.has(d)) {
    const label = routeDispositionLabel(disposition, routeStatus);
    return label.detail && label.detail !== label.title
      ? `${label.title}（${label.detail}）`
      : label.title;
  }
  const label = routeDispositionLabel(disposition, routeStatus);
  return label.detail && label.detail !== label.title
    ? `${label.title}（${label.detail}）`
    : label.title;
}

/** Primary badge code: prefer primaryOutcome; never invent route disposition as primary. */
export function primaryStatusCode(summary: {
  status: string;
  primaryOutcome?: string | null;
}): string {
  return summary.primaryOutcome || summary.status;
}

export type RouteDisposition =
  | "not_requested"
  | "not_configured"
  | "not_running"
  | "not_current_target"
  | "unsupported_app"
  | "blocked_non_loopback"
  | "attempted";

/** Neutral Chinese copy for route disposition (auxiliary only — never primary badge). */
export function routeDispositionLabel(
  disposition?: string | null,
  routeStatus?: string | null,
): { title: string; detail: string; kind: StatusKind } {
  const d = (disposition || "").toLowerCase();
  if (d === "attempted" || routeStatus === "CCS_ROUTE_OK") {
    return {
      title: "已验证",
      detail: routeStatus ? statusBadge(routeStatus).zh : "CCS 路由业务请求已发送",
      kind: routeStatus ? statusBadge(routeStatus).kind : "ok",
    };
  }
  if (d === "not_running" || routeStatus === "CCS_ROUTE_NOT_RUNNING") {
    return {
      title: "未验证",
      detail: "CCS 路由已配置但未运行",
      kind: "skip",
    };
  }
  if (d === "not_current_target") {
    return {
      title: "未验证",
      detail: "该 Provider 不是当前 CCS 路由目标",
      kind: "skip",
    };
  }
  if (d === "not_requested") {
    return {
      title: "未请求",
      detail: "本次未执行 CCS 路由验证（例如仅直连模式）",
      kind: "skip",
    };
  }
  if (d === "blocked_non_loopback") {
    return {
      title: "未验证",
      detail: "监听地址非 loopback，已禁止路由探测",
      kind: "skip",
    };
  }
  if (d === "unsupported_app") {
    return {
      title: "未验证",
      detail: "当前应用类型不支持路由协议探测",
      kind: "skip",
    };
  }
  if (d === "not_configured" || routeStatus === "CCS_ROUTE_NOT_APPLICABLE") {
    return {
      title: "未验证",
      detail: "CCS 路由配置不可用或不适用",
      kind: "skip",
    };
  }
  if (routeStatus) {
    const b = statusBadge(routeStatus);
    return { title: b.zh, detail: routeStatus, kind: b.kind };
  }
  return { title: "—", detail: "无路由信息", kind: "skip" };
}

/** Group attempt evidence lines by URL/protocol for default (non-debug) display. */
export function groupAttemptsByCanonical(
  attempts: {
    url: string;
    protocol: string;
    stream?: boolean;
    classification: string;
    httpSent?: boolean;
    reusedFromCache?: boolean;
    ok?: boolean;
  }[],
): { key: string; label: string; realSends: number; cacheHits: number; finalStatus: string }[] {
  const map = new Map<
    string,
    { label: string; realSends: number; cacheHits: number; finalStatus: string }
  >();
  for (const a of attempts) {
    const path = (() => {
      try {
        return new URL(a.url).pathname || a.url;
      } catch {
        return a.url;
      }
    })();
    const key = `${path}|${a.protocol}|${a.stream ? "stream" : "post"}`;
    const label = `${path} · ${a.protocol}${a.stream ? " · stream" : ""}`;
    const prev = map.get(key) || {
      label,
      realSends: 0,
      cacheHits: 0,
      finalStatus: a.classification,
    };
    if (a.httpSent) prev.realSends += 1;
    if (a.reusedFromCache) prev.cacheHits += 1;
    prev.finalStatus = a.classification;
    map.set(key, prev);
  }
  return [...map.entries()].map(([key, v]) => ({ key, ...v }));
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

export function versionVerificationLabel(v?: string | null): string {
  switch (v) {
    case "verified":
      return "已验证";
    case "known_compatible":
      return "已知兼容";
    case "unverified_structure_compatible":
      return "结构兼容（尚未完整验证）";
    case "unknown":
      return "未知";
    default:
      return "—";
  }
}

export function versionVerificationKind(v?: string | null): StatusKind {
  switch (v) {
    case "verified":
      return "ok";
    case "known_compatible":
    case "unverified_structure_compatible":
      return "warn";
    case "unknown":
      return "schema";
    default:
      return "skip";
  }
}

export function capabilityLabel(state?: string | null): string {
  switch (state) {
    case "supported":
      return "可用";
    case "degraded":
      return "降级可用";
    case "disabled":
      return "不可用";
    default:
      return "—";
  }
}

export function capabilityKind(state?: string | null): StatusKind {
  switch (state) {
    case "supported":
      return "ok";
    case "degraded":
      return "warn";
    case "disabled":
      return "danger";
    default:
      return "skip";
  }
}

export function defaultSelectedIds(providers: ProviderListItem[]): Set<string> {
  return new Set(providers.filter((p) => p.isCurrent && p.selectable).map((p) => p.opaqueId));
}
