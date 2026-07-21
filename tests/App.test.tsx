import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";
import { ResultCard } from "@/components/ResultCard";
import type { ProviderDiagnosisSummary } from "@/types";
import { statusBadge } from "@/lib/utils";

async function dismissSafety(user: ReturnType<typeof userEvent.setup>) {
  const knows = await screen.findAllByRole("button", { name: "知道了" });
  await user.click(knows[0]);
}

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

  it("defaults to Claude filter and smart mode; core filters always present", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    expect(screen.getByRole("button", { name: "智能诊断" }).className).toMatch(/active/);
    expect(screen.getByRole("tab", { name: "Claude" }).className).toMatch(/active/);
    expect(screen.getByRole("tab", { name: "全部" }).className).not.toMatch(/active/);
    expect(screen.getByRole("tab", { name: "Codex" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Gemini" })).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/sk-[a-zA-Z0-9]{16,}/);
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
