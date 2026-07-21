# CC Switch Doctor v0.1.2 审查修复任务文档

> 用途：将本文件放入仓库根目录，交给具备代码、终端、Git 和 GitHub Actions 权限的 AI 编程工具执行。
>
> 目标：在不推倒重写、不改变现有产品定位、不破坏已完成 UI 的前提下，完成本次审查发现的问题修复，并发布 `v0.1.2`。

---

## 一、项目与执行范围

仓库：

```text
Super-YYQ/cc-switch-doctor
```

目标版本：

```text
v0.1.2
```

当前状态：

- v0.1.1 已完成 UI 产品化重构；
- 当前技术栈为 Tauri 2 + React + TypeScript + Rust；
- 当前产品定位正确：只读扫描 CC Switch、纯 HTTP 请求、不触发任何 AI CLI、不读取登录态、不保存业务数据；
- 本次不允许重新设计整体 UI，不允许推倒核心架构；
- 本次只针对审查问题进行精确修复、测试、文档更新和发版。

---

## 二、必须继续遵守的安全红线

以下约束不得因本次修复而削弱：

1. 不得启动或调用：
   - Codex / Codex CLI
   - Claude / Claude Code
   - OpenCode
   - Gemini CLI
   - CC Switch
   - PowerShell、cmd、bash、wsl 或其他运行时子进程
2. 不得读取或修改：
   - `.codex`
   - `.claude`
   - `.claude.json`
   - OpenCode 配置目录
   - `.gemini`
3. CC Switch 数据库必须：
   - SQLite `mode=ro`
   - `SQLITE_OPEN_READ_ONLY`
   - `PRAGMA query_only=ON`
   - 不执行写入
4. 完整 API Key 只能存在 Rust 内存：
   - 不进入 WebView
   - 不进入 DOM
   - 不进入日志
   - 不进入文件
   - 不进入 localStorage / IndexedDB
5. 自动请求变体只能访问原始 Base URL 的相同：
   - scheme
   - host
   - effective port
6. 跨 Host 重定向必须阻断，且不得继续携带凭据。
7. 官方订阅、OAuth、Codex OAuth、GitHub Copilot、ChatGPT Backend 等托管认证必须继续安全跳过。

---

# 三、P1 必须修复

以下问题全部完成后，才能发布 v0.1.2。

---

## P1-1：请求预算必须按整个诊断会话的 Host 共享

### 当前问题

当前 `host_request_count` 位于 `diagnose_one()` 内部，因此每个 Provider 都会独立获得 30 次同 Host 请求额度。

错误行为示例：

```text
5 个 Provider 都指向 api.example.com
当前实现最多可能请求 5 × 30 = 150 次
```

正确要求：

```text
整个诊断会话内，同一 scheme + host + effective port 总计最多 30 次
```

### 修改要求

在 `run_diagnosis()` 级别创建共享预算对象，例如：

```rust
Arc<Mutex<HashMap<OriginKey, HostBudget>>>
```

建议结构：

```rust
struct OriginKey {
    scheme: String,
    host: String,
    port: u16,
}

struct HostBudget {
    sent: usize,
    consecutive_rate_limits: usize,
}
```

每次真实发送 HTTP 请求前必须原子执行：

1. 读取当前 Origin 的已发送数量；
2. 达到 30 次则停止发送；
3. 未达到则先占用额度，再发送请求；
4. 并发情况下不得超额；
5. 取消、构造失败或被同源策略阻断且未真正发送的请求，不应计入“已发送 HTTP 请求”；
6. 一旦实际执行 `reqwest.send()`，则计入预算。

### 额外要求：连续 429 停止

同一个 Host 连续两次明确返回：

```text
RATE_LIMITED
HTTP 429
```

则该 Host 在本次会话中停止继续请求。

### 额外要求：请求结果去重

同一诊断会话中，相同测试组合应复用内存结果：

```text
scheme + host + effective port
+ key fingerprint
+ protocol
+ model
+ request purpose
+ stream
+ tool_call
+ token limit field
```

要求：

- 不得保存完整 Key 作为 HashMap Key；
- 使用当前进程内存中的不可逆短指纹，例如 SHA-256 后截取；
- 去重结果只存在本次 run 的 Rust 内存；
- 关闭或结束 run 后释放。

### 验收测试

必须新增 Rust 测试：

1. 两个 Provider 指向同一个 Host，共享 30 次预算；
2. 并发 3 时仍不能超过 30 次；
3. 不同 Host 各自拥有独立预算；
4. 连续两次 429 后该 Host 停止；
5. 相同测试组合第二次命中内存缓存，不产生新 HTTP 请求；
6. 不同 Key 指纹不得错误复用结果。

---

## P1-2：真正实现 `max_completion_tokens` → `max_tokens` 兼容回退

### 当前问题

当前 OpenAI Chat 请求固定使用：

```json
{
  "max_completion_tokens": 32
}
```

代码注释声称不兼容时会回退到 `max_tokens`，但实际没有对应执行逻辑。

这会把旧中转站或旧 OpenAI 兼容接口误判为不可用。

### 修改要求

新增明确类型：

```rust
enum TokenLimitField {
    MaxCompletionTokens,
    MaxTokens,
}
```

`build_chat_request()` 必须由调用方明确指定使用哪个字段。

默认顺序：

```text
第一次：max_completion_tokens
第二次：仅在明确字段不支持时，使用 max_tokens
```

只有错误响应明确表达以下语义时才允许回退：

```text
unknown parameter
unsupported parameter
unrecognized field
invalid field
max_completion_tokens
```

不得因为以下错误触发字段回退：

- 401 / Key 无效
- 403 / 权限不足
- 402 / 余额不足
- 429 / 限流
- DNS / TLS / 网络失败
- 模型不存在
- 404 Endpoint 不存在

### 计划与预算要求

- 回退请求必须计入 Provider 最大尝试数；
- 回退请求必须计入 Host 会话预算；
- UI 尝试链需要明确显示：

```text
字段兼容回退：max_completion_tokens → max_tokens
```

### 验收测试

必须新增测试：

1. 第一次请求体含 `max_completion_tokens`，不含 `max_tokens`；
2. 明确字段不支持时，第二次使用 `max_tokens`；
3. 401 不触发回退；
4. 429 不触发回退；
5. 模型不存在不触发回退；
6. 两次请求都失败时，最终诊断应准确说明字段回退已尝试。

---

## P1-3：刷新数据库后必须彻底清理旧会话状态

### 当前问题

每次后端扫描会重新生成随机 `opaque_id`，但前端点击“刷新”后只替换 `scan`，没有同步清理：

- selected
- activeId
- summaries
- liveLog
- runningIds
- completedCount
- sentRequests
- currentName
- runId

这会导致刷新后显示旧结果、旧选择或失效 ID。

### 修改要求

前端新增统一函数：

```ts
function applyFreshScan(view: ProviderScanView) {
  setScan(view);
  setSelected(new Set());
  setActiveId(null);
  setSummaries([]);
  setLiveLog([]);
  setRunningIds(new Set());
  setCompletedCount(0);
  setSentRequests(0);
  setCurrentName(null);
  setRunId(null);
  setRunning(false);
  setError(null);
}
```

以下流程必须全部调用该函数：

1. 应用首次扫描成功；
2. 点击“刷新配置”；
3. 手动选择 DB；
4. 数据库路径变化；
5. 兼容性状态从可测试变为未知或不支持。

### 运行中刷新

运行诊断期间：

- 刷新按钮应保持禁用；
- 选择 DB 应保持禁用；
- 不允许切换数据库；
- 停止诊断后才可刷新。

### 验收测试

新增前端测试：

1. 刷新后 selected 为空；
2. activeId 清空；
3. 旧 summaries 清空；
4. 旧 liveLog 清空；
5. 新 Provider ID 不会和旧选择混用；
6. 运行中无法刷新或选择 DB。

---

# 四、P2 应修复

以下问题应在 v0.1.2 一并完成。

---

## P2-1：区分“观察到的 CC Switch 版本”和“已验证版本”

### 当前问题

应用的 `load_verified_release()` 实际读取：

```text
ccSwitch.latestObservedRelease
```

但该字段只代表“监控到的最新上游版本”，不代表 Doctor 已完成兼容验证。

### 修改要求

在 `compatibility/manifest.json` 中新增：

```json
{
  "ccSwitch": {
    "latestObservedRelease": "x.y.z",
    "latestVerifiedRelease": "x.y.z",
    "verifiedReleases": ["x.y.z"]
  }
}
```

规则：

- `latestObservedRelease`：上游监控使用；
- `latestVerifiedRelease`：应用 UI 和安全兼容结论使用；
- `verifiedReleases`：完整已验证版本列表；
- `compatibleReleasePrefixes`：只表示推定兼容，不得显示成“已验证”。

更新模块必须读取：

```text
latestVerifiedRelease
```

### UI 文案

需要区分：

```text
CC Switch 最新：3.18.0
Doctor 已验证：3.17.0
状态：发现新版本，尚未完成兼容验证
```

不得写成：

```text
与已验证基线一致或兼容
```

除非有明确规则支持。

### 验收测试

1. latestObserved > latestVerified 时显示“尚未验证”；
2. 两者一致时显示“已验证”；
3. compatible prefix 只能显示“推定兼容”，不能显示“已验证”；
4. Manifest 缺失字段时安全回退，不得错误升级验证结论。

---

## P2-2：Upstream Watch 不得重复创建 Issue

### 当前问题

工作流查重依赖 `upstream-change` 标签，但标签可能不存在；fallback 创建的 Issue 不带标签，第二天会重复创建。

### 修改要求

工作流开始时确保标签存在：

```bash
gh label create upstream-change \
  --color 8250df \
  --description "CC Switch upstream compatibility review" \
  --force
```

查重逻辑至少使用：

```text
精确标题 + open 状态
```

不要只依赖标签。

推荐：

```bash
EXISTING=$(gh issue list \
  --state open \
  --search "in:title upstream-change: CC Switch v${LATEST}" \
  --json number,title \
  --jq 'map(select(.title == "upstream-change: CC Switch v'"${LATEST}"'"))[0].number // empty')
```

### 验收

- 标签不存在时可以自动创建；
- 同一个上游版本连续执行两次，只保留一个 open Issue；
- Issue 已关闭后是否重新创建，应在文档中明确规则；建议同版本不再重复创建，除非 manifest 回退或手动触发特殊参数。

---

## P2-3：移除当前无效的“可选代码签名”步骤

### 当前问题

当前 Release Workflow 只把 Base64 PFX 写入临时文件，但没有：

- 导入证书；
- 获取 thumbprint；
- 调用 signtool；
- 配置 Tauri sign command；
- 使用证书密码；
- 验证数字签名。

因此目前该步骤不能真正签名。

### 修改要求

v0.1.2 固定发布 unsigned 版本：

1. 删除当前无效的 Optional code signing cert 步骤；
2. Release Notes 明确：

```text
This release is unsigned.
```

3. README 保留 SmartScreen 说明；
4. 不再声称“若配置 Secrets 则自动签名”；
5. 在 `docs/code-signing.md` 可记录未来支持计划，但不得影响当前发布。

### 验收

- 工作流不引用未实现的证书环境变量；
- Release Notes 与实际签名状态一致；
- EXE 属性检查结果允许 unsigned；
- SHA256SUMS 必须继续提供。

---

## P2-4：所有 GitHub Actions 固定到完整 Commit SHA

### 当前问题

当前使用浮动标签：

```yaml
actions/checkout@v4
actions/setup-node@v4
Swatinem/rust-cache@v2
actions/upload-artifact@v4
softprops/action-gh-release@v2
dtolnay/rust-toolchain@stable
```

### 修改要求

所有第三方 Actions 必须固定到 40 位 commit SHA，并保留版本注释：

```yaml
- uses: actions/checkout@<40位SHA> # v4.x.x
```

范围包括：

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.github/workflows/upstream-watch.yml`
- 其他 workflow

不得保留：

```text
@main
@master
@stable
@v2
@v4
```

如 `dtolnay/rust-toolchain` 的使用方式难以满足固定 SHA，应改成：

- 固定 SHA 的 Action；或
- 使用官方 rustup 命令安装固定 stable toolchain；
- 但不要引入未审查的新 Action。

### 自动检查

新增脚本：

```text
scripts/verify-actions-pinned.mjs
```

扫描 `.github/workflows/*.yml`，任何 `uses:` 未固定 40 位 SHA 时失败。

将其加入：

```text
npm run security:verify
```

---

## P2-5：Release 必须强校验版本一致性

### 当前问题

`workflow_dispatch` 默认版本仍可能是旧版本，且当前没有验证输入版本与源码版本一致。

### 修改要求

发布前读取并比较：

```text
输入版本 / tag
package.json version
src-tauri/Cargo.toml package.version
src-tauri/tauri.conf.json version
compatibility/manifest.json doctorVersion
```

要求全部一致。

任何不一致：

```text
立即失败，禁止创建 Release
```

`workflow_dispatch`：

- 删除默认版本值；
- 要求显式输入；
- 输入必须符合 `x.y.z`；
- 不允许覆盖已有 Tag 或 Release。

Tag 触发时：

```text
GITHUB_REF_NAME 去掉 v 后必须与源码版本一致
```

新增脚本建议：

```text
scripts/verify-release-version.mjs
```

---

# 五、P3 建议修复

---

## P3-1：正确支持 Windows UNC SQLite 路径

### 当前问题

Windows UNC canonical path 可能为：

```text
\\?\UNC\server\share\cc-switch.db
```

当前简单移除 `\\?\` 后可能生成错误 SQLite URI。

### 修改要求

显式区分：

```text
普通盘符路径
UNC 路径
```

UNC 应生成类似：

```text
file://server/share/cc-switch.db?mode=ro
```

必须保持：

- mode=ro
- URI 编码正确
- 空格和特殊字符正确编码

增加 Windows 单元测试。

---

## P3-2：修复 Provider 卡片的交互语义

### 当前问题

当前外层：

```tsx
<article role="button" tabIndex={0}>
```

内部包含：

```tsx
<input type="checkbox" />
```

会形成不理想的嵌套交互语义。

### 修改建议

二选一：

#### 方案 A（推荐）

- 外层卡片使用普通 `<article>`；
- Checkbox 负责勾选；
- 卡片标题区提供独立 `查看详情` 按钮；
- 点击卡片非交互区域只切换 active，不切换 checked。

#### 方案 B

- 整卡点击直接切换勾选；
- 不再区分 active 和 selected；
- 但右侧详情选择方式需要重新设计。

优先采用方案 A，改动最小。

需要保留键盘操作和焦点态。

---

## P3-3：修正只读测试中的错误注释

当前测试只比较文件长度，却写成 hash stability。

修改为：

- 注释改为 size stability；或
- 直接复用 SHA-256 真正验证前后文件一致。

推荐统一使用 SHA-256。

---

## P3-4：清理重复规格文件

### 当前问题

仓库根目录同时存在多份重复或历史规格，容易让后续 AI 读取错误版本。

### 修改要求

保留规范化结构：

```text
docs/project/
├─ PROJECT_SPEC.md
├─ UI_UX_ADDENDUM.md
├─ UI_WIREFRAME_COMPONENT_SPEC.md
├─ FINAL_GOAL_PROMPT.md
└─ REVIEW_FIX_SPEC_v0.1.2.md
```

删除或归档根目录中的旧重复文件：

```text
CC Switch Doctor 项目需求与单目标交付设计文档.md
```

根目录可保留：

```text
README.md
AGENTS.md
CHANGELOG.md
SECURITY.md
PRIVACY.md
LICENSE
```

移动文档后需修正 README、AGENTS 或其他链接。

---

# 六、UI 要求

当前 v0.1.1 UI 已基本通过，不允许大范围推倒重做。

本次仅允许：

- 修复刷新状态；
- 增加必要的更新状态文案；
- 优化 Provider 卡片无障碍语义；
- 在尝试链中显示 token 字段回退；
- 显示请求复用或预算停止原因；
- 保持现有 CC Switch companion 风格。

不得：

- 改回调试页样式；
- 删除结构化结果卡片；
- 默认展开原始日志；
- 把大量原始错误堆到首屏；
- 改变现有左右主布局。

---

# 七、诊断结果新增文案

## Host 预算耗尽

```text
已停止继续请求：该 Host 在本次诊断会话中已达到 30 次请求上限。
```

## 连续限流

```text
已停止继续请求：该 Host 连续两次返回限流响应，避免进一步消耗配额或触发封禁。
```

## 复用结果

```text
本次结果复用了同一会话内相同配置组合的已完成请求，未重复发送 HTTP 请求。
```

## Token 字段回退成功

```text
接口不支持 max_completion_tokens，切换为 max_tokens 后请求成功。
```

## 上游版本未验证

```text
检测到 CC Switch 新版本，但 Doctor 尚未完成该版本的兼容验证。当前不会自动升级兼容结论。
```

---

# 八、测试与质量门禁

完成修改后必须运行并通过：

## 前端

```bash
npm run format:check
npm run lint
npm run typecheck
npm run test
npm run build
```

## Rust

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## 安全门禁

```bash
npm run security:verify
```

安全门禁必须至少包含：

- 无生产进程启动能力；
- 无受保护登录目录读取；
- 版本同步；
- GitHub Actions 固定 SHA；
- 不允许 Shell 插件依赖；
- 不允许完整测试 Key 泄漏到前端 Fixture 或快照。

## Windows 构建

```bash
npm run tauri build -- --bundles nsis
```

必须验证：

- NSIS 安装包存在；
- 主 EXE 存在；
- 便携 ZIP 正确；
- SHA256SUMS.txt 正确；
- 文件大小大于 0；
- 版本号一致。

---

# 九、GitHub Actions 验收

必须确认远程执行：

1. CI workflow 全绿；
2. Release workflow 全绿；
3. Upstream Watch 手动执行成功；
4. Upstream Watch 连续执行两次不会创建重复 Issue；
5. Release workflow 发现版本不一致时会失败；
6. 所有 Actions 已固定 Commit SHA；
7. Release 不包含虚假签名说明。

---

# 十、v0.1.2 发版要求

版本统一更新为：

```text
0.1.2
```

需要更新：

- `package.json`
- `package-lock.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`（如需要）
- `src-tauri/tauri.conf.json`
- `compatibility/manifest.json` 的 `doctorVersion`
- `CHANGELOG.md`
- README 支持范围版本

Tag：

```text
v0.1.2
```

Release 名称：

```text
CC Switch Doctor v0.1.2
```

Release 资产：

```text
CC-Switch-Doctor-v0.1.2-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.2-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release Notes 必须包含：

- 修复跨 Provider 共享 Host 请求预算；
- 增加重复请求内存复用；
- 增加 `max_tokens` 兼容回退；
- 修复刷新后旧状态残留；
- 区分上游最新版本与已验证版本；
- 修复 Upstream Watch 重复 Issue；
- Actions 固定 SHA；
- 当前 Release 为 unsigned；
- 不触发任何 AI CLI；
- 不读取官方登录态；
- 数据库只读。

---

# 十一、完成定义

只有同时满足以下条件，任务才算完成：

- 所有 P1 已修复；
- 所有 P2 已修复；
- P3 已处理或在最终报告中逐项说明未处理原因；
- 本地测试全部通过；
- Windows 构建成功；
- 远程 CI 全绿；
- `v0.1.2` Release 已成功创建；
- 三个资产可下载且大小大于 0；
- SHA256 校验文件正确；
- Git 工作区干净；
- 没有 force push；
- 没有删除用户有效历史；
- 没有虚构测试结果。

---

# 十二、最终汇报格式

AI 工具完成后只按以下格式汇报：

```text
## 完成状态
- 版本：v0.1.2
- Commit：<SHA>
- Tag：v0.1.2
- Release：<URL>

## P1 修复
- 共享 Host 请求预算：完成 / 未完成
- 会话内请求去重：完成 / 未完成
- max_tokens 回退：完成 / 未完成
- 刷新状态清理：完成 / 未完成

## P2 修复
- observed / verified 版本区分：完成 / 未完成
- upstream issue 去重：完成 / 未完成
- 无效签名步骤移除：完成 / 未完成
- Actions 固定 SHA：完成 / 未完成
- Release 版本一致性校验：完成 / 未完成

## P3 修复
- UNC 路径：完成 / 未完成
- Provider 卡片语义：完成 / 未完成
- SHA-256 只读测试：完成 / 未完成
- 重复规格清理：完成 / 未完成

## 测试结果
- Frontend format：通过 / 失败
- Frontend lint：通过 / 失败
- Typecheck：通过 / 失败
- Frontend tests：通过 / 失败
- Frontend build：通过 / 失败
- Rust fmt：通过 / 失败
- Rust clippy：通过 / 失败
- Rust tests：通过 / 失败
- Security verify：通过 / 失败
- Tauri Windows build：通过 / 失败

## GitHub Actions
- CI：成功 / 失败
- Release：成功 / 失败
- Upstream Watch：成功 / 失败

## Release 资产
- setup.exe：存在 / 缺失，大小
- portable.zip：存在 / 缺失，大小
- SHA256SUMS.txt：存在 / 缺失

## 安全确认
- 未启动 AI CLI：是 / 否
- 未读取登录目录：是 / 否
- DB 保持只读：是 / 否
- 完整 Key 未进入前端或日志：是 / 否
- 跨 Host 请求已阻断：是 / 否

## 已知限制
- <仅列真实仍存在限制>
```

不得以“代码已完成但需要用户自己发版”结束。
