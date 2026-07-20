import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";

describe("App UI product shell", () => {
  it("renders product title and compact header", () => {
    render(<App />);
    expect(screen.getByText("CC Switch Doctor")).toBeInTheDocument();
    expect(screen.getAllByText(/只读扫描/).length).toBeGreaterThan(0);
  });

  it("opens safety drawer and can dismiss", async () => {
    const user = userEvent.setup();
    render(<App />);
    // drawer may auto-open on load
    const knows = await screen.findAllByRole("button", { name: "知道了" });
    await user.click(knows[0]);
    expect(screen.getByRole("button", { name: /开始诊断|重新诊断/ })).toBeInTheDocument();
  });

  it("defaults to smart mode and keeps managed row uncheckable", async () => {
    const user = userEvent.setup();
    render(<App />);
    const knows = await screen.findAllByRole("button", { name: "知道了" });
    await user.click(knows[0]);
    expect(screen.getByRole("button", { name: "智能诊断" }).className).toMatch(/active/);
    expect(screen.getAllByText(/已跳过|官方登录/).length).toBeGreaterThan(0);
    expect(document.body.textContent).not.toMatch(/sk-[a-zA-Z0-9]{16,}/);
  });

  it("primary diagnose button disabled without selection", async () => {
    const user = userEvent.setup();
    render(<App />);
    const knows = await screen.findAllByRole("button", { name: "知道了" });
    await user.click(knows[0]);
    const start = screen.getByRole("button", { name: /开始诊断/ });
    expect(start).toBeDisabled();
  });
});
