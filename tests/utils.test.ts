import { describe, expect, it } from "vitest";
import {
  assertNoFullKeyInDom,
  estimateClientSide,
  filterProviders,
  groupAttemptsByCanonical,
  hostFromUrl,
  primaryStatusCode,
  routeDispositionLabel,
  statusBadge,
} from "@/lib/utils";
import type { ProviderListItem } from "@/types";

const sample: ProviderListItem[] = [
  {
    opaqueId: "1",
    sourceId: "a",
    appType: "claude",
    appLabel: "Claude Code",
    displayName: "GLM",
    safeBaseUrl: "https://api.example.com/v1",
    maskedKey: "sk-tes…1234",
    selectable: true,
    isCurrent: true,
    authKind: "anthropic_key",
    providerKind: "third_party_api",
    configuredModel: "glm-4",
    protocolLabel: "Anthropic Messages",
  },
  {
    opaqueId: "2",
    sourceId: "b",
    appType: "codex",
    appLabel: "Codex",
    displayName: "Official",
    safeBaseUrl: "—",
    maskedKey: "",
    selectable: false,
    isCurrent: false,
    skipReason: "安全跳过：Codex OAuth",
    authKind: "codex_oauth",
    providerKind: "managed_account",
  },
];

describe("filterProviders", () => {
  it("filters by app and query", () => {
    expect(filterProviders(sample, { app: "claude", query: "" })).toHaveLength(1);
    expect(filterProviders(sample, { app: "all", query: "Official" })).toHaveLength(1);
    expect(filterProviders(sample, { app: "all", query: "glm" })).toHaveLength(1);
  });

  it("can keep only selectable", () => {
    expect(filterProviders(sample, { app: "all", query: "", onlySelectable: true })).toHaveLength(
      1,
    );
  });

  it("supports onlySelected", () => {
    const selected = new Set(["1"]);
    expect(
      filterProviders(sample, { app: "all", query: "", onlySelected: true, selected }),
    ).toHaveLength(1);
  });
});

describe("statusBadge", () => {
  it("marks success and danger with Chinese copy", () => {
    expect(statusBadge("CURRENT_CONFIG_OK").kind).toBe("ok");
    expect(statusBadge("CURRENT_CONFIG_OK").zh).toContain("使用");
    expect(statusBadge("KEY_INVALID").kind).toBe("danger");
    expect(statusBadge("MANAGED_AUTH_SKIPPED").kind).toBe("skip");
  });

  // v0.1.7 P0: primary badge must reflect real direct outcomes, not route disposition labels alone.
  it("keeps network / auth failures as danger primary outcomes", () => {
    expect(statusBadge("NETWORK_UNREACHABLE").kind).toBe("danger");
    expect(statusBadge("NETWORK_UNREACHABLE").zh).toContain("网络");
    expect(statusBadge("AUTH_INVALID").kind).toBe("danger");
    expect(statusBadge("AUTH_PERMISSION_DENIED").kind).toBe("danger");
    expect(statusBadge("QUOTA_EXHAUSTED").kind).toBe("warn");
    expect(statusBadge("MODEL_NOT_FOUND").kind).toBe("warn");
    expect(statusBadge("ENDPOINT_NOT_FOUND").kind).toBe("warn");
    expect(statusBadge("TLS_ERROR").kind).toBe("danger");
  });

  it("treats route disposition codes as auxiliary labels only", () => {
    // These may still appear in routeStatus chips, never as sole primary when route was not sent.
    expect(statusBadge("CCS_ROUTE_NOT_APPLICABLE").kind).toBe("skip");
    expect(statusBadge("CCS_ROUTE_NOT_RUNNING").kind).toBe("warn");
    expect(statusBadge("CCS_ROUTE_OK_DIRECT_NATIVE_OK").kind).toBe("ok");
    expect(statusBadge("CCS_ROUTE_FAILED_DIRECT_OK").kind).toBe("warn");
    expect(statusBadge("CCS_ROUTE_AND_DIRECT_FAILED").kind).toBe("danger");
  });
});

describe("primaryStatusCode and routeDispositionLabel", () => {
  it("prefers primaryOutcome over legacy status", () => {
    expect(
      primaryStatusCode({
        status: "CCS_ROUTE_NOT_APPLICABLE",
        primaryOutcome: "NETWORK_UNREACHABLE",
      }),
    ).toBe("NETWORK_UNREACHABLE");
    expect(primaryStatusCode({ status: "AUTH_INVALID" })).toBe("AUTH_INVALID");
  });

  it("maps not_current_target to neutral 未验证 copy", () => {
    const d = routeDispositionLabel("not_current_target", "CCS_ROUTE_NOT_APPLICABLE");
    expect(d.title).toBe("未验证");
    expect(d.detail).toContain("不是当前");
    expect(d.kind).toBe("skip");
  });

  it("maps not_requested for DirectOnly", () => {
    const d = routeDispositionLabel("not_requested");
    expect(d.title).toBe("未请求");
    expect(d.kind).toBe("skip");
  });
});

describe("groupAttemptsByCanonical", () => {
  it("collapses cache reuse into real-send counts", () => {
    const groups = groupAttemptsByCanonical([
      {
        url: "https://api.example.com/v1/messages",
        protocol: "Anthropic Messages",
        classification: "NETWORK_UNREACHABLE",
        httpSent: true,
      },
      {
        url: "https://api.example.com/v1/messages",
        protocol: "Anthropic Messages",
        classification: "NETWORK_UNREACHABLE",
        reusedFromCache: true,
      },
      {
        url: "https://api.example.com/v1/messages",
        protocol: "Anthropic Messages",
        classification: "NETWORK_UNREACHABLE",
        reusedFromCache: true,
      },
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].realSends).toBe(1);
    expect(groups[0].cacheHits).toBe(2);
    expect(groups[0].finalStatus).toBe("NETWORK_UNREACHABLE");
  });
});

describe("estimate, host and key safety", () => {
  it("estimates requests", () => {
    expect(estimateClientSide(2, "smart")).toBe(24);
  });

  it("detects full keys", () => {
    expect(assertNoFullKeyInDom("sk-abcdefghijklmnopqrstuvwxyz")).toBe(false);
    expect(assertNoFullKeyInDom("sk-tes…1234")).toBe(true);
  });

  it("extracts host", () => {
    expect(hostFromUrl("https://api.example.com/v1/chat")).toBe("api.example.com");
  });
});
