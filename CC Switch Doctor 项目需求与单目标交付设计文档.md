# CC Switch Doctor 项目需求与单目标交付设计文档

> 文档版本：1.0  
> 目标首发版本：v0.1.0  
> 目标平台：Windows 10/11 x64  
> 推荐仓库名：`cc-switch-doctor`  
> 产品名：**CC Switch Doctor**  
> 可执行文件名：`CC-Switch-Doctor.exe`

---

## 0. 文档目的

本文件是可以直接交给 Codex、Claude Code 或其他具备终端、Git 和 GitHub 权限的 AI 编程工具执行的完整项目规格。

目标不是只生成代码或原型，而是在一次 Goal 任务中完成以下完整交付：

1. 初始化并建设 GitHub 仓库；
2. 完成可运行的 Windows Tauri 桌面应用；
3. 实现 CC Switch 配置只读扫描、选择性测试和智能诊断；
4. 完成自动化测试、安全检查和构建；
5. 配置 GitHub Actions；
6. 创建版本标签；
7. 创建 GitHub Release；
8. 上传 Windows 便携版 EXE、安装版 EXE 和 SHA-256 校验文件；
9. 验证 Release 资产可以下载，应用可以启动并完成核心流程；
10. 输出完整 README、架构、安全、隐私和兼容性文档。

本文中的“必须”“禁止”“不得”均为不可降级约束。AI 编程工具不得为了快速完成而绕过安全边界。

---

## 1. 项目背景

CC Switch 已经能够管理 Claude Code、Claude Desktop、Codex、Gemini CLI、OpenCode、OpenClaw、Hermes 等应用的供应商配置，但其内置“连通性检查”主要回答 Base URL 是否能够收到 HTTP 响应，并不验证 Key、模型、请求格式和流式调用是否真正可用。

用户当前需要一个独立、轻量、安全的诊断工具：直接读取 CC Switch 当前配置，选择部分供应商进行纯 HTTP API 测试，并在原配置失败时自动尝试合理的 URL、协议和认证组合，判断问题究竟来自网络、URL、Key、模型名、API 格式、额度或 CC Switch 路由配置。

该工具不是供应商管理器，不是代理网关，不是 CLI 启动器，也不是 CC Switch 的替代品。

---

## 2. 已验证的 CC Switch 基线

实现前必须重新检查 `farion1231/cc-switch` 官方仓库的最新 Release、`main` 分支和数据库结构，不得仅依赖本文快照。

截至本文形成时，官方仓库显示 CC Switch v3.17.0 为最新 Release；项目使用 MIT License。实现时必须把实际检查到的 CC Switch Release、提交 SHA、数据库 schema 指纹和检查日期写入本项目的兼容性清单。

当前可依赖的核心事实：

1. CC Switch 默认数据目录为 `~/.cc-switch/`；
2. 单一事实源为 `cc-switch.db` SQLite 数据库；
3. `providers` 表保存供应商配置；
4. `provider_endpoints` 保存候选端点；
5. `settings_config` 是不同 app 类型的结构化配置；
6. Codex 官方登录态主要位于 `~/.codex/auth.json`；
7. Claude Code 配置主要位于 `~/.claude/`；
8. CC Switch 允许自定义数据目录；
9. 官方/OAuth/托管账户供应商需要和普通第三方 API Key 供应商严格区分；
10. CC Switch 的数据库和协议结构会持续演进，因此必须通过适配层和 schema 指纹处理变化。

推荐实现时重点审查官方仓库中的以下文件：

```text
src-tauri/src/database/schema.rs
src-tauri/src/database/migration.rs
src-tauri/src/provider.rs
src-tauri/src/config.rs
src-tauri/src/app_store.rs
src-tauri/src/proxy/providers/
src-tauri/src/proxy/model_mapper.rs
src-tauri/src/services/stream_check.rs
docs/user-manual/zh/5-faq/5.1-config-files.md
docs/release-notes/
```

不得复制不必要的大段 CC Switch 源码。确需复用 MIT 代码时，必须保留来源、版权和许可证信息，并写入 `THIRD_PARTY_NOTICES.md`。优先采用基于公开结构的独立实现。

---

## 3. 产品定位

### 3.1 一句话定位

**CC Switch Doctor 是一个只读、无状态、纯 HTTP 的 CC Switch 第三方供应商诊断工具。它自动扫描 CC Switch 当前数据库，让用户选择配置，并通过多 URL、多协议、多认证格式的低成本真实模型请求定位配置问题，全程不启动任何 AI CLI、不切换供应商、不修改任何登录态或配置。**

### 3.2 核心用户价值

用户打开工具后，不再需要：

- 从 CC Switch 手动导出 SQL；
- 到多个测试网站复制 Base URL 和 Key；
- 手工尝试是否需要 `/v1`；
- 手工判断供应商支持 Chat Completions、Responses 还是 Anthropic Messages；
- 为测试而切换当前供应商；
- 冒险触碰 Codex Plus、Claude 订阅或 OAuth 登录缓存。

### 3.3 非目标

v0.1.0 明确不做：

- 不编辑或保存 CC Switch 配置；
- 不自动修复配置；
- 不切换 CC Switch 当前供应商；
- 不启动、停止或接管 CC Switch 本地路由；
- 不调用 Codex、Codex CLI、Claude、Claude Code、OpenCode、Gemini CLI 等程序；
- 不测试官方 Plus、Max、Pro、OAuth、Copilot 等登录态；
- 不做常驻后台监控；
- 不做 API 聚合或故障转移；
- 不托管 Key；
- 不做用户账户系统；
- 不做云同步；
- 不保存历史记录；
- 不提供任意 URL/Key 手工管理中心；
- 不在 v0.1.0 实现自动修改 CC Switch 的“一键修复”。

---

## 4. 不可违反的安全边界

### 4.1 纯 HTTP 测试

所有测试由 CC Switch Doctor 自身的 Rust HTTP 客户端完成。

禁止通过任何方式启动或调用：

```text
codex.exe
codex
claude.exe
claude
claude-code
opencode
gemini
cc-switch.exe
任何 PowerShell、cmd、bash 或 shell 子进程
```

项目不得引入或使用：

```text
std::process::Command
tokio::process
tauri-plugin-shell
ShellExecute
CreateProcess
Command Prompt / PowerShell 调用封装
```

CI 必须包含源码安全扫描，发现上述进程调用能力时直接失败。

### 4.2 禁止读取或修改登录目录

程序不得读取、监控、备份、修改或哈希以下路径：

```text
%USERPROFILE%\.codex\auth.json
%USERPROFILE%\.codex\config.toml
%USERPROFILE%\.codex\
%USERPROFILE%\.claude\settings.json
%USERPROFILE%\.claude.json
%USERPROFILE%\.claude\
%USERPROFILE%\.config\opencode\
%USERPROFILE%\.gemini\
```

唯一允许主动读取的业务数据源是 CC Switch 数据目录中的 SQLite 数据库和用于定位该数据库的 CC Switch 路径元数据。

### 4.3 CC Switch 数据库只读

SQLite 必须使用只读连接：

```text
mode=ro
PRAGMA query_only=ON
仅执行 SELECT / PRAGMA 元数据查询
```

禁止：

```text
INSERT
UPDATE
DELETE
REPLACE
CREATE
DROP
ALTER
VACUUM
ATTACH 写入库
```

不建议使用 `immutable=1`，因为 CC Switch 运行时可能更新数据库。应使用普通只读连接、合理的 `busy_timeout` 和短生命周期查询，以兼容 WAL。

### 4.4 Key 不得进入前端

数据库读取、凭据解析和 HTTP 请求必须全部在 Rust 后端完成。

前端只能收到：

- 供应商 opaque ID；
- 应用类型；
- 供应商名称；
- 脱敏 Key，例如 `sk-abcd…wxyz`；
- 已清理的 Base URL；
- 模型名；
- 协议和诊断状态；
- 已脱敏的错误信息。

不得把完整 Key、完整 `settings_config` 或包含 Key 的请求头通过 Tauri IPC 发送给 WebView。

### 4.5 无持久化

应用不得创建自己的数据库、配置文件、历史文件、日志文件、缓存文件或浏览器存储。

禁止使用：

```text
localStorage
IndexedDB
持久化 Zustand/Redux 插件
Tauri Store
SQLite 写库
文件日志
自动崩溃上传
遥测 SDK
分析 SDK
```

可以使用：

- React 进程内状态；
- Rust 进程内状态；
- `sessionStorage` 也不建议使用，v0.1.0 直接不用；
- 显式“复制到剪贴板”，但必须由用户点击，且 UI 提醒剪贴板属于系统外部状态。

应用关闭时必须取消未完成请求，并尽可能清理内存中的敏感字符串。Rust 凭据建议使用 `secrecy` 和 `zeroize`。

说明：应用自身不持久化并不意味着上游供应商、系统代理、DNS、杀毒软件或 Windows 操作系统不会留下网络或崩溃记录。隐私文档必须明确此边界。

### 4.6 同源保护

所有自动变体只能在原始 Base URL 的同一主机内进行。

允许：

```text
https://api.example.com
https://api.example.com/v1
https://api.example.com/v1/responses
https://api.example.com/v1/chat/completions
```

禁止：

```text
把 api.example.com 的 Key 发送到 api.openai.com
把 api.example.com 的 Key 发送到 api.anthropic.com
根据品牌名猜测其他域名
跨主机跟随 301/302 并携带 Authorization
```

HTTP 客户端应关闭自动跨域重定向，或手动只允许 scheme、host、port 完全符合安全规则的重定向。任何跨主机重定向必须停止并报告。

---

## 5. 目标用户与使用场景

### 5.1 主要用户

- 在 CC Switch 中配置多套第三方 API 的开发者；
- 同时使用 Codex、Claude Code、OpenCode 等工具的人；
- 需要保护官方订阅登录态的人；
- 经常遇到中转站 Base URL、`/v1`、模型名和协议配置问题的人；
- 希望快速筛选失效 Key、无额度 Key 或不兼容模型的人。

### 5.2 核心用户故事

1. 我打开工具即可看到 CC Switch 当前所有第三方供应商，无需导出。
2. 我可以按应用、供应商类型、当前状态筛选并勾选需要测试的配置。
3. 我可以先测试当前配置，失败后让工具自动尝试合理变体。
4. 我可以看懂失败原因和建议修改项，而不是只看到红灯。
5. 我能确信工具不会启动 Codex/Claude/OpenCode，也不会读取官方登录文件。
6. CC Switch 更新后，工具能提示当前版本是否已经验证兼容。
7. 工具关闭后，不保留 Key、选择、结果或历史。

---

## 6. 产品形态与技术选型

### 6.1 产品形态

Windows Tauri 2 桌面应用，提供：

- 便携版：ZIP 内含 `CC-Switch-Doctor.exe`；
- 安装版：NSIS `CC-Switch-Doctor-vX.Y.Z-Windows-x64-setup.exe`；
- 默认按当前用户安装，不要求管理员权限；
- 不注册后台服务；
- 不设置开机启动；
- 不创建托盘常驻；
- 不写注册表业务数据。

Tauri 官方支持在 Windows 上通过 `tauri build` 生成 NSIS setup EXE；发布流水线使用 Windows GitHub Actions runner。

### 6.2 推荐技术栈

前端：

```text
React
TypeScript
Vite
Tailwind CSS 或结构化原生 CSS
Lucide Icons
Vitest
Testing Library
```

后端：

```text
Rust stable
Tauri 2
rusqlite（bundled SQLite）
reqwest（Windows native TLS）
serde / serde_json
tokio
tokio-util CancellationToken
url
secrecy
zeroize
thiserror
sha2（只用于 schema/文件校验，不哈希 Key）
```

测试：

```text
cargo test
Vitest
httpmock 或 wiremock-rs
临时 SQLite fixtures
属性测试：proptest（推荐）
```

包管理：

```text
npm + package-lock.json
```

采用 npm 是为了减少一次性构建环境中的额外依赖。不得同时保留多个 lockfile。

### 6.3 不引入的能力

Tauri capabilities 中不得包含：

- shell 执行；
-任意文件系统写入；
- 任意目录读取；
- updater 自动安装（v0.1.0 只提醒）；
- 全局快捷键；
- 自启动；
- 深链协议注册。

---

## 7. 总体架构

```text
┌───────────────────────────────────────────────┐
│                 React UI                      │
│  配置选择 / 测试模式 / 实时进度 / 诊断结果   │
└──────────────────────┬────────────────────────┘
                       │ 仅 opaque ID 和脱敏数据
┌──────────────────────▼────────────────────────┐
│              Tauri Command Boundary           │
└──────────────────────┬────────────────────────┘
                       │
┌──────────────────────▼────────────────────────┐
│                Rust Application Core          │
│                                               │
│  ┌─────────────────────────────────────────┐  │
│  │ CC Switch Compatibility Adapter         │  │
│  │ 路径发现 / schema 指纹 / Provider 解析  │  │
│  └─────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────┐  │
│  │ Security Guard                          │  │
│  │ OAuth 阻断 / 同源限制 / 脱敏 / 无写入   │  │
│  └─────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────┐  │
│  │ Diagnostic Planner                      │  │
│  │ URL 候选 / 协议候选 / 模型候选 / 限额   │  │
│  └─────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────┐  │
│  │ Protocol Adapters                       │  │
│  │ OpenAI Chat / Responses / Anthropic /   │  │
│  │ Gemini Native                           │  │
│  └─────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────┐  │
│  │ HTTP Executor                           │  │
│  │ TLS / timeout / SSE / cancel / metrics  │  │
│  └─────────────────────────────────────────┘  │
└───────────────────────────────────────────────┘
```

### 7.1 模块边界

#### `ccs_adapter`

只负责理解 CC Switch，不负责发请求：

- 数据目录发现；
- 数据库只读连接；
- schema 检测；
- 兼容性状态；
- Provider 配置解析；
- 托管账户识别；
- 生成统一 `NormalizedProvider`。

#### `diagnostic_planner`

只负责生成有优先级且受限的测试计划：

- 当前配置优先；
- URL 标准化；
- 协议候选；
- 认证候选；
- 模型候选；
- 尝试次数、成本和停止条件。

#### `protocols`

每个协议独立实现：

```text
OpenAiChatAdapter
OpenAiResponsesAdapter
AnthropicMessagesAdapter
GeminiNativeAdapter
```

未来新增协议不应修改 UI 和数据库读取核心，只需实现统一 trait 并登记规则。

#### `http_executor`

只负责安全发送请求：

- 连接和 TLS；
- 超时；
- 同源重定向；
- SSE 读取；
- 首字时间；
- 响应大小限制；
- 取消；
- 敏感信息清理。

#### `result_classifier`

将原始结果转为用户可读诊断：

- 网络不可达；
- TLS 错误；
- Key 无效；
- 权限不足；
- 额度不足或限流；
- 模型不存在；
- API 格式错误；
- URL 路径错误；
- 需要 CC Switch 本地路由；
- 流式不兼容；
- Tool Calling 不兼容；
- 未知错误。

---

## 8. 仓库建议结构

```text
cc-switch-doctor/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   ├── release.yml
│   │   └── upstream-watch.yml
│   ├── ISSUE_TEMPLATE/
│   └── dependabot.yml
├── compatibility/
│   ├── manifest.json
│   ├── schemas/
│   │   ├── cc-switch-v3.16.json
│   │   └── cc-switch-v3.17.json
│   └── fixtures/
│       ├── README.md
│       └── sanitized-*.sql
├── docs/
│   ├── architecture.md
│   ├── security-model.md
│   ├── privacy.md
│   ├── compatibility.md
│   ├── testing-strategy.md
│   └── release-process.md
├── scripts/
│   ├── verify-no-process-spawn.mjs
│   ├── verify-no-protected-paths.mjs
│   ├── verify-version-sync.mjs
│   ├── package-portable.ps1
│   └── generate-checksums.ps1
├── src/
│   ├── app/
│   ├── components/
│   ├── features/
│   ├── i18n/
│   ├── lib/
│   ├── types/
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/
│   ├── src/
│   │   ├── ccs_adapter/
│   │   ├── diagnostics/
│   │   ├── protocols/
│   │   ├── security/
│   │   ├── updates/
│   │   ├── commands.rs
│   │   ├── error.rs
│   │   ├── state.rs
│   │   └── lib.rs
│   ├── Cargo.toml
│   ├── build.rs
│   └── tauri.conf.json
├── tests/
├── AGENTS.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE
├── PRIVACY.md
├── README.md
├── SECURITY.md
├── THIRD_PARTY_NOTICES.md
├── package.json
├── package-lock.json
├── tsconfig.json
└── vite.config.ts
```

---

## 9. CC Switch 数据发现与实时扫描

### 9.1 查找顺序

启动时按以下顺序查找数据库：

1. 读取 CC Switch Tauri Store 中的自定义数据目录覆盖；
2. `%USERPROFILE%\.cc-switch\cc-switch.db`；
3. Windows 历史兼容路径：`HOME\.cc-switch\cc-switch.db`，仅在默认路径不存在时使用；
4. 常见便携目录仅做安全的只读探测；
5. 仍找不到时，显示“选择 cc-switch.db”按钮。

手工选择只对当前进程有效，不保存路径。

### 9.2 实时扫描定义

“实时扫描”指：

- 每次应用启动重新读取数据库；
- 用户点击刷新时重新读取；
- 应用打开期间监听 `cc-switch.db`、`-wal` 和 `-shm` 的变动；
- 发现变化后只提示“CC Switch 配置已变化，点击刷新”；
- 不自动打断正在进行的测试；
- 不保存上一次扫描快照。

### 9.3 并发读取

- 查询必须短连接或短事务；
- 设置合理 `busy_timeout`；
- 遇到数据库繁忙时重试 1～2 次；
- 不复制数据库到磁盘；
- 可以在内存中建立规范化快照；
- 刷新时替换内存快照，并 zeroize 旧凭据。

### 9.4 Schema 检测

不得只依赖版本号。

每次扫描计算 schema 指纹：

```text
PRAGMA user_version
sqlite_master 中的表名
providers 表字段
provider_endpoints 表字段
settings 表字段
关键索引/字段存在性
```

指纹只基于结构，不基于配置内容，也不包含 Key。

兼容状态：

```text
Verified       已有测试 fixture 验证
Compatible     字段满足已知解析器，但版本未正式验证
Unknown        结构变化，禁止读取敏感字段并要求更新 Doctor
Unsupported    旧版本或缺失关键结构
```

Unknown 状态下不得“猜字段”并发送请求。只能显示数据库路径、检测到的表结构摘要和更新提示。

---

## 10. 供应商规范化模型

Rust 内部建议结构：

```rust
struct NormalizedProvider {
    opaque_id: String,
    source_id: String,
    app_type: AppType,
    display_name: String,
    category: Option<String>,
    auth_kind: AuthKind,
    provider_kind: ProviderKind,
    base_url: Url,
    api_key: SecretString,
    configured_protocol: Option<ProtocolKind>,
    configured_model: Option<String>,
    model_candidates: Vec<String>,
    endpoint_candidates: Vec<Url>,
    custom_user_agent: Option<String>,
    needs_local_routing: Option<bool>,
    metadata: NormalizedMetadata,
}
```

### 10.1 应用类型

至少识别：

```text
Claude Code
Claude Desktop
Codex
Gemini CLI
OpenCode
OpenClaw
Hermes
Unknown
```

### 10.2 认证类型

```text
ApiKey
BearerToken
AnthropicKey
GeminiKey
AzureApiKey
ManagedOAuth
GitHubCopilot
CodexOAuth
OfficialSubscription
Unknown
```

以下类型默认锁定，不允许勾选：

- `codex_oauth`；
- GitHub Copilot；
- `chatgpt.com/backend-api/codex`；
- 官方 OpenAI/Claude/Gemini 登录；
- 需要刷新 Token 的托管账户；
- 数据库中没有静态第三方 API Key 的配置；
- 无法确认是否属于登录凭据的配置。

界面显示“安全跳过：托管登录/OAuth”。不得提供强制绕过开关。

### 10.3 URL 脱敏

显示 URL 时必须：

- 移除 userinfo；
- 对 query 参数中的 `key`、`token`、`api_key`、`access_token` 等值脱敏；
- fragment 不参与请求；
- 不在日志或错误中显示完整含密 URL。

---

## 11. 用户界面需求

### 11.1 首页结构

```text
┌─────────────────────────────────────────────────────┐
│ CC Switch Doctor     DB: 已连接      兼容: 已验证   │
│ CC Switch: vX.Y.Z    Doctor: vA.B.C  [检查更新]     │
├─────────────────────────────────────────────────────┤
│ [全部] [Claude] [Codex] [Gemini] [OpenCode] ...    │
│ 搜索供应商...   状态筛选...          [刷新配置]     │
├─────────────────────────────────────────────────────┤
│ □ 应用   供应商    地址       模型     安全状态      │
│ ☑ Codex  MiniMax   api...     model-x  可测试       │
│ ⛔ Codex  Official  —          —        官方登录跳过 │
│ ☑ Claude GLM       api...     model-y  可测试       │
├─────────────────────────────────────────────────────┤
│ 模式：○快速验证  ●智能诊断  ○深度兼容性            │
│ 并发：1   单配置最大尝试：12   预估请求：N          │
│ [开始测试] [取消]                                   │
├─────────────────────────────────────────────────────┤
│ 实时结果 / 尝试链 / 诊断建议                        │
└─────────────────────────────────────────────────────┘
```

### 11.2 配置列表

功能：

- 按 app_type 分组；
- 全选当前筛选；
- 取消全选；
- 只选当前供应商；
- 搜索名称、模型、主机；
- 默认不自动勾选；
- 官方/OAuth 条目灰显并带原因；
- Key 只显示脱敏值；
- 显示当前协议和是否需要本地路由；
- 数据变化时显示非阻塞刷新提示。

### 11.3 测试模式

#### 快速验证

只按 CC Switch 当前配置进行：

1. 可选轻量网络探测；
2. 最小真实模型生成；
3. 默认非流式；
4. 成功立即结束；
5. 失败只分类，不尝试修复变体。

#### 智能诊断（默认）

1. 先测当前配置；
2. 失败时按评分生成受限候选；
3. 尝试 URL 修正；
4. 尝试相邻协议；
5. 尝试模型候选；
6. 必要时测试流式；
7. 找到成功组合后停止高成本尝试；
8. 输出“当前配置”和“成功组合”的差异。

#### 深度兼容性

在智能诊断基础上增加：

- 非流式；
- 流式 SSE；
- Tool Calling；
- 连续两次最小请求稳定性；
- TTFT 和总延迟；
- 响应格式完整性；
- 更严格的终止事件检查。

深度模式仍不得启动任何 CLI。

### 11.4 结果详情

每个供应商显示：

- 总体状态；
- 原配置测试结果；
- 每一次尝试的脱敏 URL、协议、模型、状态码、耗时；
- 失败分类；
- 成功候选；
- 建议在 CC Switch 中修改的字段；
- 是否需要本地路由；
- 结果可信度；
- “复制诊断摘要”按钮。

不得显示：

- 原始 Key；
- Authorization Header；
- 完整请求 Body 中的敏感字段；
- 原始数据库 JSON；
- 可能含 Key 的上游完整错误正文。

---

## 12. 底层测试模式

### 12.1 原则

模型可用性不能由 Base URL 返回 HTTP 状态码来证明。核心判断必须是：

> 使用数据库中的静态第三方 API Key，直接向该供应商发送一个最小、低成本、可验证的模型生成请求，并解析出预期内容或结构。

不经过 Codex、Claude Code、OpenCode、CC Switch Local Routing 或任何 CLI。

### 12.2 统一最小提示词

默认提示词：

```text
只输出字符串 CCS_DOCTOR_OK，不要输出其他内容。
```

英文兼容备用：

```text
Reply with exactly CCS_DOCTOR_OK and nothing else.
```

输出 token 上限：

```text
16
```

成功条件：

- HTTP 成功；
- 响应结构符合协议；
- 提取到非空文本；
- 文本包含 `CCS_DOCTOR_OK`；
- 无嵌套 error；
- 流式模式收到有效增量和合理终止。

部分模型不严格遵循“只输出”时，可以将“有有效文本但缺少标记”标记为 `Partial`，不得直接判为完全失败。

### 12.3 OpenAI Chat Completions

候选端点：

```text
/chat/completions
/v1/chat/completions
```

请求核心：

```json
{
  "model": "<model>",
  "messages": [
    {"role": "user", "content": "Reply with exactly CCS_DOCTOR_OK and nothing else."}
  ],
  "max_tokens": 16,
  "stream": false
}
```

认证优先：

```text
Authorization: Bearer <key>
```

解析：

```text
choices[0].message.content
```

流式解析：

```text
choices[0].delta.content
reasoning_content 仅作为辅助，不作为最终答案
[DONE] 或等价终止
```

### 12.4 OpenAI Responses

候选端点：

```text
/responses
/v1/responses
```

请求核心：

```json
{
  "model": "<model>",
  "input": "Reply with exactly CCS_DOCTOR_OK and nothing else.",
  "max_output_tokens": 16,
  "stream": false
}
```

解析优先级：

```text
output_text
output[].content[].text
```

流式需识别 Responses 事件，而不是按 Chat Completions delta 强行解析。

### 12.5 Anthropic Messages

候选端点：

```text
/v1/messages
/messages
```

请求核心：

```json
{
  "model": "<model>",
  "max_tokens": 16,
  "messages": [
    {"role": "user", "content": "Reply with exactly CCS_DOCTOR_OK and nothing else."}
  ],
  "stream": false
}
```

认证候选：

```text
x-api-key: <key>
anthropic-version: 2023-06-01
```

仅在配置明确使用 Bearer/中转格式时尝试：

```text
Authorization: Bearer <key>
```

解析：

```text
content[].text
```

### 12.6 Gemini Native

候选端点按 Gemini 原生格式生成：

```text
/v1beta/models/<model>:generateContent
/v1/models/<model>:generateContent
```

请求核心：

```json
{
  "contents": [
    {
      "role": "user",
      "parts": [{"text": "Reply with exactly CCS_DOCTOR_OK and nothing else."}]
    }
  ],
  "generationConfig": {
    "maxOutputTokens": 16
  }
}
```

认证：

```text
x-goog-api-key: <key>
```

仅在配置明确时使用 query `?key=`，显示和日志必须脱敏。

### 12.7 Tool Calling 测试

只在深度模式执行。

定义一个无副作用工具：

```text
ccs_doctor_echo(value: string)
```

提示模型调用工具并传入 `"ok"`。验证：

- 返回结构化 tool call；
- 工具名正确；
- 参数可解析；
- 参数值合理；
- 不实际执行任何外部工具；
- 不继续第二轮模型请求，除非未来有明确需求。

### 12.8 `/models` 测试

`GET /models` 只作为模型发现和鉴权辅助，不作为最终可用性证明。

用途：

- 当前模型 404 时查找同名或映射模型；
- 判断 Key 是否能访问模型列表；
- 生成受限模型候选。

不得自动测试 `/models` 返回的全部模型。默认最多选择：

1. 当前配置模型；
2. 配置中的映射目标；
3. 精确同名模型；
4. 一个用户在本次会话中临时选择的模型。

### 12.9 额度判断

统一额度 API 不存在，因此结果必须分级：

```text
明确余额不足
明确请求限流
可能余额/限流
余额未知但推理成功
余额查询不受支持
```

规则：

- 402 或响应正文明确 `insufficient_quota`：明确余额不足；
- 429 + 明确 quota 文本：明确余额不足或配额耗尽；
- 429 + rate limit 文本：限流；
- 429 无明确正文：可能额度/限流；
- 推理成功：当前至少仍可调用，但不能证明剩余余额；
- 已知供应商余额端点可作为可选适配器，但不得执行 CC Switch 数据库中保存的任意脚本；
- v0.1.0 可以先实现“根据推理结果判断”，余额专用适配器作为模块化扩展。

---

## 13. 智能诊断候选生成

### 13.1 禁止组合爆炸

不得对所有 URL × 所有协议 × 所有认证 × 所有模型进行无限暴力枚举。

默认限制：

```text
并发数：1
单供应商最大尝试：12
单次超时：15 秒
深度模式单次超时：30 秒
最大响应 Body：2 MB
最大错误 Body：64 KB
最大重试：网络/超时类 1 次
输出 token：16
```

UI 必须在开始前显示预估请求数。

### 13.2 URL 变体

从原始 Base URL 生成去重、有序候选：

1. 原始 URL；
2. 去除尾部 `/`；
3. 添加 `/v1`；
4. 移除尾部 `/v1`；
5. 把重复 `/v1/v1` 归一；
6. 从误填的 `/chat/completions`、`/responses`、`/messages` 回退到 Base；
7. 再附加当前协议正确 endpoint；
8. 使用 `provider_endpoints` 中同一供应商已有候选 URL；
9. 始终限制同源。

### 13.3 协议优先级

基于 app 类型、CC Switch 当前字段和 meta 评分。

示例：

```text
Codex 当前 Responses
1. Responses 原配置
2. Responses + URL 修正
3. Chat Completions + URL 修正
4. Chat 成功时诊断为“需要本地路由/协议转换”

Claude 当前 Anthropic
1. Anthropic 原配置
2. Anthropic + URL 修正
3. OpenAI Chat（仅中转配置有迹象时）

OpenCode
1. 按 options/API 类型
2. OpenAI Chat
3. Anthropic
4. Responses

Gemini
1. Gemini Native
2. OpenAI 兼容（仅配置明确时）
```

### 13.4 认证候选

只尝试与协议和数据库字段相符的认证方案，不做无依据猜测。

认证变体失败不能把 Key 发送到其他主机。

### 13.5 停止条件

- 当前配置完整成功：立即停止智能修复尝试；
- 找到一个高可信成功组合：停止同级更低分候选；
- 明确无效 Key 且 endpoint 可信：停止高成本测试；
- 明确额度不足：停止重复生成；
- TLS/DNS/连接失败：只尝试同源 URL 归一，不切换大量协议；
- 用户点击取消：立即取消所有请求；
- 供应商连续触发 429：停止该供应商剩余测试。

---

## 14. 诊断状态与建议

至少实现以下状态：

```text
CURRENT_CONFIG_OK
CORRECTED_BASE_PATH_OK
PROTOCOL_FALLBACK_OK
AUTH_VARIANT_OK
MODEL_VARIANT_OK
LOCAL_ROUTING_REQUIRED
STREAMING_UNSUPPORTED
TOOL_CALLING_UNSUPPORTED
KEY_INVALID
PERMISSION_DENIED
QUOTA_EXHAUSTED
RATE_LIMITED
MODEL_NOT_FOUND
ENDPOINT_NOT_FOUND
NETWORK_UNREACHABLE
TLS_ERROR
TIMEOUT
CROSS_ORIGIN_REDIRECT_BLOCKED
MANAGED_AUTH_SKIPPED
UNKNOWN_SCHEMA
UNSUPPORTED_PROTOCOL
UNKNOWN_ERROR
```

### 14.1 示例诊断

```text
MiniMax / Codex

当前配置：
Base URL: https://api.example.com
协议：Responses
模型：model-x

尝试 1：
POST /responses -> 404

尝试 2：
POST /v1/responses -> 404

尝试 3：
POST /v1/chat/completions -> 200，返回 CCS_DOCTOR_OK

结论：
供应商、Key 和模型可用，但上游只支持 Chat Completions。

CC Switch 建议：
- Base URL 使用 https://api.example.com/v1
- API 格式改为 OpenAI Chat Completions
- Codex 场景开启“需要本地路由映射”并启用 Codex Local Routing

本工具未自动修改任何配置。
```

建议必须描述“证据”和“差异”，不得只给笼统建议。

---

## 15. CC Switch 兼容扩展机制

### 15.1 兼容适配层

所有 CC Switch 字段路径必须集中在 `ccs_adapter`，不得散落在 UI 和协议实现中。

采用：

```text
SchemaDetector
SchemaAdapter trait
ProviderExtractor trait
CompatibilityRegistry
```

示例：

```rust
trait SchemaAdapter {
    fn matches(&self, fingerprint: &SchemaFingerprint) -> bool;
    fn list_providers(&self, db: &Connection) -> Result<Vec<RawProvider>>;
    fn normalize(&self, raw: RawProvider) -> Result<NormalizedProvider>;
}
```

### 15.2 兼容性清单

`compatibility/manifest.json` 至少包含：

```json
{
  "manifestVersion": 1,
  "doctorVersion": "0.1.0",
  "verifiedAt": "ISO-8601",
  "ccSwitch": {
    "latestObservedRelease": "3.17.0",
    "verifiedReleases": ["3.16.x", "3.17.x"],
    "baselineCommit": "<sha>",
    "schemaFingerprints": [
      {
        "id": "ccs-schema-v12-example",
        "status": "verified"
      }
    ]
  },
  "rulesVersion": "1"
}
```

实现任务开始时必须更新为实际最新信息。

### 15.3 规则包

协议和 URL 规则允许数据驱动，但 v0.1.0 不允许远程下载并执行代码。

规则只能是声明式 JSON，且必须经过严格 schema 验证。未知字段拒绝或忽略必须有明确策略。

远程兼容清单只用于提醒，不得自动替换本地可执行逻辑。

---

## 16. CC Switch 版本变化监听与更新提醒

### 16.1 本地版本检测

采用多信号、best-effort：

1. 数据库 schema 指纹；
2. 数据库中可用的版本/迁移信息；
3. 运行中的 CC Switch 进程路径和 Windows 文件版本，仅读取，不启动；
4. 常见安装路径下可执行文件版本；
5. 无法得到软件版本时仍可依赖 schema 指纹。

不得为了获取版本启动 CC Switch。

### 16.2 应用启动检查

每次启动只在内存中执行一次：

- 查询 CC Switch 官方 GitHub 最新 Release；
- 查询 CC Switch Doctor 官方 GitHub 最新 Release；
- 超时 3～5 秒；
- 失败不阻塞使用；
- 不发送 Key、供应商名称、数据库路径或设备标识；
- 只发送普通 GitHub HTTPS GET；
- 提供“手动检查更新”。

显示状态：

```text
CC Switch 本地版本：3.17.0
CC Switch 官方最新：3.18.0
Doctor 已验证到：3.17.x
状态：上游有新版本，当前兼容性尚未验证

Doctor 当前版本：0.1.0
Doctor 最新版本：0.2.0
状态：建议更新 Doctor
```

### 16.3 数据库变更监听

应用打开时监听数据库文件变更，只做本地提醒：

```text
“检测到 CC Switch 数据或 schema 发生变化。建议刷新；若 schema 未验证，将停止测试。”
```

### 16.4 仓库上游监控工作流

创建 `.github/workflows/upstream-watch.yml`：

- `schedule` 每天一次；
- `workflow_dispatch`；
- 查询 `farion1231/cc-switch` 最新 Release；
- 和 `compatibility/manifest.json` 的 `latestObservedRelease` 比较；
- 发现新版本时创建或更新一个带 `upstream-change` 标签的 Issue；
- Issue 包含版本、发布日期、Release 摘要链接和待检查清单；
- 不自动把新版本标记为 verified；
- 不自动发布 Doctor；
- 避免每天重复创建 Issue。

### 16.5 Doctor 自身更新

v0.1.0 只提醒并打开官方 Release 页面，不做静默自动更新。

原因：

- 无代码签名时自动更新风险较高；
- 无状态设计不应引入额外更新缓存；
- 首版优先保证安全和可审计性。

---

## 17. 网络、代理和证书策略

- 默认使用 Windows 系统信任存储；
- 支持标准 `HTTP_PROXY`、`HTTPS_PROXY`、`NO_PROXY` 环境变量；
- 可在当前会话临时输入代理，但不保存；
- 不修改系统代理；
- 不启动 CC Switch Local Routing；
- 不默认把请求发往 `127.0.0.1:15721`；
- 不允许关闭 TLS 证书验证作为默认行为；
- v0.1.0 不提供“忽略证书错误”开关；
- 明确显示 TLS 错误，并建议用户修复证书或代理环境。

企业自签证书应通过 Windows 信任链解决，不通过应用关闭验证。

---

## 18. 前后端 API 建议

```rust
#[tauri::command]
async fn discover_cc_switch() -> Result<DiscoveryInfo, PublicError>;

#[tauri::command]
async fn scan_providers() -> Result<ProviderScanView, PublicError>;

#[tauri::command]
async fn refresh_providers() -> Result<ProviderScanView, PublicError>;

#[tauri::command]
async fn start_diagnosis(request: StartDiagnosisRequest) -> Result<RunHandle, PublicError>;

#[tauri::command]
async fn cancel_diagnosis(run_id: String) -> Result<(), PublicError>;

#[tauri::command]
async fn check_updates() -> Result<UpdateStatus, PublicError>;
```

诊断过程通过 Tauri channel/event 流式通知：

```text
run_started
provider_started
attempt_started
attempt_finished
provider_finished
run_cancelled
run_finished
```

事件中不得包含完整 Key、Authorization Header 或未脱敏错误正文。

---

## 19. 内存状态与生命周期

Rust `AppState` 只在进程内保存：

```text
当前数据库路径
当前 schema 信息
当前 Provider 内存快照
当前诊断任务
CancellationToken
当前结果
```

要求：

- 窗口关闭时取消任务；
- Provider 快照替换时 zeroize 凭据；
- 不把状态写到前端持久化；
- 不恢复上次选择；
- 不保存更新检查结果；
- 进程重新打开后从零开始。

---

## 20. 错误脱敏

实现统一 `SecretRedactor`：

1. 替换内存中已知完整 Key；
2. 替换 Bearer Token；
3. 替换 URL query 中敏感值；
4. 对常见 `sk-`、`key-`、JWT 等模式做保守脱敏；
5. 限制错误正文长度；
6. 不把原始响应写入日志；
7. Rust panic 不携带 Provider 配置 Debug 输出。

所有包含 Provider 的结构体不得无条件 `derive(Debug)`，或必须对 Secret 字段使用安全 Debug 实现。

---

## 21. 测试策略

### 21.1 Rust 单元测试

必须覆盖：

- 默认和自定义数据库路径发现；
- Windows `HOME` 历史路径回退；
- schema 指纹；
- v3.16/v3.17 fixtures；
- Provider 凭据提取；
- 托管账户识别；
- URL 归一；
- `/v1` 添加、移除、去重；
- 已填 endpoint 回退；
- 同源校验；
- 跨源重定向阻断；
- 协议候选顺序；
- 最大尝试限制；
- 状态码和错误正文分类；
- SSE 解析；
- Responses 事件解析；
- Key 脱敏；
- 取消测试；
- 只读数据库行为。

### 21.2 HTTP 集成测试

用本地 mock server 模拟：

- Chat Completions 成功；
- Responses 成功；
- Anthropic 成功；
- Gemini 成功；
- 流式成功；
- 200 但空正文；
- 200 但嵌套 error；
- 401；
- 403；
- 404；
- 402；
- 429 quota；
- 429 rate limit；
- 500/502；
- 超时；
- TLS 类错误可通过分类单元测试；
- 跨主机 redirect；
- Tool Calling 成功/失败。

### 21.3 前端测试

- 列表筛选；
- 官方配置不可勾选；
- 默认不勾选；
- 测试模式切换；
- 预估请求数；
- 取消按钮；
- 结果详情；
- 更新警告；
- Key 不出现在 DOM；
- 刷新提示；
- Unknown schema 安全阻断。

### 21.4 安全回归测试

CI 运行：

```text
verify-no-process-spawn.mjs
verify-no-protected-paths.mjs
```

扫描 `src-tauri/src`，禁止：

```text
std::process
Command::new
tokio::process
tauri_plugin_shell
.codex
.claude
opencode home
```

允许在测试和文档中出现说明，但生产源码不能出现对受保护路径的访问实现。

额外测试：

- 测试前后比较临时 HOME 中受保护文件 hash 未变化；
- 测试前后比较 CC Switch fixture DB hash 未变化；
- 确保数据库连接是只读；
- 确保前端 IPC payload 不含完整 Key；
- 确保日志捕获中不含 Key。

### 21.5 质量门禁

以下命令必须全部成功：

```bash
npm ci
npm run format:check
npm run lint
npm run typecheck
npm run test
npm run build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run security:verify
npm run tauri build
```

---

## 22. UI 与体验要求

- 默认中文；
- 架构支持 i18n，v0.1.0 可只完整交付 `zh-CN`；
- 浅色/深色跟随系统，不保存选择；
- 使用清晰状态图标但不能只靠颜色；
- 结果表格支持键盘操作；
- 所有按钮有禁用状态和原因；
- 取消操作应快速响应；
- 长任务持续显示进度；
- 不弹出大量阻塞式对话框；
- 不自动开始测试；
- 第一次测试前显示一次当前会话安全说明，不保存“已读”；
- 明确显示“测试会消耗极少量上游 token”；
- 默认并发 1，用户本次会话可改为 2 或 3；
- 不提供高并发选项。

---

## 23. 文档交付要求

### README.md

必须包含：

- 产品截图；
- 产品定位；
- 安全保证；
- 支持范围；
- 安装方法；
- 便携版说明；
- 测试模式；
- 常见诊断；
- CC Switch 兼容状态；
- 构建方法；
- 发版方法；
- 已知限制；
- 免责声明。

### SECURITY.md

包含：

- 如何报告漏洞；
- Key 泄漏风险；
- 同源安全；
- 不接触登录态保证；
- 支持版本；
- 禁止公开粘贴含 Key 日志。

### PRIVACY.md

明确：

- 应用不保存配置和结果；
- Key 只存在于内存并发往原供应商；
- 更新检查只访问 GitHub；
- 上游供应商可能记录请求；
- 系统和代理可能记录网络；
- 显式复制会写入系统剪贴板；
- 不包含遥测。

### docs/compatibility.md

记录：

- 检查的 CC Switch 版本；
- baseline commit；
- schema 指纹；
- 支持的 app_type；
- 支持的协议；
- 未支持内容；
- 新版本验证流程。

### AGENTS.md

给后续 AI 开发使用，第一条必须是：

```text
本项目绝不启动任何 AI CLI、绝不读取 Codex/Claude/OpenCode 登录目录、绝不写入 CC Switch 数据库、绝不持久化 Key 或诊断结果。
```

---

## 24. GitHub Actions

### 24.1 `ci.yml`

触发：

```text
pull_request
push 到 main
workflow_dispatch
```

任务：

1. 前端 format/lint/typecheck/test/build；
2. Rust fmt/clippy/test；
3. 安全扫描；
4. Windows Tauri build smoke test；
5. 上传 CI 构建产物供验证，不创建正式 Release。

最少在 `windows-latest` 运行完整构建。纯 Rust/前端测试可增加 Ubuntu job，但不得以跨平台扩展拖延 Windows 首发。

### 24.2 `release.yml`

触发：

```text
push tag: v*
workflow_dispatch（输入版本）
```

权限：

```yaml
permissions:
  contents: write
```

步骤：

1. checkout；
2. setup Node LTS；
3. setup Rust stable；
4. npm ci；
5. 完整质量门禁；
6. 验证 package.json、Cargo.toml、tauri.conf.json 版本一致；
7. `npm run tauri build`；
8. 生成 NSIS setup EXE；
9. 将 release binary 打包为便携 ZIP；
10. 生成 `SHA256SUMS.txt`；
11. 创建非草稿 GitHub Release；
12. 上传全部资产；
13. Release body 包含安全边界、兼容基线和未签名提示；
14. 最后验证 Release API 中资产存在且大小大于零。

推荐使用官方 `tauri-apps/tauri-action@v0` 或等价可审计脚本。Tauri 官方文档已提供 Windows GitHub Actions 发版方式。

### 24.3 Release 资产

必须生成：

```text
CC-Switch-Doctor-v0.1.0-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.0-Windows-x64-portable.zip
SHA256SUMS.txt
```

便携 ZIP 至少包含：

```text
CC-Switch-Doctor.exe
README.txt
LICENSE
PRIVACY.md
```

可选：

```text
SBOM
MSI
调试符号（单独资产，不默认下载）
```

### 24.4 代码签名

首版必须支持两种情况：

#### 没有证书

- 构建和 Release 必须成功；
- 明确标注“未签名，Windows SmartScreen 可能提示”；
- 不伪造签名；
- 不使用不可信免费证书。

#### 已配置证书 Secrets

可选 Secrets：

```text
WINDOWS_CERTIFICATE
WINDOWS_CERTIFICATE_PASSWORD
```

存在时自动签名；不存在时走 unsigned 路径。不得因为缺少证书导致整个 Goal 失败。

---

## 25. 版本管理

- 使用 SemVer；
- 首版 `0.1.0`；
- 版本必须在 package.json、Cargo.toml、tauri.conf.json 一致；
- `verify-version-sync.mjs` 在 CI 阻止不一致；
- Tag 格式 `v0.1.0`；
- Release 名称 `CC Switch Doctor v0.1.0`；
- CHANGELOG 使用 Keep a Changelog 风格。

---

## 26. 首版验收标准

### 26.1 功能验收

- [ ] 启动自动找到默认 CC Switch DB；
- [ ] 支持自定义数据目录探测；
- [ ] 找不到时可临时选择 DB；
- [ ] 列出当前第三方供应商；
- [ ] 按 app 分组筛选；
- [ ] 允许勾选部分配置；
- [ ] 官方/OAuth 配置无法勾选；
- [ ] 完整 Key 不进入 DOM；
- [ ] 快速模式发送当前配置的最小真实请求；
- [ ] 智能模式自动尝试 `/v1` 添加/移除；
- [ ] 支持 Chat Completions；
- [ ] 支持 Responses；
- [ ] 支持 Anthropic Messages；
- [ ] 支持 Gemini Native；
- [ ] 支持流式测试；
- [ ] 深度模式支持 Tool Calling 探测；
- [ ] 能诊断 Codex“Chat 可用但 Responses 不可用，需要本地路由”；
- [ ] 能区分 401、403、404、429、5xx、超时和 TLS；
- [ ] 能取消测试；
- [ ] 能提示 CC Switch 配置变化；
- [ ] 能检查 CC Switch/Doctor 更新；
- [ ] 关闭重开后没有历史和上次选择。

### 26.2 安全验收

- [ ] 生产依赖没有 shell 插件；
- [ ] 生产源码没有 process spawn；
- [ ] 不读取 `.codex`；
- [ ] 不读取 `.claude`；
- [ ] 不读取 OpenCode home；
- [ ] CC Switch fixture DB hash 测试前后相同；
- [ ] 受保护目录 fixture hash 测试前后相同；
- [ ] Key 不出现在前端事件、DOM、日志和错误；
- [ ] 跨源 redirect 不携带 Key；
- [ ] 未知 schema 时停止测试；
- [ ] 应用没有自己的持久化文件；
- [ ] 没有遥测或自动崩溃上传。

### 26.3 工程验收

- [ ] README、SECURITY、PRIVACY、架构和兼容文档完整；
- [ ] CI 全绿；
- [ ] release workflow 成功；
- [ ] GitHub Release 非草稿；
- [ ] setup EXE 存在；
- [ ] portable ZIP 存在；
- [ ] SHA256SUMS 存在；
- [ ] 资产大小大于零；
- [ ] 便携版在干净 Windows x64 环境启动；
- [ ] 至少一套 mock/fixture 完成端到端核心流程；
- [ ] Git 工作区干净；
- [ ] main 分支已推送；
- [ ] v0.1.0 tag 已推送。

---

## 27. Definition of Done

只有同时满足以下条件才算完成 Goal：

1. 仓库中存在完整源码和文档；
2. 所有质量门禁成功；
3. 安全约束有自动化测试保护；
4. Windows 本地/CI 构建成功；
5. GitHub Actions 在远程仓库运行成功；
6. GitHub Release 已创建；
7. 安装版 EXE、便携版 ZIP 和 SHA256 校验已上传；
8. Release 资产经过 API 或 `gh release view` 验证；
9. 应用核心流程可操作；
10. 没有未提交改动；
11. 最终答复提供仓库、Release、资产名称、测试结果和已知限制；
12. 不得以“代码已写但未发版”结束；
13. 不得以“等待用户后续手动操作”代替可自动完成的步骤。

唯一允许的外部阻塞：

- GitHub 未登录或无仓库写权限；
- GitHub Actions 被组织策略禁用；
- 用户要求签名但没有证书；
- 网络完全无法访问 GitHub/npm/crates.io。

遇到阻塞时必须明确说明已完成部分、阻塞证据和最短修复命令；但在有权限的前提下不得提前停止。

---

## 28. 开发实施顺序

AI 编程工具应按此顺序执行，但不需要每一步等待用户确认：

1. 环境和 GitHub 权限预检；
2. 检查 CC Switch 官方最新 Release 和 `main`；
3. 记录 baseline commit 和 schema；
4. 初始化 Tauri 2 + React + TypeScript；
5. 建立安全约束和 CI deny checks；
6. 实现数据库发现和只读 schema adapter；
7. 实现 Provider 规范化和托管账户阻断；
8. 实现安全 HTTP executor；
9. 实现协议 adapters；
10. 实现智能诊断 planner；
11. 实现结果分类和脱敏；
12. 实现 UI；
13. 实现版本检查和上游监控；
14. 完成 fixtures、单元和集成测试；
15. 完成文档；
16. 本地完整质量门禁；
17. 提交并推送 main；
18. 观察并修复远程 CI；
19. 创建并推送 v0.1.0 tag；
20. 观察并修复 release workflow；
21. 验证 Release 资产；
22. 输出最终交付报告。

不得在测试和 CI 未通过前创建最终 tag。

---

## 29. 建议的空仓库准备方式

为了提高一次 Goal 成功率，建议用户先创建一个空 GitHub 仓库：

```text
仓库名：cc-switch-doctor
默认分支：main
不要预创建 README
不要预创建 LICENSE
不要预创建 .gitignore
```

然后让 AI 工具在已 clone 的仓库目录运行。

执行前确认：

```bash
git remote -v
gh auth status
gh repo view
```

GitHub 仓库设置中需要：

```text
Actions 已启用
Workflow permissions: Read and write permissions
允许 GitHub Actions 创建 Release
```

没有 Windows 代码签名证书也可以完成首版，只是安装时可能出现 SmartScreen 提示。

AI 工具有 `gh repo create` 权限时也可自行创建仓库，但预先创建空仓库更可靠。

---

## 30. 可直接交给 AI 编程工具的单目标 Prompt

下面内容可原样作为 Goal 任务。将尖括号变量替换为实际信息。

```text
你现在负责从零完整建设并发布 GitHub 项目“CC Switch Doctor”。

仓库：<本地仓库路径或 owner/cc-switch-doctor>
目标版本：v0.1.0
目标平台：Windows 10/11 x64

请严格以仓库根目录中的《CC Switch Doctor 项目需求与单目标交付设计文档》为唯一产品规格，一次性完成源码、测试、文档、CI、GitHub Release 和 Windows EXE 资产。不要只交付设计、原型或未发版代码。

最高优先级硬约束：
1. 工具只允许通过 Rust HTTP 客户端测试 API。
2. 绝不启动或调用 Codex、Codex CLI、Claude、Claude Code、OpenCode、Gemini CLI、CC Switch 或任何 shell/子进程。
3. 绝不读取或修改 .codex、.claude、OpenCode、Gemini 等登录和配置目录。
4. CC Switch 数据库必须只读，绝不写入或切换供应商。
5. 完整 Key 只存在 Rust 内存，不得进入前端、日志、文件、localStorage、数据库或遥测。
6. 自动 URL/协议变体只能访问原 Base URL 的同一 host，跨 host redirect 必须阻断且不得携带凭据。
7. 每次启动实时读取 CC Switch，应用不保存配置、选择、结果和历史。
8. 官方订阅、OAuth、Codex OAuth、GitHub Copilot、ChatGPT backend 等托管认证配置必须安全跳过，不提供绕过。

开始编码前：
- 访问 farion1231/cc-switch 官方 GitHub 仓库；
- 检查真正最新的 Release、main 分支、database schema、provider 结构、protocol adapters 和 release notes；
- 记录检查日期、最新版本和 baseline commit；
- 根据最新源码更新 compatibility/manifest.json 和实现，不要盲信文档中的版本快照；
- 只使用官方仓库和官方 Tauri/GitHub 文档作为关键技术依据。

实施要求：
- 使用 Tauri 2 + React + TypeScript + Rust；
- Windows x64 首发；
- 数据库只读扫描；
- UI 可按应用筛选和勾选配置；
- 支持快速验证、智能诊断和深度兼容性模式；
- 支持 OpenAI Chat Completions、OpenAI Responses、Anthropic Messages、Gemini Native；
- 支持非流式、流式和深度模式 Tool Calling 检测；
- 实现 /v1 添加/移除、已填 endpoint 归一、协议切换和模型候选；
- 诊断 Key、权限、模型、额度/限流、URL、API 格式、流式和本地路由需求；
- 实现 CC Switch schema 指纹和未知 schema 安全停止；
- 实现 CC Switch/Doctor 版本检查；
- 实现每日 upstream-watch GitHub Action；
- 不实现自动配置修改和 CLI 实测。

必须先建立自动化安全门禁，防止后续代码引入 process spawn、受保护路径读取、数据库写入和秘密泄漏。

请自主解决实现细节，不要把正常技术选择逐项抛回给用户确认。只有遇到 GitHub 无权限、网络不可用或签名证书缺失但用户明确要求必须签名时，才把它作为外部阻塞。

完成前必须运行并通过：
- 前端 format/lint/typecheck/test/build；
- Rust fmt/clippy/test；
- 安全扫描；
- Tauri Windows production build；
- 远程 GitHub CI；
- Release workflow。

发版要求：
- 提交并推送 main；
- 创建并推送 v0.1.0；
- 创建非草稿 GitHub Release“CC Switch Doctor v0.1.0”；
- 上传：
  1. CC-Switch-Doctor-v0.1.0-Windows-x64-setup.exe
  2. CC-Switch-Doctor-v0.1.0-Windows-x64-portable.zip
  3. SHA256SUMS.txt
- 没有签名证书时发布 unsigned 版本并在 Release 中明确 SmartScreen 提示，不能因此中止发版；
- 验证每个 Release 资产存在且大小大于零；
- 确认 Git 工作区干净。

最终只在全部完成后汇报：
- 仓库地址；
- baseline CC Switch 版本和 commit；
- CI 和 Release workflow 状态；
- Release 地址和资产名称；
- 本地执行的测试；
- 安全验证结果；
- 已知限制；
- 是否签名。

不要以“后续可以”“建议下一步”“代码已生成但需要用户自己发版”作为完成结果。
```

---

## 31. 首版之后的扩展路线

不属于 v0.1.0 Definition of Done，但架构需允许：

1. 新增 CC Switch schema adapter；
2. 新增 Bedrock Converse 等协议；
3. 新增已知供应商余额适配器；
4. 新增签名的声明式兼容规则包；
5. 新增 macOS/Linux 构建；
6. 新增诊断报告显式导出；
7. 新增用户确认后的 CC Switch 修改建议应用；
8. 新增本地代理环境诊断；
9. 新增模型能力矩阵；
10. 新增应用自身安全自动更新。

任何未来“真实 CLI 测试”必须是独立项目或独立、默认不存在的可选组件，不能悄悄进入主程序。

---

## 32. 最终设计决策摘要

```text
产品：Windows Tauri 2 桌面诊断器
数据：实时只读 CC Switch SQLite
凭据：仅 Rust 内存
测试：直接 HTTPS API 请求
CLI：绝不调用
登录态：绝不读取
存储：无
自动尝试：同源 URL + 协议 + 认证 + 模型受限矩阵
结果：证据链 + 配置建议，不自动修改
兼容：schema adapter + 指纹 + manifest
更新：启动检查 + 上游监控 Action，仅提醒
发版：GitHub Actions 自动生成 setup EXE、portable ZIP、SHA256
许可证：MIT
```

本规格的核心不是“尽可能多地测试”，而是：

> 在不触碰任何官方登录态、不改变任何本地配置、不保存任何秘密的前提下，用最少的真实 API 请求，准确判断 CC Switch 第三方供应商配置为什么不能使用，以及应当如何修正。
