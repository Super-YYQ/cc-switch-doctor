import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";

describe("App UI", () => {
  it("renders title and safety notice", () => {
    render(<App />);
    expect(screen.getByText("CC Switch Doctor")).toBeInTheDocument();
    expect(screen.getByText(/安全说明/)).toBeInTheDocument();
  });

  it("does not auto-start tests and keeps managed row uncheckable", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: "知道了" }));
    const start = screen.getByRole("button", { name: /开始测试/ });
    expect(start).toBeDisabled();
    // demo providers include OAuth skipped
    expect(screen.getAllByText(/安全跳过/).length).toBeGreaterThan(0);
    // no full key in document
    expect(document.body.textContent).not.toMatch(/sk-[a-zA-Z0-9]{16,}/);
  });

  it("defaults to smart mode", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: "知道了" }));
    const smart = screen.getByLabelText("智能诊断") as HTMLInputElement;
    expect(smart.checked).toBe(true);
  });
});
