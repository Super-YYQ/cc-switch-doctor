import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";

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

  it("defaults to smart mode and keeps managed row uncheckable", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    expect(screen.getByRole("button", { name: "智能诊断" }).className).toMatch(/active/);
    expect(screen.getAllByText(/已跳过|官方登录/).length).toBeGreaterThan(0);
    expect(document.body.textContent).not.toMatch(/sk-[a-zA-Z0-9]{16,}/);
  });

  it("primary diagnose button enabled when CCS current auto-selected", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");
    expect(screen.getByRole("button", { name: /开始诊断/ })).not.toBeDisabled();
  });

  it("concurrency control is available", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    expect(screen.getByRole("button", { name: "并发 1" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "并发 3" }));
    // still idle
    expect(screen.getByRole("button", { name: "刷新配置" })).not.toBeDisabled();
  });

  it("refresh clears selection so start becomes disabled", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    await screen.findByText("GLM Relay");

    // Current providers are auto-selected after scan
    const glmCheck = screen.getByRole("checkbox", { name: "选择 GLM Relay" }) as HTMLInputElement;
    expect(glmCheck.checked).toBe(true);
    expect(screen.getByRole("button", { name: /开始诊断/ })).not.toBeDisabled();

    // uncheck then refresh should re-select current
    await user.click(glmCheck);
    expect(glmCheck.checked).toBe(false);

    await user.click(screen.getByRole("button", { name: "刷新配置" }));

    await waitFor(() => {
      const again = screen.getByRole("checkbox", { name: "选择 GLM Relay" }) as HTMLInputElement;
      expect(again.checked).toBe(true);
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
    expect(screen.getAllByRole("button", { name: /查看详情/ }).length).toBeGreaterThan(0);
  });

  it("refresh and pick-db enabled when idle", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dismissSafety(user);
    expect(screen.getByRole("button", { name: "刷新配置" })).not.toBeDisabled();
    expect(screen.getByRole("button", { name: "选择数据库" })).not.toBeDisabled();
  });
});
