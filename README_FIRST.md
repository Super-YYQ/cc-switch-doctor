# 使用说明

把本目录中的 5 个 Markdown 文件全部放到 `Super-YYQ/cc-switch-doctor` 本地仓库根目录：

```text
PROJECT_SPEC.md
UI_UX_ADDENDUM.md
UI_WIREFRAME_COMPONENT_SPEC.md
FINAL_GOAL_PROMPT.md
README_FIRST.md
```

然后把 `FINAL_GOAL_PROMPT.md` 中“【开始执行】”到“【结束执行】”的全部内容，作为一次 Goal 交给 Codex、Claude Code 或其他具备终端、GitHub 和 Windows 构建权限的 AI 编程工具。

推荐先确认：

```powershell
git remote -v
gh auth status
gh repo view Super-YYQ/cc-switch-doctor
git status --short
```

注意：

- 不要同时保留旧版、同名但内容不同的规格文件；
- 已有 v0.1.0 tag/release 时，Prompt 会要求 AI 自动发布下一个未占用 patch；
- 首版固定 unsigned，不需要准备代码签名证书或额外 GitHub Secrets；
- GitHub Actions 的 Workflow permissions 需保持 `Read and write permissions`。
