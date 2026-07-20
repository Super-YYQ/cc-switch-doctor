import { describe, expect, it } from "vitest";
import {
  assertNoFullKeyInDom,
  estimateClientSide,
  filterProviders,
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
});

describe("statusBadge", () => {
  it("marks success and danger", () => {
    expect(statusBadge("CURRENT_CONFIG_OK").kind).toBe("ok");
    expect(statusBadge("KEY_INVALID").kind).toBe("danger");
    expect(statusBadge("MANAGED_AUTH_SKIPPED").kind).toBe("skip");
  });
});

describe("estimate and key safety", () => {
  it("estimates requests", () => {
    expect(estimateClientSide(2, "smart")).toBe(24);
  });

  it("detects full keys", () => {
    expect(assertNoFullKeyInDom("sk-abcdefghijklmnopqrstuvwxyz")).toBe(false);
    expect(assertNoFullKeyInDom("sk-tes…1234")).toBe(true);
  });
});
