# CC Switch Doctor

**只读 · 无状态 · 纯 HTTP** 的 CC Switch 第三方供应商诊断工具。

> 扫描本地 CC Switch 数据库，勾选配置，通过真实低成本模型请求定位 Key / URL / 协议 / 模型 / 额度问题。  
> **绝不**启动 Codex、Claude、OpenCode、Gemini CLI 或 CC Switch；**绝不**读取官方登录目录；**绝不**写入 CC Switch 数据库。

[![CI](https://github.com/Super-YYQ/cc-switch-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/Super-YYQ/cc-switch-doctor/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Super-YYQ/cc-switch-doctor)](https://github.com/Super-YYQ/cc-switch-doctor/releases)

## 截图

| 空状态 | 已选择 | 混合结果 |
| --- | --- | --- |
| ![main-empty](docs/screenshots/main-empty.png) | ![main-selected](docs/screenshots/main-selected.png) | ![results-mixed](docs/screenshots/results-mixed.png) |

## 安全保证

| 保证     | 说明                                               |
| -------- | -------------------------------------------------- |
| 纯 HTTP  | 仅 `reqwest` 发请求，CI 禁止 process spawn         |
| 登录隔离 | 不读 `.codex` / `.claude` / OpenCode / `.gemini`   |
| DB 只读  | `mode=ro` + `query_only=ON`，仅 SELECT/PRAGMA      |
| Key 内存 | 完整 Key 不进前端、日志、文件、localStorage        |
| 同源     | 自动变体仅限原 Base URL 同一 Host                  |
| 无状态   | 关闭后无历史、无选择、无结果持久化                 |
| 托管跳过 | OAuth / Copilot / ChatGPT Backend 等硬跳过，无绕过 |

## 支持范围（v0.1.1）

- 平台：Windows 10/11 x64
- 应用：Claude Code、Claude Desktop、Codex、Gemini CLI、OpenCode、OpenClaw、Hermes、Grok Build
- 协议：OpenAI Chat Completions、OpenAI Responses、Anthropic Messages、Gemini Native
- 模式：快速验证 / 智能诊断 / 深度兼容性（含流式与 Tool Calling）

## 证据边界

本工具验证的是**上游 HTTP API**是否可用。它**不能**证明 Codex/Claude CLI 端到端完整链路（本地路由、客户端特有头等）。若 Chat 可用而 Responses 不可用，会标注为可能需要 CC Switch 本地路由。

## 安装

### 安装版

下载 Release 中的 `CC-Switch-Doctor-v*-Windows-x64-setup.exe`（当前用户，无需管理员）。

### 便携版

下载 `CC-Switch-Doctor-v*-Windows-x64-portable.zip`，解压后运行 `CC-Switch-Doctor.exe`。

### 校验

使用 `SHA256SUMS.txt` 校验下载文件。

### 未签名与 SmartScreen

若 Release 标注 **unsigned**，Windows SmartScreen 可能提示“未知发布者”。这是缺少代码签名证书所致。可选择“更多信息 → 仍要运行”，或用 `SHA256SUMS.txt` 校验后再运行。需要 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。

## 使用

1. 先安装并至少打开过一次 [CC Switch](https://github.com/farion1231/cc-switch)。
2. 启动 Doctor，确认 DB 已连接、兼容状态为 verified/compatible。
3. 按应用筛选，勾选第三方配置（官方/OAuth 会灰显跳过）。
4. 选择模式（默认智能诊断），查看预估请求数。
5. 开始诊断；可随时停止。
6. 根据结构化结果卡片中的建议**手动**在 CC Switch 中修改配置。

## 测试模式

- **快速验证**：只测当前配置。
- **智能诊断（默认）**：失败后尝试 `/v1` 归一、协议/模型候选，上限 12 次/配置，同 Host 上限 30 次。
- **深度兼容性**：增加流式 SSE、Tool Calling、稳定性复测。

## CC Switch 兼容

详见 [`docs/compatibility.md`](docs/compatibility.md) 与 [`compatibility/manifest.json`](compatibility/manifest.json)。

- Baseline：CC Switch **v3.17.0**
- Schema：`user_version = 15`
- 检查日期：2026-07-20

## 构建

```bash
npm ci
npm run security:verify
npm run test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

## 已知限制

- 仅 Windows x64 首发
- 无自动修复 / 无 CLI 实测
- 无代码签名时 SmartScreen 可能拦截
- 未知 schema 时安全停止
- 更新检查仅访问 GitHub API

## 文档

- [实施审计](docs/implementation-audit.md)
- [架构](docs/architecture.md)
- [安全模型](docs/security-model.md)
- [隐私](PRIVACY.md)
- [安全政策](SECURITY.md)
- [兼容性](docs/compatibility.md)

## 免责声明

本工具仅供诊断辅助。上游供应商、系统代理、DNS 与杀毒软件可能产生网络日志。与 CC Switch 官方无隶属关系。

## License

MIT — 见 [LICENSE](LICENSE)
