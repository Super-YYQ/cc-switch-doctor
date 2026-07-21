# CC Switch Doctor 最终单目标交付 Prompt

> 使用方式：把本文件放在仓库根目录，文件名保持为 `FINAL_GOAL_PROMPT.md`。同时放入：
>
> - `PROJECT_SPEC.md`：项目需求与安全规格 v1.1 审查修订版
> - `UI_UX_ADDENDUM.md`：UI/UX 美化补充规范
> - `UI_WIREFRAME_COMPONENT_SPEC.md`：页面线框与组件实施规范
>
> 然后把“从【开始执行】到【结束执行】”的全部内容，作为一次 Goal 交给具备终端、Git、GitHub 和 Windows 构建权限的 AI 编程工具。

---

## 【开始执行】

你现在是本项目的唯一交付负责人。请在当前仓库中一次性完成 **CC Switch Doctor 的审计、修复、产品化 UI 重构、自动化测试、Windows 构建、GitHub Actions、版本标签、GitHub Release 和 EXE 资产发布**。

仓库远程目标：

```text
Super-YYQ/cc-switch-doctor
```

目标平台：

```text
Windows 10/11 x64
```

当前仓库可能已经存在可运行的 v0.1.0 初版，不要假设是空仓库，也不要无理由推倒重写。先审计现有实现，保留正确的功能和安全设计，再完成必要重构。

必须首先完整阅读并遵守仓库根目录中的以下文件，优先级从高到低：

```text
1. PROJECT_SPEC.md
2. UI_UX_ADDENDUM.md
3. UI_WIREFRAME_COMPONENT_SPEC.md
4. 本 FINAL_GOAL_PROMPT.md
5. AGENTS.md（若存在）
```

发生冲突时：

- 安全边界始终最高优先级；
- `PROJECT_SPEC.md` 负责功能、架构、安全和发版；
- `UI_UX_ADDENDUM.md` 与 `UI_WIREFRAME_COMPONENT_SPEC.md` 覆盖旧文档中的视觉和交互实现；
- 不得为了美化而削弱安全约束；
- 不得为了赶发版而跳过测试或伪造结果。

---

# 一、最终交付目标

本 Goal 结束时，必须同时完成：

1. 当前仓库源码经过审计并达到 `PROJECT_SPEC.md` 的安全和功能要求；
2. 当前粗糙原型 UI 被重构为明显产品化、接近 CC Switch 视觉语言的桌面界面；
3. 数据库扫描、供应商选择、三种测试模式、智能诊断、尝试链、建议和更新检查可用；
4. 工具仍然只通过 Rust HTTP 客户端直接请求供应商 API；
5. 工具绝不启动或触发 Codex、Claude、OpenCode、Gemini CLI、CC Switch 或任何 Shell/子进程；
6. 工具绝不读取或修改任何 AI CLI 登录目录；
7. 完整 Key 不进入 WebView、DOM、日志、文件、数据库、测试快照或遥测；
8. 前端与 Rust 的测试、安全门禁、CI 全部通过；
9. Windows NSIS 安装版和便携版构建成功；
10. GitHub Release 已创建且资产已上传、校验并可下载；
11. 仓库文档、截图和 Release Notes 完整；
12. 最终 Git 工作区干净，远程 CI 和 Release workflow 成功。

不得以“代码完成但需要用户自己发版”“界面以后再优化”“本地成功但远程 CI 未验证”结束。

---

# 二、第一阶段：严格预检

在修改代码前，执行并记录以下预检。

## 2.1 Git 与 GitHub

检查：

```bash
git status --short
git remote -v
git branch --show-current
gh auth status
gh repo view Super-YYQ/cc-switch-doctor
```

要求：

- 确认当前仓库 remote 指向 `Super-YYQ/cc-switch-doctor`；
- 确认有 push、Actions 和 Release 权限；
- 不得删除用户未提交的有效修改；
- 若工作区有修改，先审计来源并纳入本次交付或安全保存，不得直接 `reset --hard`；
- 不得改写已公开历史，不得 force push。

## 2.2 当前项目审计

先运行现有项目并检查：

- 当前技术栈与依赖；
- 当前 UI 截图对应实现；
- 数据库扫描路径；
- Rust IPC 边界；
- Key 是否可能进入前端；
- 是否使用进程启动、shell 插件、系统 URL opener；
- 当前测试与 Actions；
- 当前版本号和已有 tag/release；
- 当前构建产物是否真正可启动。

创建简短审计记录：

```text
docs/implementation-audit.md
```

内容必须包含：

- 可保留部分；
- 必须修复部分；
- 安全风险；
- UI/UX 主要问题；
- 本次采用的重构范围；
- 不重写的理由或必须重写的理由。

## 2.3 CC Switch 上游基线

访问官方仓库：

```text
farion1231/cc-switch
```

通过官方 GitHub API、默认分支源码和 Release 信息检查：

- 最新正式 Release；
- 默认分支 HEAD SHA；
- SQLite schema；
- migration；
- Provider 数据结构；
- app types；
- managed/OAuth provider 判断；
- URL、协议、模型映射和本地路由相关 Adapter；
- 自定义数据目录的真实 Tauri Store 位置与键名；
- 可能影响 Doctor 的近期变更。

把结果写入：

```text
compatibility/manifest.json
docs/compatibility.md
```

不得把旧文档里的固定版本号当作当前事实。

---

# 三、不可违反的安全红线

以下规则必须通过代码结构、单元测试和 CI 静态扫描三重保护。

## 3.1 只允许纯 HTTP 测试

所有供应商测试必须由 Rust HTTP 客户端直接完成。

严禁启动或调用：

```text
codex.exe
codex
claude.exe
claude
claude-code
opencode
gemini
cc-switch.exe
powershell
pwsh
cmd
bash
wsl
任何其他 shell 或子进程
```

生产源码和生产依赖不得使用：

```text
std::process::Command
tokio::process
tauri-plugin-shell
ShellExecute
CreateProcess
系统 URL opener
```

允许 GitHub Actions 和开发脚本在构建环境中执行工具链命令；禁止的是生产应用运行时能力。安全扫描需区分生产源码与 CI/构建脚本，避免误报但不能漏报。

## 3.2 禁止读取 AI CLI 登录与配置目录

生产程序不得读取、监控、备份、修改、哈希或枚举：

```text
%USERPROFILE%\.codex\
%USERPROFILE%\.claude\
%USERPROFILE%\.claude.json
%USERPROFILE%\.config\opencode\
%USERPROFILE%\.gemini\
```

不得为了获取版本号而扫描这些目录，也不得运行相关程序。

## 3.3 CC Switch 数据库只读

必须使用 SQLite 只读连接和 `PRAGMA query_only=ON`，只允许查询。

不得：

- 写数据库；
- 修改当前 Provider；
- 更新健康状态；
- 写日志表；
- 切换供应商；
- 启停或接管 CC Switch Local Routing。

使用人工构造的 synthetic fixture，验证测试前后数据库 SHA-256 完全一致。

## 3.4 Key 只存在 Rust 内存

前端只能获得：

- opaque provider ID；
- app type；
- provider name；
- 脱敏 Key 摘要；
- 已清理 URL；
- model；
- protocol；
- 脱敏诊断结果。

不得通过 IPC 把完整 Key、完整 `settings_config`、Authorization Header、原始请求体或含密错误传给前端。

实现统一 Secret Redactor，至少覆盖：

- Key 原值；
- Bearer Header；
- `x-api-key`；
- query 中的 key/token；
- JSON 中常见密钥字段；
- 上游回显的凭据；
- panic/error chain。

## 3.5 同 Host 限制和重定向保护

自动 URL 与协议候选只能在原始 Base URL 的同一规范化 Host 上测试。

- 禁止跨 Host 猜测；
- 禁止把 Key 发往官方 API 或其他域名；
- 跨 Host redirect 必须中止；
- redirect 时不得转发凭据；
- URL userinfo 必须拒绝；
- fragment 必须移除；
- 敏感 query 必须脱敏。

## 3.6 零业务持久化

不得保存：

- Provider 配置；
- Key；
- 上次选择；
- 测试结果；
- 诊断历史；
- 原始日志；
- localStorage；
- IndexedDB；
- 遥测；
- 自动崩溃上传。

允许 WebView2/Windows 产生不可避免的运行时数据，但不得包含业务秘密。尽量使用会话临时目录，并在正常退出时 best-effort 清理。

---

# 四、功能实现要求

## 4.1 数据库发现和实时刷新

实现：

1. 按当前 CC Switch 源码确认的 Tauri Store 明确位置读取 `app_config_dir_override`；
2. 默认 `%USERPROFILE%\.cc-switch\cc-switch.db`；
3. 仅在默认不存在时尝试历史 `HOME\.cc-switch\cc-switch.db`；
4. 用户可临时选择 DB；
5. 手选路径只存在当前进程内存；
6. 点击刷新时重新打开短生命周期只读连接并扫描；
7. 可检测数据库发生变化并显示非阻塞刷新提示；
8. 未知 schema 时安全停止，不猜测读取 Key。

## 4.2 Provider 列表

支持：

- All、Claude、Claude Desktop、Codex、Gemini、OpenCode、OpenClaw、Hermes、Grok 等实际存在 app 类型；
- 搜索 provider、host、model；
- 全选当前筛选；
- 取消全选；
- 仅看已选择；
- 当前 Provider 标记；
- 官方/OAuth/managed provider 灰显并说明跳过原因；
- 完整 Key 永不显示；
- 默认不要自动勾选全部；
- 同一凭据和目标组合进行本次会话内去重。

## 4.3 三种诊断模式

### 快速验证

- 只按当前配置发送一个最小真实模型请求；
- 非流式优先；
- 成功立即结束；
- 失败只分类，不展开大量变体。

### 智能诊断（默认）

- 先测当前配置；
- 对失败进行分类；
- 在请求预算内尝试 URL 归一化、`/v1` 添加/移除、已填写 endpoint 移除、邻近协议、认证格式和模型候选；
- 找到可信成功组合后停止无意义请求；
- 输出当前配置与成功组合差异；
- 明确标注“上游 API 已验证”或“仅推断需要 CC Switch Local Routing”。

### 深度兼容性

在智能诊断基础上增加：

- 非流式；
- 流式 SSE；
- Tool Calling；
- 两次最小稳定性请求；
- TTFT、总延迟；
- 协议完整性和终止事件检查。

深度模式仍然禁止启动任何 CLI。

## 4.4 协议支持

至少支持：

- OpenAI Chat Completions；
- OpenAI Responses；
- Anthropic Messages；
- Gemini Native。

Tool Calling 必须按协议分别实现，不得错误共用一种 JSON：

- Chat Completions `tool_calls`；
- Responses function-call item；
- Anthropic `tool_use`；
- Gemini `functionDeclarations` / `functionCall`。

现代 OpenAI 兼容性：

- 优先 `max_completion_tokens`；
- 只有在上游明确表示字段不支持时回退 `max_tokens`；
- `/models` 与 `/v1/models` 都可作为受控候选；
- 参数回退计入请求预算。

## 4.5 请求预算

默认：

```text
并发数：1
单 Provider 最大请求：12
单 Host 本次会话最大请求：30
同一 Host 连续两次 429：停止该 Host 后续高成本尝试
明确 401/403/402：停止重复认证变体
```

UI 必须显示预计请求数和已消耗请求数，避免无意识消耗共享额度。

---

# 五、UI/UX 产品化重构

当前原型存在顶部拥挤、列表字段断裂、按钮主次不清、结果区像日志终端、字体与换行不统一等问题。此次必须完成真正的产品化重构，不能只换颜色或增加圆角。

严格按照 `UI_UX_ADDENDUM.md` 和 `UI_WIREFRAME_COMPONENT_SPEC.md` 实现。

## 5.1 必须达到的视觉目标

- 第一眼明显比当前原型整洁、现代、专业；
- 视觉语言明显参考 CC Switch，但不复制其资产和代码；
- 保持“Companion Tool”家族感；
- 浅色主题高质量完成；
- 页面不是后台管理系统，也不是调试控制台；
- 主流程在 1366×768、1440×900、1920×1080 下均清晰可用。

## 5.2 页面结构

使用：

```text
紧凑顶部工具栏
+ 会话状态/测试控制条
+ 左侧 Provider 工作区
+ 右侧结构化结果工作区
```

要求：

- 左右区域独立滚动；
- 页面整体不要共用一个长滚动条；
- 默认窗口最小尺寸合理；
- 右侧不得窄到文字每几个字符换行；
- 安全说明改为简洁摘要 + 可打开的 Drawer/Modal，不占据整个首屏。

## 5.3 左侧列表

必须实现卡片化列表或精致 Data List，而不是粗糙 HTML 表格。

每行视觉层级：

1. 勾选框；
2. Provider 名和当前标记；
3. App 类型与脱敏 Key；
4. Host/Base URL 单行省略；
5. Model；
6. Protocol badge；
7. 可诊断/跳过/运行中/完成状态。

URL 不得按字母乱折行；模型名不得碎裂；完整内容通过 Tooltip 或详情查看。

## 5.4 主按钮与测试控制

必须有明确主按钮：

```text
开始诊断
```

并包含：

- 模式分段选择；
- 已选数量；
- 预计请求数；
- 并发数（首版固定或默认 1）；
- 运行中进度；
- 停止；
- 重新诊断。

全选/取消全选应为次级操作，不能比“开始诊断”更醒目。

## 5.5 右侧结果

不得默认显示一块原始日志文本框。

每个 Provider 使用结构化 ResultCard：

- 标题与状态 badge；
- 一句话中文结论；
- 可信度；
- 当前配置结果；
- 成功组合或失败原因；
- 建议在 CC Switch 中修改的字段；
- “上游已验证 / 端到端仅推断”证据标识；
- 尝试链折叠区；
- 原始调试日志高级折叠区；
- 复制摘要和复制建议。

原始枚举码可以显示，但不能替代自然中文说明。

## 5.6 设计系统

使用统一：

- CSS Variables / design tokens；
- Inter、Segoe UI、system-ui 字体栈；
- Lucide 或单一线性图标库；
- 统一按钮、Badge、Card、Input、Tooltip、Toast、Dialog、Drawer、Accordion；
- 统一成功、变体成功、警告、失败、跳过、未知 schema 状态颜色；
- 统一圆角、间距、边框、轻阴影；
- 完整 Hover、Focus、Active、Disabled、Loading 状态。

可使用 Tailwind CSS 和 shadcn/ui 风格组件，但不得把项目变成沉重的组件堆；保持构建可靠和视觉一致。

## 5.7 截图和视觉回归

使用完全人工构造的 synthetic fixture 数据建立仅限开发/测试的 UI fixture 模式。

通过 Playwright 或等价方式生成：

```text
docs/screenshots/main-empty.png
docs/screenshots/main-selected.png
docs/screenshots/diagnosing.png
docs/screenshots/results-mixed.png
docs/screenshots/schema-unknown.png
```

要求：

- 截图不得包含真实 URL、真实 Key、真实用户名；
- 至少覆盖 1440×900；
- 另外验证 1366×768 无关键内容溢出；
- 对核心页面添加稳定的视觉回归或结构截图测试；
- Release 前人工/自动检查截图，若仍明显像调试页面则不得发版。

---

# 六、工程与测试要求

## 6.1 前端

必须通过：

```text
format
lint
typecheck
unit test
component test
Playwright UI/e2e test
production build
```

重点测试：

- Provider 筛选；
- 选择与批量选择；
- 状态 badge；
- ResultCard 文案；
- Accordion 尝试链；
- 日志默认折叠；
- 长 URL/模型不破坏布局；
- 复制反馈；
- 空状态；
- 运行中状态；
- 取消诊断；
- 窗口尺寸下布局。

## 6.2 Rust

必须通过：

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

重点测试：

- DB 只读；
- schema fingerprint；
- Provider normalization；
- managed/OAuth 阻断；
- Secret Redactor；
- same-host URL planner；
- redirect credential stripping；
- request budget；
- protocol adapters；
- SSE parser；
- error classifier；
- cancellation；
- IPC 不包含 Secret。

## 6.3 安全门禁

建立 CI 脚本，至少检查：

- 生产源码无 process spawn；
- 无 shell plugin；
- 无受保护目录读取；
- SQLite 无写 SQL；
- 前端无 localStorage/IndexedDB；
- Release 关闭 DevTools；
- CSP 合理；
- 依赖安全扫描；
- fixture 全为 synthetic；
- 测试日志与截图不含 fixture Key 原值。

## 6.4 Windows Smoke Test

在 `windows-latest`：

- production build 成功；
- setup EXE 和便携 EXE/ZIP 存在且非空；
- 对便携版执行受控启动 smoke test；
- 验证应用启动后保持存活或呈现预期状态，再由测试脚本结束；
- 不允许 smoke test 启动任何 AI CLI；
- 验证文件版本和应用显示版本一致。

---

# 七、GitHub Actions

至少需要：

```text
.github/workflows/ci.yml
.github/workflows/release.yml
.github/workflows/upstream-watch.yml
```

## 7.1 CI

触发：push、pull_request、workflow_dispatch。

权限最小化：

```yaml
permissions:
  contents: read
```

包含：前端检查、Rust 检查、安全门禁、测试、Windows build 验证。

## 7.2 Release

触发：`v*` tag 和可选 workflow_dispatch。

必须：

```yaml
permissions:
  contents: write
```

要求：

- 第三方 Actions 固定到完整 commit SHA，并在注释中标明对应版本；
- 构建 Windows x64；
- 生成 NSIS setup EXE；
- 生成便携版 ZIP；
- 生成 `SHA256SUMS.txt`；
- 创建非草稿 Release；
- 上传资产；
- 再通过 GitHub API 或 `gh release view` 验证资产存在和大小；
- v0.1.x 固定 unsigned，不伪造签名；
- Release Notes 明确 SmartScreen 和 WebView2 Runtime 提示。

## 7.3 Upstream Watch

每日和手动触发，权限：

```yaml
permissions:
  contents: read
  issues: write
```

监控：

- CC Switch 新 Release；
- 默认分支影响 schema、Provider、协议、model mapper、config path 的关键文件变化。

要求：

- 有变化时创建或更新单一 tracking Issue；
- 避免每天重复创建同内容 Issue；
- 无变化不产生噪声；
- Issue 中不包含任何用户本地信息。

---

# 八、版本与发布决策

先查询：

```bash
git tag --list
gh release list --repo Super-YYQ/cc-switch-doctor
```

自动确定版本：

- 若远程不存在 `v0.1.0` tag 且不存在 v0.1.0 Release：发布 `v0.1.0`；
- 若 `v0.1.0` 已存在：不得覆盖、删除或重写，发布下一个未占用 patch，例如 `v0.1.1`；
- 若仓库已经有更高版本，则选择下一个未占用 patch；
- 把最终版本同步到 `package.json`、`Cargo.toml`、`tauri.conf.json` 和 UI；
- 更新 CHANGELOG；
- CI 未全绿前不得创建最终 tag。

Release 资产命名使用实际版本：

```text
CC-Switch-Doctor-v<version>-Windows-x64-setup.exe
CC-Switch-Doctor-v<version>-Windows-x64-portable.zip
SHA256SUMS.txt
```

---

# 九、仓库文档

必须完成或更新：

```text
README.md
SECURITY.md
PRIVACY.md
CHANGELOG.md
CONTRIBUTING.md
THIRD_PARTY_NOTICES.md
docs/architecture.md
docs/security-model.md
docs/privacy.md
docs/compatibility.md
docs/testing-strategy.md
docs/release-process.md
docs/implementation-audit.md
docs/screenshots/*.png
```

README 首页必须包含：

- 产品截图；
- 一句话定位；
- 支持的协议；
- 安全边界；
- 下载方式；
- SmartScreen 说明；
- “不启动任何 AI CLI”的明确声明；
- “上游 API 验证不等于 CLI 端到端验证”的证据边界；
- 构建和测试状态 badge。

---

# 十、执行策略

- 正常技术选择自行决定，不要逐项询问用户；
- 发现问题直接修复并继续；
- 不要只输出计划；
- 不要在中间阶段停下来等待确认；
- 不要为了减少工作量删除需求；
- 不要编造测试、构建、Release 或截图结果；
- 遇到失败应阅读日志、修复并重跑；
- 本地构建成功后必须继续验证远程 Actions；
- 若某一第三方依赖阻塞，优先换成更稳定、官方或自实现方案，不要降低安全边界。

唯一允许停止并报告外部阻塞的情况：

1. GitHub 未登录或无仓库写权限；
2. 仓库/组织策略禁用 Actions 或 Release；
3. 网络完全无法访问 GitHub、npm 或 crates.io；
4. 用户把可信代码签名提升为本次硬要求，但没有可用证书/签名服务。

没有可信签名证书不是 unsigned 首版发版的阻塞。

---

# 十一、完成顺序

建议按以下顺序持续执行，不等待用户确认：

1. 预检 Git/GitHub/现有代码；
2. 审计当前实现；
3. 检查 CC Switch 上游；
4. 建立/修正安全门禁；
5. 修复数据库 Adapter 与秘密边界；
6. 修复协议和诊断引擎；
7. 建立 design tokens 和基础组件；
8. 按线框重构主页面；
9. 重构 ResultCard、尝试链和高级日志；
10. 建立 synthetic UI fixture 和截图测试；
11. 完成前端/Rust/安全测试；
12. 更新文档和截图；
13. 本地 production build；
14. 提交并 push main；
15. 观察远程 CI，失败则修复、push、重跑；
16. 自动确定版本；
17. 更新版本和 CHANGELOG；
18. 再次全量检查；
19. 创建并 push tag；
20. 观察 Release workflow，失败则修复并使用新的合法版本/tag 或按安全方式处理，禁止覆盖公开 tag；
21. 验证 Release 资产和 SHA-256；
22. 确认工作区干净；
23. 输出最终交付报告。

---

# 十二、Definition of Done

只有以下全部为真才算完成：

- [ ] 当前功能和安全要求全部实现；
- [ ] 粗糙原型 UI 已被明显产品化重构；
- [ ] 视觉与交互符合两个 UI 规格文件；
- [ ] 长 URL、模型和错误不会难看乱换行；
- [ ] 左右工作区独立滚动；
- [ ] 原始日志默认折叠；
- [ ] 诊断结果为结构化卡片；
- [ ] synthetic 截图已生成并写入 README；
- [ ] 前端检查全部通过；
- [ ] Rust 检查全部通过；
- [ ] 安全门禁全部通过；
- [ ] Windows production build 成功；
- [ ] 远程 CI 成功；
- [ ] Release workflow 成功；
- [ ] 非草稿 GitHub Release 存在；
- [ ] setup EXE 存在且非空；
- [ ] portable ZIP 存在且非空；
- [ ] SHA256SUMS.txt 存在且内容正确；
- [ ] Release 资产可通过 API 查询；
- [ ] main 和 tag 已 push；
- [ ] Git 工作区干净；
- [ ] 未伪造签名；
- [ ] 未启动任何 AI CLI；
- [ ] 未触碰任何 AI 登录态；
- [ ] 未保存任何业务数据。

---

# 十三、最终答复格式

只在全部完成或确有外部阻塞时汇报。

完成时必须提供：

```text
1. 仓库地址
2. 最终版本
3. Release 地址
4. Release 资产名称和大小
5. SHA-256 验证结果
6. CC Switch baseline Release、commit 和检查时间
7. 本地测试命令与结果
8. 远程 CI URL/状态
9. Release workflow URL/状态
10. 安全门禁结果
11. UI 重构摘要
12. 截图文件列表
13. 已知限制
14. 签名状态（预计为 unsigned）
15. git status 结果
```

不得用下面的话作为完成结果：

```text
“代码已经生成，后续你自己运行”
“Release 需要你手工创建”
“UI 可以以后继续美化”
“CI 应该能通过”
“由于没有签名证书所以无法发版”
```

## 【结束执行】
