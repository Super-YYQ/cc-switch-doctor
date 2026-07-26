import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";
import { ResultCard } from "@/components/ResultCard";
import { ProviderRow } from "@/components/ProviderRow";
import { DiagnosisWorkspace } from "@/components/DiagnosisWorkspace";
import type { ProviderDiagnosisSummary, ProviderListItem } from "@/types";
import { isInteractiveTarget, shouldShowRouteDetail, statusBadge } from "@/lib/utils";

async function dismissSafety(user: ReturnType<typeof userEvent.setup>) {
  const knows = await screen.findAllByRole("button", { name: "知道了" });
  await user.click(knows[0]);
}

const sampleProvider: ProviderListItem = {
  opaqueId: "p1",
  sourceId: "s1",
  appType: "claude",
  appLabel: "Claude Code",
  displayName: "GLM Relay",
  safeBaseUrl: "https://api.example.com/v1",
  maskedKey: "sk-abc…123",
  protocolLabel: "Anthropic Messages",
  configuredModel: "glm-5.2",
  isCurrent: true,
  selectable: true,
  authKind: "anthropic_key",
  providerKind: "third_party_api",
};

describe("App UI product shell", () => {
  it("renders product title and compact header", async () => {
    render(<App />);
    expect(await screen.findByText("CC Switch Doctor")).toBeInTheDocument();
    expect(screen.getAllByText(/只读扫描/).length).toBeGreaterThan(0);
  });

  it("opens safety drawer and can dismiss", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    expect(screen.getByRole("button", { name: /开始诊断|重新诊断/ })).toBeInTheDocument();
  });

  it("defaults to Quick mode with concurrency 1 and low-impact copy", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    expect(screen.getByRole("button", { name: "快速验证" }).className).toMatch(/active/);
    expect(screen.getByRole("button", { name: "智能诊断" }).className).not.toMatch(/active/);
    expect(screen.getByRole("tab", { name: "Claude" }).className).toMatch(/active/);
    expect(screen.getByRole("tab", { name: "全部" }).className).not.toMatch(/active/);
    expect(screen.getByRole("tab", { name: "Codex" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Gemini" })).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/sk-[a-zA-Z0-9]{16,}/);
    expect(screen.getAllByText(/低扰动/).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "并发 2" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "并发 3" })).toBeDisabled();
  });

  it("shows multi-request impact notice for smart/deep only", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");
    await user.click(screen.getByRole("checkbox", { name: "选择 GLM Relay" }));
    expect(screen.queryByTestId("multi-request-notice")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "智能诊断" }));
    expect(screen.getByTestId("multi-request-notice")).toHaveTextContent(
      /可能发送多次自动化诊断请求/,
    );
    await user.click(screen.getByRole("button", { name: "深度兼容" }));
    expect(screen.getByTestId("multi-request-notice")).toBeInTheDocument();
    expect(screen.getByText(/Streaming、Tool Calling/)).toBeInTheDocument();
  });

  it("safety drawer documents non-evasion boundary", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText(/无法保证供应商不会识别自动化请求/)).toBeInTheDocument();
    expect(screen.getByText(/不会伪装官方客户端或绕过供应商风控/)).toBeInTheDocument();
    expect(screen.getByText(/仅发送一次标准生成请求/)).toBeInTheDocument();
    await dismissSafety(user);
  });

  it("does not auto-check providers; start disabled until selection", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");
    const glm = screen.getByRole("checkbox", { name: "选择 GLM Relay" }) as HTMLInputElement;
    expect(glm.checked).toBe(false);
    expect(screen.getByRole("button", { name: /开始诊断/ })).toBeDisabled();
    await user.click(glm);
    expect(screen.getByRole("button", { name: /开始诊断/ })).not.toBeDisabled();
  });

  it("concurrency control is available", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    expect(screen.getByRole("button", { name: "并发 1" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "智能诊断" }));
    await user.click(screen.getByRole("button", { name: "并发 3" }));
    expect(screen.getByRole("button", { name: "刷新配置" })).not.toBeDisabled();
  });

  it("refresh clears selection and restores Claude filter", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");
    await user.click(screen.getByRole("checkbox", { name: "选择 GLM Relay" }));
    await user.click(screen.getByRole("tab", { name: "全部" }));
    await user.click(screen.getByRole("button", { name: "刷新配置" }));
    await waitFor(() => {
      expect(screen.getByRole("tab", { name: "Claude" }).className).toMatch(/active/);
      const glm = screen.getByRole("checkbox", { name: "选择 GLM Relay" }) as HTMLInputElement;
      expect(glm.checked).toBe(false);
    });
  });

  it("provider cards are plain articles without role=button", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");
    const articles = document.querySelectorAll("article.provider-card");
    expect(articles.length).toBeGreaterThan(0);
    articles.forEach((a) => {
      expect(a.getAttribute("role")).not.toBe("button");
    });
  });

  it("more menu closes on outside click", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await user.click(screen.getByRole("button", { name: "更多操作" }));
    expect(screen.getByRole("menuitem", { name: "全选当前筛选" })).toBeInTheDocument();
    await user.click(screen.getByText("CC Switch Doctor"));
    await waitFor(() => {
      expect(screen.queryByRole("menuitem", { name: "全选当前筛选" })).not.toBeInTheDocument();
    });
  });

  it("provider cards use compact density class layout", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");
    const card = document.querySelector("article.provider-card") as HTMLElement;
    expect(card.querySelector(".provider-card-inner")).toBeTruthy();
    expect(card.querySelector(".provider-host-model")).toBeTruthy();
    expect(card.querySelector(".provider-footer")).toBeNull();
    expect(screen.queryByRole("button", { name: /查看详情/ })).not.toBeInTheDocument();
  });
});

describe("ProviderRow v0.1.10 navigation gate", () => {
  it("hides details action and does not navigate without a result", async () => {
    const user = userEvent.setup();
    const onActivate = vi.fn();
    const onToggle = vi.fn();
    render(
      <ProviderRow
        provider={sampleProvider}
        checked={false}
        active={false}
        running={false}
        hasResult={false}
        disabled={false}
        onToggle={onToggle}
        onActivate={onActivate}
      />,
    );

    expect(screen.queryByText("查看详情")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /查看 GLM Relay 的诊断结果/ }),
    ).not.toBeInTheDocument();
    expect(document.querySelector("article.provider-card")?.getAttribute("role")).not.toBe(
      "button",
    );
    expect(document.querySelector(".provider-card-body.navigable")).toBeNull();

    await user.click(screen.getByText("GLM Relay"));
    expect(onActivate).not.toHaveBeenCalled();

    await user.click(screen.getByRole("checkbox", { name: "选择 GLM Relay" }));
    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onActivate).not.toHaveBeenCalled();
  });

  it("navigates from body click / keyboard only when a result exists", async () => {
    const user = userEvent.setup();
    const onActivate = vi.fn();
    const onToggle = vi.fn();
    render(
      <ProviderRow
        provider={sampleProvider}
        checked={false}
        active={false}
        running={false}
        hasResult
        resultStatus="RATE_LIMITED"
        disabled={false}
        onToggle={onToggle}
        onActivate={onActivate}
      />,
    );

    expect(screen.queryByText("查看详情")).not.toBeInTheDocument();
    const body = screen.getByRole("button", { name: "查看 GLM Relay 的诊断结果" });
    expect(body.className).toMatch(/navigable/);

    await user.click(body);
    expect(onActivate).toHaveBeenCalledTimes(1);

    body.focus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");
    expect(onActivate).toHaveBeenCalledTimes(3);

    await user.click(screen.getByRole("checkbox", { name: "选择 GLM Relay" }));
    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onActivate).toHaveBeenCalledTimes(3);
  });

  it("keeps protocol, host, model and masked key in the compact three-line layout", () => {
    render(
      <ProviderRow
        provider={sampleProvider}
        checked={false}
        active={false}
        running={false}
        hasResult={false}
        disabled={false}
        onToggle={() => {}}
        onActivate={() => {}}
      />,
    );
    expect(screen.getByText("GLM Relay")).toBeInTheDocument();
    expect(screen.getByText(/Claude Code · sk-abc…123 · Anthropic Messages/)).toBeInTheDocument();
    expect(screen.getByText("api.example.com")).toBeInTheDocument();
    expect(screen.getByText("glm-5.2")).toBeInTheDocument();
    expect(screen.getByText("可诊断")).toBeInTheDocument();
    expect(screen.getByText("当前")).toBeInTheDocument();
  });
});

describe("ResultCard evidence and collapse", () => {
  const base: ProviderDiagnosisSummary = {
    opaqueId: "o1",
    sourceId: "s1",
    displayName: "Relay",
    appLabel: "Claude",
    status: "QUOTA_EXHAUSTED",
    currentConfigOk: false,
    anySuccess: false,
    safeBaseUrl: "https://api.example.com/v1",
    suggestion: "请检查余额",
    evidence: ["尝试 1：POST https://api.example.com/v1 -> 402 (QUOTA_EXHAUSTED)"],
    attempts: [
      {
        ok: false,
        partial: false,
        statusCode: 402,
        latencyMs: 12,
        protocol: "anthropic_messages",
        model: "m",
        url: "https://api.example.com/v1/messages",
        stream: false,
        purpose: "generate",
        classification: "QUOTA_EXHAUSTED",
        errorMessage: "余额不足",
        errorEvidence: [
          {
            source: "structured_flag",
            code: "402",
            matchedKeyword: "余额不足",
            message: "余额不足",
          },
        ],
      },
    ],
    confidence: "high",
  };

  it("shows error evidence section for QUOTA", () => {
    render(<ResultCard summary={base} onCopy={() => {}} />);
    expect(screen.getByText(/判定依据/)).toBeInTheDocument();
    expect(statusBadge("QUOTA_EXHAUSTED").zh).toMatch(/余额|额度/);
    // Debug log is inside a closed <details>, not open by default
    const debug = document.querySelector("pre.debug-log");
    expect(debug).toBeTruthy();
    expect(debug?.closest("details")?.open).toBeFalsy();
  });

  it("shows protocol variant note", () => {
    const s: ProviderDiagnosisSummary = {
      ...base,
      status: "RESPONSE_PROTOCOL_VARIANT_OK",
      anySuccess: true,
      attempts: [
        {
          ...base.attempts[0],
          ok: true,
          classification: "RESPONSE_PROTOCOL_VARIANT_OK",
          suggestionNote: "目标协议：Anthropic Messages；实际返回结构：OpenAI Chat Completions",
          errorEvidence: [],
        },
      ],
    };
    render(<ResultCard summary={s} onCopy={() => {}} />);
    expect(
      screen.getByText(/目标协议：Anthropic Messages；实际返回结构：OpenAI Chat Completions/),
    ).toBeInTheDocument();
  });
});

describe("ResultCard v0.1.10 compact primary and channels", () => {
  const rateLimited: ProviderDiagnosisSummary = {
    opaqueId: "rl-1",
    sourceId: "s-rl",
    displayName: "Provider",
    appLabel: "Claude Code",
    status: "RATE_LIMITED",
    primaryOutcome: "RATE_LIMITED",
    currentConfigOk: false,
    anySuccess: false,
    safeBaseUrl: "https://api.example.com/v1",
    suggestion: "请稍后重试，并检查 Retry-After 或供应商限流策略。",
    evidence: ["尝试 1：POST /v1/messages -> 429"],
    attempts: [
      {
        ok: false,
        partial: false,
        statusCode: 429,
        latencyMs: 20,
        protocol: "anthropic_messages",
        model: "glm-5.2",
        url: "https://api.example.com/v1/messages",
        stream: false,
        purpose: "generate",
        classification: "RATE_LIMITED",
        httpSent: true,
        channel: "direct_upstream",
        errorEvidence: [],
      },
    ],
    confidence: "low",
    direct: {
      attempted: true,
      status: "RATE_LIMITED",
      success: false,
      nativeSuccess: false,
    },
    directStatus: "RATE_LIMITED",
    route: {
      disposition: "not_running",
      attempted: false,
    },
    routeStatus: "CCS_ROUTE_NOT_RUNNING",
  };

  it("shows primary conclusion once and uses compact channel summary for not_running", () => {
    render(<ResultCard summary={rateLimited} onCopy={() => {}} />);
    // Header badge is the single Primary surface; channel summary may restate Direct status.
    expect(document.querySelector(".result-card-head .badge.warn")?.textContent).toContain(
      "请求被限流",
    );
    expect(screen.queryByText("诊断结论")).not.toBeInTheDocument();
    expect(document.querySelector(".result-conclusion")).toBeNull();
    expect(screen.getByText("可信度：低")).toBeInTheDocument();
    expect(screen.getByText(/直连：请求被限流 · 路由：未验证（CCS 未运行）/)).toBeInTheDocument();
    expect(screen.queryByText("上游直连")).not.toBeInTheDocument();
    expect(screen.queryByText("CCS 路由")).not.toBeInTheDocument();
    expect(screen.queryByText("路由未验证")).not.toBeInTheDocument();
    expect(screen.getByText(/真实请求 1 · 未发现成功组合/)).toBeInTheDocument();
    // Evidence tag must not restate the primary Chinese label alone.
    expect(screen.queryByText(/^请求被限流$/)).toBeTruthy();
  });

  it("keeps detailed route evidence when route was attempted", () => {
    const attempted: ProviderDiagnosisSummary = {
      ...rateLimited,
      status: "CCS_ROUTE_OK",
      primaryOutcome: "CCS_ROUTE_OK",
      anySuccess: true,
      currentConfigOk: true,
      confidence: "high",
      route: {
        disposition: "attempted",
        attempted: true,
        generate: { attempted: true, success: true, status: "GENERATE_OK" },
        streaming: { attempted: true, success: false, status: "STREAMING_UNSUPPORTED" },
        actualProviderName: "Relay A",
        actualProviderId: "pid-1",
        failoverCountBefore: 0,
        failoverCountAfter: 1,
      },
      routeStatus: "CCS_ROUTE_OK",
      attempts: [
        {
          ...rateLimited.attempts[0],
          ok: true,
          classification: "GENERATE_OK",
          channel: "ccs_local_route",
          httpSent: true,
        },
      ],
    };
    render(<ResultCard summary={attempted} onCopy={() => {}} />);
    expect(screen.getByText("CCS 路由")).toBeInTheDocument();
    expect(screen.getByText(/基础推理：成功/)).toBeInTheDocument();
    expect(screen.getByText(/流式输出：不支持或失败/)).toBeInTheDocument();
    expect(screen.getByText(/实际处理 Provider：Relay A/)).toBeInTheDocument();
    expect(screen.getByText(/故障转移次数：0 → 1/)).toBeInTheDocument();
  });

  it("compacts model semantics and success combo when present", () => {
    const withModel: ProviderDiagnosisSummary = {
      ...rateLimited,
      status: "CURRENT_CONFIG_OK",
      primaryOutcome: "CURRENT_CONFIG_OK",
      currentConfigOk: true,
      anySuccess: true,
      confidence: "high",
      configuredModel: "GLM-5.2[1M]",
      outboundModel: "GLM-5.2",
      modelTransform: "[1M] 归一化",
      successProtocol: "Anthropic Messages",
      successModel: "GLM-5.2",
      successUrl: "https://api.example.com/v1/messages",
      route: undefined,
      routeStatus: undefined,
      direct: { attempted: true, status: "CURRENT_CONFIG_OK", success: true, nativeSuccess: true },
    };
    render(<ResultCard summary={withModel} onCopy={() => {}} />);
    expect(screen.getByText(/模型：GLM-5.2\[1M\] → GLM-5.2（\[1M\] 归一化）/)).toBeInTheDocument();
    expect(
      screen.getByText(
        /成功组合：Anthropic Messages · GLM-5.2 · https:\/\/api.example.com\/v1\/messages/,
      ),
    ).toBeInTheDocument();
  });
});

describe("DiagnosisWorkspace interaction isolation", () => {
  const summary: ProviderDiagnosisSummary = {
    opaqueId: "o-nav",
    sourceId: "s-nav",
    displayName: "Relay",
    appLabel: "Claude Code",
    status: "RATE_LIMITED",
    primaryOutcome: "RATE_LIMITED",
    currentConfigOk: false,
    anySuccess: false,
    safeBaseUrl: "https://api.example.com/v1",
    suggestion: "请稍后重试",
    evidence: ["e1"],
    attempts: [
      {
        ok: false,
        partial: false,
        statusCode: 429,
        latencyMs: 10,
        protocol: "anthropic_messages",
        model: "m",
        url: "https://api.example.com/v1/messages",
        stream: false,
        purpose: "generate",
        classification: "RATE_LIMITED",
        httpSent: true,
        errorEvidence: [
          {
            source: "status",
            message: "rate limited",
          },
        ],
      },
    ],
    confidence: "low",
  };

  it("activates provider on blank area but not on copy / accordion controls", async () => {
    const user = userEvent.setup();
    const onActivate = vi.fn();
    render(
      <DiagnosisWorkspace
        summaries={[summary]}
        activeId={null}
        providers={[{ ...sampleProvider, opaqueId: "o-nav" }]}
        running={false}
        liveLog={[]}
        onCopy={() => {}}
        onActivateProvider={onActivate}
      />,
    );

    const host = screen.getByText("api.example.com");
    await user.click(host);
    expect(onActivate).toHaveBeenCalledWith("o-nav");
    onActivate.mockClear();

    await user.click(screen.getByRole("button", { name: /复制摘要/ }));
    expect(onActivate).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /复制建议/ }));
    expect(onActivate).not.toHaveBeenCalled();

    await user.click(screen.getByText(/判定依据/));
    expect(onActivate).not.toHaveBeenCalled();

    await user.click(screen.getByText(/尝试链/));
    expect(onActivate).not.toHaveBeenCalled();

    await user.click(screen.getByText("调试日志（高级）"));
    expect(onActivate).not.toHaveBeenCalled();
  });

  it("reveals a filtered-out result when left pane activates that provider", async () => {
    const okSummary: ProviderDiagnosisSummary = {
      ...summary,
      opaqueId: "ok-1",
      displayName: "OK Relay",
      status: "CURRENT_CONFIG_OK",
      primaryOutcome: "CURRENT_CONFIG_OK",
      currentConfigOk: true,
      anySuccess: true,
      confidence: "high",
      suggestion: "可用",
      attempts: [],
    };
    const failSummary: ProviderDiagnosisSummary = {
      ...summary,
      opaqueId: "fail-1",
      displayName: "Fail Relay",
    };

    const { rerender } = render(
      <DiagnosisWorkspace
        summaries={[okSummary, failSummary]}
        activeId={null}
        providers={[
          { ...sampleProvider, opaqueId: "ok-1", displayName: "OK Relay" },
          { ...sampleProvider, opaqueId: "fail-1", displayName: "Fail Relay" },
        ]}
        running={false}
        liveLog={[]}
        onCopy={() => {}}
        onActivateProvider={() => {}}
      />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "正常" }));
    expect(screen.getByText(/Claude Code \/ OK Relay/)).toBeInTheDocument();
    expect(screen.queryByText(/Claude Code \/ Fail Relay/)).not.toBeInTheDocument();

    rerender(
      <DiagnosisWorkspace
        summaries={[okSummary, failSummary]}
        activeId="fail-1"
        providers={[
          { ...sampleProvider, opaqueId: "ok-1", displayName: "OK Relay" },
          { ...sampleProvider, opaqueId: "fail-1", displayName: "Fail Relay" },
        ]}
        running={false}
        liveLog={[]}
        onCopy={() => {}}
        onActivateProvider={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText(/Claude Code \/ Fail Relay/)).toBeInTheDocument();
    });
  });
});

describe("v0.1.10 helper utilities", () => {
  it("detects interactive targets for event filtering", () => {
    const root = document.createElement("div");
    root.innerHTML = `<button id="b">x</button><div id="plain">y</div><details><summary id="s">z</summary></details>`;
    document.body.appendChild(root);
    expect(isInteractiveTarget(root.querySelector("#b"))).toBe(true);
    expect(isInteractiveTarget(root.querySelector("#s"))).toBe(true);
    expect(isInteractiveTarget(root.querySelector("#plain"))).toBe(false);
    root.remove();
  });

  it("only expands route detail for real attempts or complex evidence", () => {
    expect(
      shouldShowRouteDetail({
        route: { disposition: "not_running", attempted: false },
        routeStatus: "CCS_ROUTE_NOT_RUNNING",
        attempts: [],
      }),
    ).toBe(false);
    expect(
      shouldShowRouteDetail({
        route: {
          disposition: "attempted",
          attempted: true,
          generate: { attempted: true, success: true, status: "GENERATE_OK" },
        },
        attempts: [{ channel: "ccs_local_route", httpSent: true }],
      }),
    ).toBe(true);
    expect(
      shouldShowRouteDetail({
        route: {
          disposition: "not_current_target",
          attempted: false,
          actualProviderName: "Other",
        },
      }),
    ).toBe(true);
  });
});

describe("App frozen regressions", () => {
  it("previous/next result navigation remains available after diagnosis", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");
    await user.click(screen.getByRole("checkbox", { name: "选择 GLM Relay" }));
    await user.click(screen.getByRole("tab", { name: "全部" }));
    await user.click(screen.getByRole("checkbox", { name: "选择 MiniMax Codex" }));
    await user.click(screen.getByRole("button", { name: /开始诊断/ }));
    await screen.findByText(/本次诊断完成/);
    expect(screen.getByRole("button", { name: "上一条" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "下一条" })).toBeInTheDocument();
    // Full keys never enter the DOM
    expect(document.body.textContent).not.toMatch(/sk-[a-zA-Z0-9]{16,}/);
  });
});
