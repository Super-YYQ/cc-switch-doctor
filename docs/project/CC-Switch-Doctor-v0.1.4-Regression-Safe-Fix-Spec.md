# CC Switch Doctor v0.1.4 回归安全修复开发文档

> 目标仓库：`Super-YYQ/cc-switch-doctor`  
> 当前基线：`v0.1.3`  
> 目标版本：`v0.1.4`  
> 文档性质：可直接交给 Codex、Claude Code 或其他 AI 开发工具执行的定向修复规格  
> 首要原则：**修复当前缺陷时，禁止破坏此前已经完成并验收通过的功能。不得推倒重做，不得顺手重构无关模块。**

---

# 0. 给 AI 工具的直接执行指令

请先完整阅读本文件，再开始修改代码。

你正在修复一个已经发布多个版本的 Windows 桌面工具，不是在新建项目。必须遵守以下执行纪律：

1. 基于当前 `main / v0.1.3` 做最小范围修复；
2. 创建独立修复分支，例如 `fix/v0.1.4-regression-safe`；
3. 修改前先运行现有测试并记录基线；
4. 先修复 v0.1.3 的回归问题，再处理诊断引擎问题；
5. 每完成一组修复，立即运行对应测试，不要积累到最后一起验证；
6. 不允许为了通过测试删除、跳过、放宽或伪造安全检查；
7. 不允许大范围重构 UI、状态管理、Tauri 命令、数据库读取或诊断架构；
8. 不允许改变未在本文件中明确要求修改的用户行为；
9. 不允许删除现有功能、按钮、状态、日志、模式或安全说明；
10. 不允许覆盖已有 `v0.1.3` Tag 或 Release；
11. 全部测试、Windows 构建和远程 CI 成功后，才能发布 `v0.1.4`；
12. 远程 CI 或 Release 失败时继续修复，不能在失败状态下宣布完成。

最终交付必须包含：

- 修改文件清单；
- 每项缺陷的根因与修复方式；
- 新增和更新的测试；
- 本地测试结果；
- GitHub Actions 状态；
- v0.1.4 Tag 对应提交 SHA；
- Release 资产及 SHA-256；
- 已知限制。

---

# 1. 回归保护红线

## 1.1 已通过能力必须冻结

以下能力视为已经通过，除非本文件明确要求，不得改变其语义：

- CC Switch 数据库严格只读：`mode=ro`、`SQLITE_OPEN_READ_ONLY`、`query_only=ON`；
- 不写入 CC Switch 数据库；
- 不切换 Provider；
- 不启动 Codex、Claude Code、Claude、OpenCode、Gemini CLI、CC Switch；
- 不启动 Shell、PowerShell、CMD 或任意外部进程；
- 不读取 `.codex`、`.claude`、`.gemini`、OpenCode 等登录目录；
- 完整 API Key 只存在于 Rust 内存中，不进入前端；
- 不保存诊断历史、选择、结果、日志或 Key；
- 不使用 `localStorage`、IndexedDB 或持久化 Store 保存业务数据；
- 自动 URL 变体只允许同 Origin；
- 不跟随跨 Host 重定向；
- 默认并发为 1，最大并发为 3；
- 同一 Origin 单次会话最多 30 次真实请求；
- 连续两次限流后停止该 Origin；
- Quick / Smart / Deep 三种诊断模式继续保留；
- 当前 CC Switch 自定义目录读取机制继续保留；
- 现有 CC Switch 风格 UI、结果卡片、尝试链、日志折叠继续保留；
- Windows NSIS 安装包和 portable ZIP 发布流程继续保留。

## 1.2 禁止“修一处坏一处”

任何修改都必须满足：

```text
新功能测试通过
+
旧功能回归测试通过
+
安全门禁通过
+
Windows 构建通过
```

不得只验证新需求，不验证旧能力。

## 1.3 修改范围控制

优先修改以下文件及其对应测试：

- `src/App.tsx`
- `src/components/ProviderWorkspace.tsx`
- 与三点菜单对应的组件
- `src/components/SessionControlBar.tsx`
- `src/lib/utils.ts`
- `src-tauri/src/ccs_adapter/fingerprint.rs`
- `compatibility/manifest.json`
- `src-tauri/src/diagnostics/classifier.rs`
- `src-tauri/src/diagnostics/engine.rs`
- `src-tauri/src/diagnostics/session_budget.rs`
- `src-tauri/src/protocols/http_executor.rs`
- `src-tauri/src/security/redact.rs`
- 对应测试文件

除非确有必要，不修改：

- Tauri capability；
- SQLite 只读实现；
- CSP；
- Release 资产命名；
- 应用整体布局；
- 已完成的视觉 Token。

---

# 2. v0.1.3 新增回归问题（最高优先级）

## P0-1：左侧 Provider 列表为空，应用筛选只剩“全部”

### 用户现象

升级到 v0.1.3 后：

- 左侧 Provider 配置区没有任何 Provider；
- 应用筛选栏只剩“全部”；
- 原本的 Claude、Codex、Gemini 等筛选项消失；
- 无法继续诊断。

### 高概率根因

v0.1.3 将 Schema 门禁收紧为仅允许 `user_version=15`，而用户当前 CC Switch 数据库为：

```text
user_version=13
```

当前代码中 `Compatible` 分支被写成永远不可达的逻辑，导致 v13 被标记为 Unknown 并停止读取 Provider。

同时，前端应用筛选项可能根据已加载 Provider 动态生成；当 Provider 数组为空时，只剩“全部”。

### 修复要求

#### A. 正确支持经过确认的 Schema v13

不得恢复宽泛规则：

```rust
12..=20 都视为 Compatible
```

必须采用精确兼容指纹：

1. 对照 CC Switch 对应历史版本源码或现有真实 v13 数据结构；
2. 确认以下内容：
   - `providers` 必需列；
   - `provider_endpoints` 是否存在及列结构；
   - `settings_config` 语义；
   - `is_current` 语义；
   - Provider 凭据读取方式；
3. 在 `compatibility/manifest.json` 增加 v13 精确条目；
4. 代码按 Manifest 中的精确指纹匹配；
5. v13 状态可标记为：
   - `compatible`：结构已确认但未达到 verified；
   - 或 `verified`：确实完成对应上游版本验证后；
6. 未命中精确指纹的 Schema 仍然安全停止。

示意：

```json
{
  "userVersion": 13,
  "status": "compatible",
  "requiredTables": ["providers", "provider_endpoints"],
  "providersColumns": ["精确列清单"],
  "providerEndpointsColumns": ["精确列清单"]
}
```

#### B. 应用筛选标签不能依赖 Provider 是否成功加载

Provider 应用筛选至少固定展示受支持的核心类型：

```text
全部 / Claude / Codex / Gemini / OpenCode
```

可以继续展示其他已支持类型，但不得因为当前某一类数量为 0 就让核心标签完全消失。

当某标签无匹配 Provider 时：

- 保留标签；
- 列表显示友好空状态；
- 不崩溃；
- 不错误显示为数据库为空。

#### C. 明确区分三种空状态

1. `Schema 不兼容`：显示 Schema 阻断说明；
2. `数据库中没有该应用 Provider`：显示该筛选下无配置；
3. `搜索无结果`：显示搜索条件无匹配。

不得把三种情况统一显示成空白区域。

### 强制测试

- synthetic v13 fixture 能读取 Provider；
- v13 fixture 的 Provider 数量大于 0；
- v13 未知变体仍安全停止；
- v15 仍正常读取；
- 未知 v16 不会因为范围判断被放行；
- Provider 数组为空时，核心筛选标签仍显示；
- Claude 筛选无数据时显示明确空状态；
- 列表恢复后 Provider 卡片、脱敏 Key、模型和协议正常显示。

---

## P0-2：对“默认选中 CC Switch 配置”的需求理解错误

### 用户真实需求

此前说的“默认选中 CCS 配置”不是指：

```text
自动勾选 CC Switch 中 is_current=true 的 Provider 行
```

真实需求是：

> Provider 配置区顶部的应用筛选标签中，默认选中 `Claude`，而不是 `全部`。

### 修复要求

#### A. 默认筛选

应用启动、刷新配置、重新选择数据库后：

```ts
appFilter = "claude";
```

视觉上必须是：

```text
全部  [Claude]  Codex  Gemini ...
```

#### B. Provider 行不得因误解而自动勾选

删除或停用以下默认行为：

```ts
setSelected(defaultSelectedIds(view.providers));
```

默认应为：

```ts
setSelected(new Set());
```

用户必须主动勾选要诊断的 Provider。

保留可选批量操作：

- 全选当前筛选；
- 取消全选；
- 可选增加“选择当前配置”，但不得默认执行。

#### C. 不得影响运行中选择锁定

- 诊断运行时不可切换选择；
- 刷新或选 DB 后清空 Provider 行选择；
- 默认筛选恢复为 Claude；
- 开始诊断按钮在未勾选 Provider 时保持禁用。

### 强制测试

- 初始 `appFilter` 为 `claude`；
- Claude 标签具有 active 状态；
- “全部”不是默认 active；
- 初始 Provider checkbox 全部未勾选；
- 开始诊断按钮初始禁用；
- 用户勾选后按钮启用；
- 刷新后行选择清空；
- 刷新后 Claude 仍为默认筛选；
- 选择新 DB 后行为一致。

---

## P1-1：三点菜单打开后，点击页面其他位置不自动关闭

### 用户现象

Provider 配置区点击右侧 `...` 打开小型筛选/批量操作菜单后：

- 点击页面其他区域，菜单仍然悬浮；
- 容易遮挡内容；
- 必须再次点击三点按钮才能关闭。

### 修复要求

菜单必须支持：

1. 点击菜单外任意区域自动关闭；
2. 点击 `Esc` 自动关闭；
3. 点击菜单项执行后自动关闭；
4. 再次点击触发按钮可切换关闭；
5. 同一时间只允许一个菜单打开；
6. 组件卸载时清理全局事件监听；
7. 点击菜单内部不会被外部点击逻辑提前关闭；
8. 不影响搜索框、复选框、筛选标签和滚动。

推荐实现：

```tsx
const menuRef = useRef<HTMLDivElement>(null);
const triggerRef = useRef<HTMLButtonElement>(null);

useEffect(() => {
  if (!open) return;

  const onPointerDown = (event: PointerEvent) => {
    const target = event.target as Node;
    if (menuRef.current?.contains(target)) return;
    if (triggerRef.current?.contains(target)) return;
    setOpen(false);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") setOpen(false);
  };

  document.addEventListener("pointerdown", onPointerDown);
  document.addEventListener("keydown", onKeyDown);

  return () => {
    document.removeEventListener("pointerdown", onPointerDown);
    document.removeEventListener("keydown", onKeyDown);
  };
}, [open]);
```

可使用现有 Popover 组件或轻量实现，但不要为了这个问题引入大型 UI 框架。

### 强制测试

- 点击三点按钮打开；
- 点击菜单内部不关闭，除非执行菜单项；
- 点击页面空白关闭；
- 点击搜索框关闭；
- 点击 Provider 卡片关闭；
- 按 Esc 关闭；
- 执行菜单项后关闭；
- 多次打开关闭不会重复注册监听；
- 组件卸载后没有遗留监听。

---

# 3. v0.1.3 诊断引擎遗留问题

## P0-3：HTTP 200 中的错误响应仍被误判为 UNSUPPORTED_PROTOCOL

### 当前问题

很多中转站会用 HTTP 200 返回业务错误：

```json
{ "code": 1008, "msg": "余额不足" }
```

```json
{ "code": 40101, "message": "invalid api key" }
```

也可能用 HTTP 200 返回 HTML/WAF 页面。

当前执行器只在以下情况调用错误分类：

- HTTP 非 2xx；
- JSON 顶层存在 `error`。

解析不到正常文本时仍可能直接返回 `UNSUPPORTED_PROTOCOL`。

### 修复要求

所有响应按以下顺序处理：

1. 记录 HTTP 状态和 Content-Type；
2. 安全读取受限大小的响应体；
3. 检查 HTML / Cloudflare / WAF；
4. 检查协议正常成功结构；
5. 检查通用错误结构：
   - `error`；
   - `message`；
   - `msg`；
   - `code`；
   - `error_code`；
   - 嵌套错误对象；
6. 调用 `classify_with_evidence(status, body, content_type)`；
7. 仅在没有任何认证、额度、限流、模型、网关或参数证据时，才允许返回：
   - `RESPONSE_FORMAT_MISMATCH`；
   - 最终多协议均不兼容时再归纳为 `UNSUPPORTED_PROTOCOL`。

### 强制测试

- `200 + 余额不足` → `QUOTA_EXHAUSTED`；
- `200 + invalid api key` → `AUTH_INVALID`；
- `200 + permission denied` → `AUTH_PERMISSION_DENIED`；
- `200 + HTML Cloudflare` → `GATEWAY_OR_WAF`；
- `200 + 非标准 JSON 无错误证据` → `RESPONSE_FORMAT_MISMATCH`；
- 多种协议均格式不兼容 → 最终 `UNSUPPORTED_PROTOCOL`；
- 不能把明确 Key/余额错误覆盖为 `UNSUPPORTED_PROTOCOL`。

---

## P1-2：错误证据必须真正传到前端

### 当前问题

后端已经定义 `ErrorEvidence`，但 `AttemptResult` 没有携带实际证据；前端“可能原因”只是固定文案。

### 修复要求

在 `AttemptResult` 增加：

```rust
pub error_evidence: Vec<ErrorEvidence>
```

要求：

- 不含完整 Key；
- 不含完整 Authorization；
- 可包含 HTTP 状态、匹配关键词、错误字段名；
- 前端结果卡片优先显示实际证据；
- 没有证据时才显示通用“可能原因”。

示例：

```text
检测依据：
- HTTP 200
- 错误字段：msg
- 匹配关键词：余额不足
- 结论：额度不足
```

---

## P1-3：异步 Single-flight 禁止使用 std::sync::Condvar

### 当前问题

异步诊断流程中使用阻塞式 `Condvar` 等待相同请求，可能占满 Tokio Worker 并造成卡死。

### 修复要求

改用异步原语：

- `tokio::sync::Notify`；
- `tokio::sync::watch`；
- `tokio::sync::oneshot`；
- 或共享 Future。

所有等待必须 `.await`，不得阻塞异步运行时线程。

必须处理：

- Leader 成功；
- Leader 失败；
- Leader 被取消；
- Leader 超时；
- Leader Panic / Drop；
- Waiter 必须最终被唤醒。

---

## P1-4：取消诊断必须彻底消除旧 Run 竞争

### 当前问题

前端收到 `run_cancelled` 后立即允许再次启动，但旧 Run 随后仍会发送 `run_finished`，可能覆盖新 Run 状态。

### 修复要求

- `run_cancelled` 只显示“正在收尾”；
- 不在 `run_cancelled` 时设置 `running=false`；
- 只有匹配当前 Run 的 `run_finished` 才结束会话；
- 没有 active Run 时拒绝所有诊断事件；
- 新 Run 开始后，旧 Run 的任何事件都必须忽略；
- 后端 `cancel_run(run_id)`、`complete_run(run_id)` 保持精确匹配。

---

## P1-5：统一认证错误状态名和停止策略

### 当前问题

分类器返回 `AUTH_INVALID`，部分引擎逻辑仍检查 `KEY_INVALID`，导致明确的无效 Key 后继续大量尝试。

### 修复要求

建立统一状态常量或 Enum，禁止散落字符串：

```text
AUTH_INVALID
AUTH_PERMISSION_DENIED
QUOTA_EXHAUSTED
RATE_LIMITED
```

停止策略：

- `AUTH_INVALID`：仅允许一次合理认证方式变体，仍失败则停止该 Provider；
- `AUTH_PERMISSION_DENIED`：不继续模型、Streaming、Tool Calling 大量尝试；
- `QUOTA_EXHAUSTED`：立即停止；
- `RATE_LIMITED`：按 Host 规则停止；
- 不得继续无意义消耗请求。

---

## P1-6：完整 Key 不得出现在 URL Path、缓存键或前端

### 修复要求

所有前端和日志 URL 必须经过已注册 Key 的 Redactor：

- `safe_base_url`；
- Attempt URL；
- success URL；
- 建议文本；
- 复制摘要；
- 调试日志；
- 缓存键 Debug。

Query 处理：

- Key 名保留；
- 敏感值用固定占位符或不可逆指纹；
- 非敏感但影响语义的值使用稳定哈希，不能全部变成同一个 `*`；
- API Version 不同不能错误命中同一缓存。

Path 处理：

- 若 Path 中包含已注册完整 Key，必须遮盖；
- 缓存键不得保留原始 Path Secret。

---

# 4. P2 完整性修复

## P2-1：严格执行每 Provider 真实请求上限

真实发送上限：

```text
Quick：最多 2 次
Smart：最多 12 次
Deep：最多 16 次
```

动态产生的请求同样计入：

- `max_tokens` 回退；
- 认证方式回退；
- Query Key 回退；
- 稳定性复测；
- Streaming；
- Tool Calling。

UI 预计请求数必须与真实上限一致。

---

## P2-2：非流式响应必须增量限制 2MB

不得先执行完整：

```rust
response.bytes().await
```

再判断大小。

要求：

- 先检查 `Content-Length`；
- 使用 `bytes_stream()` 分块读取；
- 累计超过 2MB 立即停止；
- 返回明确 `RESPONSE_TOO_LARGE`；
- 不能造成大响应内存占用。

---

## P2-3：未配置模型时不得声称“当前配置已验证”

没有配置模型时：

- 不把硬编码模型标记为“当前配置”；
- 标记为“推测模型”；
- 成功状态使用 `MODEL_GUESS_OK`；
- 前端说明不能代表当前配置完整可用；
- 优先使用模型列表或用户临时选择。

---

## P2-4：完善 Gemini 认证和测试

需要实现或明确支持：

- `x-goog-api-key` Header；
- Query `?key=` 变体；
- `/v1beta` 去重；
- `/v1` Base 兼容；
- Streaming `alt=sse`；
- Query Key 缓存隔离。

删除永远通过的测试：

```rust
assert!(condition || true)
```

任何测试不得包含绕过断言的 `|| true`。

---

# 5. UI 行为锁定矩阵

| 行为                 | v0.1.4 期望                                             |
| -------------------- | ------------------------------------------------------- |
| 默认应用筛选         | Claude                                                  |
| 默认 Provider 行勾选 | 无                                                      |
| 开始诊断按钮         | 无勾选时禁用                                            |
| 并发选择             | 显示 1/2/3，默认 1                                      |
| 模式说明             | 短说明 + Tooltip 保留                                   |
| Provider 列表        | v13/v15 兼容指纹下可显示                                |
| 核心应用标签         | 即使无数据也显示                                        |
| 三点菜单             | 点击外部/Esc/执行项自动关闭                             |
| 刷新                 | 清空行选择，筛选回 Claude                               |
| 选择 DB              | 清空行选择，筛选回 Claude                               |
| 运行中               | 禁止刷新、换 DB、改变选择与模式                         |
| 取消                 | 等当前 Run 完成收尾后才能再次启动                       |
| 结果状态             | 优先 Key/权限/额度/WAF，不轻易显示 UNSUPPORTED_PROTOCOL |

---

# 6. 回归测试矩阵（强制）

## 6.1 前端测试

必须覆盖：

1. 默认 Claude 标签 active；
2. 全部标签非默认 active；
3. Provider 行默认未勾选；
4. 开始诊断默认禁用；
5. 勾选后启用；
6. 刷新后清空勾选；
7. 刷新后仍默认 Claude；
8. 选择 DB 后行为相同；
9. Provider 为空时核心标签仍存在；
10. 三点菜单外部点击关闭；
11. Esc 关闭；
12. 菜单项执行后关闭；
13. 运行中不允许改变并发、模式、选择；
14. 旧 Run 事件不影响新 Run；
15. Key 不出现在 DOM；
16. ResultCard 显示实际错误证据。

## 6.2 Rust 测试

必须覆盖：

- v13 精确指纹；
- v15 精确指纹；
- 未知 Schema 阻断；
- 200 业务错误分类；
- WAF HTML 分类；
- 异步 single-flight；
- Leader 取消唤醒 Waiter；
- 每 Provider 请求预算；
- Host 30 次预算；
- 两次限流停止；
- URL Path Key 脱敏；
- API Version Query 缓存隔离；
- Unicode 截断；
- 非流式 2MB 增量停止；
- Gemini Header/Query Key；
- 模型猜测状态。

## 6.3 安全门禁

必须继续通过：

- 禁止 `std::process::Command`；
- 禁止 Node `child_process`；
- 禁止 Shell 插件；
- 禁止业务数据写盘；
- 禁止读取 AI 登录目录；
- 禁止完整 Key 序列化；
- 禁止跨 Host 凭据转发；
- SQLite 只读前后 SHA-256 不变。

## 6.4 Windows 构建

必须通过：

```text
npm ci
npm run format:check
npm run lint
npm run typecheck
npm test
npm run build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm run security:verify
npm run tauri build -- --bundles nsis
```

---

# 7. 防止 AI 误改的提交策略

建议按以下小提交执行，不要做一个不可审查的大提交：

1. `fix(ui): restore provider list and default Claude filter`
2. `fix(ui): close provider actions menu on outside interaction`
3. `fix(schema): add exact CC Switch v13 compatibility fingerprint`
4. `fix(diagnostics): classify 200 business errors before protocol fallback`
5. `fix(runtime): make single-flight async and isolate cancelled runs`
6. `fix(security): redact URL path secrets and preserve cache semantics`
7. `fix(budget): enforce provider send limits and streamed body cap`
8. `test: add v0.1.4 regression and security coverage`
9. `release: prepare v0.1.4`

每次提交后运行相关测试。

不得：

- 把所有修复压成一次大规模重写；
- 修改与当前问题无关的组件；
- 更换前端框架；
- 更换状态管理库；
- 更换 HTTP 客户端；
- 更换 SQLite 库；
- 修改 Release 资产名称；
- 删除旧测试以让 CI 变绿。

---

# 8. 发布要求

版本统一更新为：

```text
0.1.4
```

必须一致：

- `package.json`
- `package-lock.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/tauri.conf.json`
- `compatibility/manifest.json`
- `CHANGELOG.md`

Release 资产：

```text
CC-Switch-Doctor-v0.1.4-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.4-Windows-x64-portable.zip
SHA256SUMS.txt
```

发布前确认：

- `main` 与 `v0.1.4` 指向同一修复提交；
- CI 全绿；
- Release Workflow 成功；
- 三个资产存在且大小大于 0；
- SHA256 文件名和资产完全一致；
- `git status --short` 为空；
- 当前仍为 unsigned 时，在 Release Notes 中明确 SmartScreen 提示。

---

# 9. 最终人工验收步骤

使用真实 CC Switch `user_version=13` 数据库执行：

1. 启动应用；
2. 顶部显示 Schema 兼容或已验证，不是空白；
3. 左侧固定显示：全部、Claude、Codex、Gemini 等标签；
4. 默认高亮 Claude；
5. Claude Provider 正常显示；
6. Provider 行默认未勾选；
7. 手动勾选一个 Provider 后开始诊断；
8. 并发可切换 1/2/3；
9. 三种模式说明仍显示；
10. 打开三点菜单，点击空白处自动关闭；
11. 测试无余额 Key，结果应显示额度不足而非 UNSUPPORTED_PROTOCOL；
12. 测试无效 Key，结果应显示鉴权失败；
13. 测试 WAF 页面，结果显示网关/WAF；
14. 取消诊断后，必须等收尾完成才能重新开始；
15. 刷新后 Provider 仍显示，筛选恢复 Claude，行选择清空；
16. 关闭应用，不留下业务配置、结果和日志文件。

---

# 10. 完成定义

只有同时满足以下条件才算完成：

- v0.1.3 新增的三个 UI 回归全部修复；
- v13 Provider 列表恢复；
- 默认筛选准确改为 Claude；
- Provider 行不再自动勾选；
- 三点菜单外部点击可关闭；
- `UNSUPPORTED_PROTOCOL` 不再掩盖明确 Key、权限、余额和 WAF 错误；
- 所有新增测试通过；
- 所有旧测试继续通过；
- 安全门禁未被削弱；
- Windows 构建和远程 Actions 全绿；
- v0.1.4 Release 资产发布完成；
- 未破坏此前已经验收通过的功能。

如果只修复了新问题，但造成任何已通过能力退化，则本任务视为失败，不得发布。
