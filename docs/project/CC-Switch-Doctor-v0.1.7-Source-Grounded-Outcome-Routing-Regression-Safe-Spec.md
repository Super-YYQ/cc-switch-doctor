# CC Switch Doctor v0.1.7 源码对齐、结果语义与路由回归安全修复规范

> 目标版本：v0.1.7  
> 基线版本：`main / v0.1.6`  
> 已审查基线提交：`7f257d3c6ba692ffc5b3b8c100a01190ee7fd08d`  
> 任务类型：P0 热修复、诊断结果分层、CCS 路由校验修复、源码对齐和回归测试补强  
> 核心原则：**先研究成熟源码，再修改；不得继续凭经验猜测模型响应、CLI 协议、Agent 事件、CCS 路由模型别名或 UI/CSS。**

---

## 0. 本轮任务范围

本轮不是重新开发应用，也不是继续无边界增加兼容分支。

必须完成：

1. 修复 v0.1.6 中 `CCS_ROUTE_NOT_APPLICABLE`、`CCS_ROUTE_NOT_RUNNING` 等路由辅助状态覆盖真实直连结果的严重回归。
2. 将 Provider 主诊断结论、上游直连结果和 CCS 路由状态真正分层。
3. 只有真实发送过 CCS 路由业务请求时，路由结果才允许参与 Provider 主结果组合。
4. 修复 CCS 路由请求前后目标 Provider 状态校验、路由请求并发去重和能力级结果聚合。
5. 删除或替换与当前 CC Switch 上游不一致的旧模型别名和协议硬编码。
6. 对协议返回结构、Streaming、Tool Call、错误映射和 UI/CSS，强制先研究成熟 GitHub 源码并形成可审计记录。
7. 建立协议 Fixture Corpus，禁止继续依靠在线接口试错和随手追加字段。
8. 不得改坏 v0.1.6 已经通过的 Provider 筛选、菜单、双向定位、紧凑 UI、安全边界和请求预算。

---

# 1. 冻结功能清单

以下功能视为已通过，修改任何相关代码前都必须补回归测试。

## 1.1 Provider 与筛选

- 默认应用筛选为 `Claude`。
- `全部 / Claude / Codex / Gemini / OpenCode` 始终显示。
- Provider 行默认不自动勾选。
- 用户手动勾选后才能开始诊断。
- 三点菜单：
  - 点击外部关闭；
  - 按 `Esc` 关闭；
  - 执行菜单项后关闭。
- Provider 与 Result 双向定位继续有效。
- 上一条、下一条和结果定位器继续有效。
- `user_version=13` 精确指纹兼容继续有效。
- 未知 Schema 不读取敏感字段、不发送诊断请求。

## 1.2 控制栏

- 快速验证、智能诊断、深度兼容含义不变。
- 并发 `1 / 2 / 3` 可见可修改。
- 默认智能诊断、默认并发 1。
- 验证方式保留：
  - 自动；
  - 仅直连；
  - 直连 + CCS 路由。
- 默认窗口保持 1100×740，最小 960×640。
- 诊断运行中禁止刷新、换 DB、重复启动。
- Run ID 隔离和取消收尾逻辑不得退化。

## 1.3 安全边界

- CC Switch SQLite 必须严格只读。
- `proxy_config` 只能 SELECT。
- 不启动、停止、重启或修改 CCS 路由。
- 不主动切换 CCS Provider。
- 不启动 Claude Code、Codex、Gemini CLI、OpenCode 或 Shell。
- 不读取 `.claude`、`.codex`、Gemini、OpenCode 登录或 Live 配置目录。
- Provider 真实 Key 不得发送到 CCS localhost。
- Provider 真实 Key 不得进入前端、日志、缓存键、错误证据和剪贴板。
- 非 loopback 地址不得执行 CCS 路由验证。
- 禁止跨 Host 重定向携带凭据。
- Provider、Host、Route 请求预算继续生效。

---

# 2. 当前最新代码审查结论

当前 `main/v0.1.6` 的最新待办按严重程度如下。

## P0-1：路由“不适用”覆盖真实直连错误

### 实际现象

调试日志明确显示：

```text
NETWORK_UNREACHABLE
error sending request for url
```

但 Provider 主 Badge 和 ResultCard 标题显示：

```text
CCS 路由不适用
```

这会让用户误以为错误原因是 CCS 路由，而不是上游连接失败。

### 根因

当前结果合并逻辑在：

```text
route_not_applicable = true
direct_native_ok = false
direct_variant_ok = false
```

时，直接返回：

```text
CCS_ROUTE_NOT_APPLICABLE
```

合并函数没有接收、保留真实 `direct_status`，因此以下直连错误都可能被覆盖：

```text
NETWORK_UNREACHABLE
AUTH_INVALID
AUTH_PERMISSION_DENIED
QUOTA_EXHAUSTED
MODEL_NOT_FOUND
ENDPOINT_NOT_FOUND
TLS_ERROR
REQUEST_TIMEOUT
```

### 正确语义

```text
路由没有发送真实业务请求
→ Provider 主状态完全采用 direct_status
→ Route 状态只作为辅助元数据
```

正确展示：

```text
主状态：网络连接失败
上游直连：连接阶段失败，未收到 HTTP 响应
CCS 路由：未验证——该 Provider 不是当前 CCS 路由目标
```

错误展示：

```text
主状态：CCS 路由不适用
```

---

## P0-2：“仅直连”模式也可能被路由状态污染

`VerifyMode::DirectOnly` 当前通过 `RouteApplicability::Skip` 表示，Engine 又将所有 `Skip` 统一映射成：

```text
CCS_ROUTE_NOT_APPLICABLE
```

这会让用户明确选择“仅直连”后，仍看到“CCS 路由不适用”的主 Badge。

正确行为：

```text
route.disposition = NotRequested
route.attempted = false
primary = direct.status
```

“仅直连”不应该生成任何路由错误状态。

---

## P0-3：路由未运行也会覆盖直连结果

当前 `route_not_running` 可能直接返回：

```text
CCS_ROUTE_NOT_RUNNING
```

即使直连已经得到明确成功或明确失败。

正确行为：

```text
主状态：当前配置直接可用
辅助状态：CCS 路由已配置但未运行
```

或：

```text
主状态：网络连接失败
辅助状态：CCS 路由已配置但未运行
```

---

## P0-4：一个扁平 `status` 同时承担过多语义

当前 `ProviderDiagnosisSummary.status` 同时表达：

- Provider 上游直连结论；
- CCS 路由是否配置；
- CCS 路由是否运行；
- Provider 是否是当前路由目标；
- 路由业务请求是否成功；
- 直连与路由组合结果。

这种结构天然会让辅助状态覆盖真正结果。

必须重构为：

```rust
struct ProviderDiagnosisSummary {
    primary_outcome: PrimaryOutcome,
    direct: DirectChannelSummary,
    route: RouteChannelSummary,
    // 旧 status 可在一个版本内保留兼容，但必须由 primary_outcome 派生
}
```

```rust
struct DirectChannelSummary {
    attempted: bool,
    status: String,
    success: bool,
    native_success: bool,
    best_attempt_index: Option<usize>,
}
```

```rust
struct RouteChannelSummary {
    disposition: RouteDisposition,
    attempted: bool,
    generate: Option<CapabilityOutcome>,
    streaming: Option<CapabilityOutcome>,
    overall_status: Option<String>,
    actual_provider_id: Option<String>,
    actual_provider_name: Option<String>,
    failover_count_before: Option<u64>,
    failover_count_after: Option<u64>,
    notice: Option<String>,
}
```

```rust
enum RouteDisposition {
    NotRequested,
    NotConfigured,
    NotRunning,
    NotCurrentTarget,
    UnsupportedApp,
    BlockedNonLoopback,
    Attempted,
}
```

### PrimaryOutcome 规则

1. 路由真实发送且成功：根据路由与直连的真实结果组合。
2. 路由真实发送且失败：根据两个通道真实结果组合。
3. 路由没有真实发送：主结果必须等于 `direct.status`。
4. `NotRequested / NotRunning / NotCurrentTarget / UnsupportedApp / BlockedNonLoopback` 永远不是 Provider 主状态。
5. Managed Auth 跳过继续保持专门状态。

---

# 3. P0 热修复最低实现

必须计算：

```rust
let route_attempted = attempts.iter().any(|attempt| {
    attempt.channel == DiagnosisChannel::CcsLocalRoute
        && attempt.http_sent
});
```

临时兼容逻辑至少应为：

```rust
let primary_status = if route_attempted {
    combine_attempted_route_and_direct(...)
} else {
    direct_status.clone()
};
```

注意：

- `combine_attempted_route_and_direct()` 不再接收 `route_not_applicable`、`route_not_running` 作为主结果参数。
- 这些值应转换为 `RouteDisposition`。
- 不能只改一行 `if`，却继续让 UI 用 `route_status` 覆盖 `primary_outcome`。

---

# 4. 用户当前案例的强制回归用例

输入：

```text
Provider 不是当前 CCS 路由目标
直连 /v1/messages → NETWORK_UNREACHABLE
直连 /v1/chat/completions → NETWORK_UNREACHABLE
CCS 路由未发送真实业务请求
```

期望：

```text
primary_outcome = NETWORK_UNREACHABLE
direct.status = NETWORK_UNREACHABLE
route.disposition = NotCurrentTarget
route.attempted = false
```

UI：

```text
主 Badge：网络连接失败
```

```text
CCS 路由
未验证
原因：该 Provider 不是当前 CCS 路由目标
```

调试日志保留真实连接错误，不得显示路由不适用为主因。

---

# 5. P1：路由目标校验使用旧快照

v0.1.6 在诊断开始时读取一次 `/status`，路由请求结束后仍然使用旧 `RoutingStatusView` 判断实际目标。

如果 CCS 在请求过程中发生：

- 自动重试；
- 自动故障转移；
- active target 变化；

Doctor 无法准确发现。

## 修复要求

路由请求前读取：

```text
GET /status
```

记录：

```text
before.active_target
before.failover_count
```

业务请求完成后再次读取：

```text
GET /status
```

记录：

```text
after.active_target
after.failover_count
```

Route Evidence 必须展示：

```text
请求前目标
请求后目标
故障转移次数变化
实际处理 Provider
```

读取第二次 `/status` 失败时：

- 不能覆盖业务请求成功；
- 置信度降低；
- 显示“无法确认请求后实际路由目标”。

---

# 6. P1：Claude 路由模型别名使用旧硬编码

当前路由 Planner 硬编码：

```text
claude-sonnet-4-20250514
```

而当前 CC Switch 上游接管逻辑使用的是稳定角色别名体系，例如：

```text
claude-haiku-4-5
claude-sonnet-4-6
claude-opus-4-8
claude-fable-5
```

旧日期模型名可能导致：

- CCS 路由验证失败；
- 命令行可用但 Doctor 失败；
- 模型映射与真实 CCS 行为不一致。

## 修复要求

禁止继续在业务代码中散落硬编码模型别名。

新增版本绑定 Profile：

```rust
struct CcsClientProtocolProfile {
    cc_switch_commit: String,
    release_range: String,
    placeholder_token: String,
    claude_role_models: ClaudeRoleModels,
    codex_wire_protocols: Vec<ProtocolKind>,
    gemini_path_policy: String,
}
```

数据来源：

1. 当前 CC Switch 上游源码；
2. Compatibility Manifest；
3. 精确 Commit SHA；
4. 自动一致性测试。

未知 CCS 版本时：

```text
路由配置可见，但不执行真实路由业务请求
原因：当前 CCS 路由协议 Profile 尚未验证
```

不得猜测。

---

# 7. P1：每 App 路由请求去重不是原子操作

当前逻辑大致为：

```text
检查 HashSet 是否包含 App
发送路由请求
请求结束后插入 HashSet
```

并发 2/3 时，两个任务理论上可能同时通过检查并重复发送路由请求。

## 修复要求

使用异步、原子的 App Route Single-Flight：

```rust
enum RouteReservation {
    Leader,
    Waiter,
    AlreadyCompleted(RouteChannelSummary),
}
```

可使用：

```text
tokio::sync::Mutex
tokio::sync::watch
tokio::sync::oneshot
```

禁止使用阻塞式 `Condvar`。

同一 App、同一次 Run：

- 最多一个 Route Leader；
- 其他任务等待或复用；
- 非当前 Provider 不能因此显示路由成功；
- 复用的是 App 路由摘要，不是 Provider 成功结论。

---

# 8. P1：Deep 模式路由能力结果可能自相矛盾

Deep 模式可发送：

1. 非流式 Generate；
2. Streaming。

若非流式成功、Streaming 失败，当前代码可能出现：

```text
当前 CCS 路由链可用
route_status = STREAMING_UNSUPPORTED
```

## 修复要求

路由摘要按能力拆分：

```rust
struct RouteChannelSummary {
    generate: CapabilityOutcome,
    streaming: Option<CapabilityOutcome>,
    overall: CapabilityOutcome,
}
```

正确文案：

```text
基础推理：成功
流式输出：不支持
当前 CCS 路由链可用于非流式请求
```

可选能力失败不得覆盖基础推理成功。

---

# 9. P1：网络错误分类过于粗糙

当前大量 Reqwest 错误统一显示：

```text
NETWORK_UNREACHABLE
error sending request for url
```

对排查帮助有限。

## 修复要求

基于 Reqwest Error API 和脱敏后的 Source Chain，保守区分：

```text
REQUEST_TIMEOUT
CONNECT_FAILED
DNS_OR_CONNECT_FAILED
TLS_HANDSHAKE_FAILED
PROXY_CONNECT_FAILED
REQUEST_BUILD_FAILED
BODY_READ_FAILED
```

要求：

- 不能在无法证明时硬说 DNS 失败；
- Windows Schannel、系统代理、Clash 和不同 TLS 后端错误链可能不同；
- 主分类应保守；
- `technicalDetail` 可展示安全截断后的底层错误链；
- 所有 URL、Host、Key 必须经过 Redactor。

示例：

```text
主状态：连接上游失败
技术详情：连接阶段未建立，未收到 HTTP 响应
```

---

# 10. P1：缓存复用不应伪装成大量真实尝试

当前高级日志可能显示：

```text
#1 真实发送
#2 复用缓存
#3 真实发送
#4 复用缓存
#5 复用缓存
```

技术上正确，但普通用户容易理解为发送了 5 次请求。

## UI 要求

默认尝试链按 Canonical Request 分组：

```text
/v1/messages · Anthropic Messages
真实发送 1 次 · 缓存复用 2 次
最终状态：NETWORK_UNREACHABLE
```

高级日志可以继续显示每个 Planner Attempt。

顶部“真实请求数”只能统计：

```text
http_sent = true
```

---

# 11. 强制门禁：先研究 GitHub 成熟源码，禁止继续猜协议

AI 在修改 Parser、Streaming、Tool Call、路由模型、认证方式或 CSS 前，必须提交：

```text
docs/research/v0.1.7-source-review.md
```

该文档至少包含：

```text
仓库名称
Commit SHA / Tag
读取的具体文件
借鉴的实现
未采用的实现及原因
许可证
Doctor 中对应修改位置
```

没有该文档，不得开始修改 Parser 和 UI。

---

# 12. 第一优先级源码：CC Switch 上游

仓库：

```text
farion1231/cc-switch
```

必须重新读取当前默认分支，而不是依赖旧记忆。

重点文件：

```text
src-tauri/src/services/stream_check.rs
src-tauri/src/proxy/handlers.rs
src-tauri/src/proxy/response_processor.rs
src-tauri/src/proxy/handler_config.rs

src-tauri/src/proxy/providers/transform.rs
src-tauri/src/proxy/providers/transform_responses.rs
src-tauri/src/proxy/providers/streaming.rs
src-tauri/src/proxy/providers/streaming_responses.rs
src-tauri/src/proxy/providers/streaming_codex_chat.rs
src-tauri/src/proxy/providers/streaming_gemini.rs
src-tauri/src/proxy/providers/streaming_codex_anthropic.rs

src-tauri/src/services/proxy.rs
src-tauri/src/proxy/server.rs
src-tauri/src/proxy/types.rs
src-tauri/src/database/dao/proxy.rs
```

需要提取的事实：

- Anthropic ↔ OpenAI Chat 转换；
- Anthropic ↔ OpenAI Responses 转换；
- Anthropic ↔ Gemini 转换；
- Codex Responses 事件；
- SSE Event 类型；
- Content-Type 错标兼容；
- usage、reasoning、Tool Call、空 output；
- 路由接管模型别名；
- `PROXY_MANAGED` 占位认证；
- 本地路由路径；
- `/health`、`/status`；
- 自动故障转移与 active target。

Doctor 不应复制整个代理转换器，只应抽取：

- Response Shape 识别；
- 文本、Reasoning、Tool Call Evidence；
- SSE Event 识别；
- 完成条件；
- Error Envelope；
- 当前版本客户端模型别名。

直接复用代码时：

- 核对许可证；
- 保留 Attribution；
- 记录源文件和 Commit SHA；
- 只做诊断需要的最小裁剪。

---

# 13. 官方客户端与 SDK 源码

## 13.1 OpenAI Codex

仓库：

```text
openai/codex
```

重点目录：

```text
codex-rs/codex-api
codex-rs/codex-client
codex-rs/app-server
```

研究：

- Responses request；
- `ResponseEvent`；
- `response.completed`；
- `output: null`；
- 流断开和连接失败；
- API Error Mapping；
- Rate Limit；
- First Event / Idle Timeout；
- Tool Call 和 Response Item。

不得只凭几个 JSON 字段就声称“兼容 Codex”。

## 13.2 Anthropic TypeScript SDK

仓库：

```text
anthropics/anthropic-sdk-typescript
```

研究：

- Message；
- ContentBlock；
- TextBlock；
- ToolUseBlock；
- MessageStream；
- `content_block_start`；
- `content_block_delta`；
- `message_delta`；
- `message_stop`；
- `finalMessage()` 完成语义。

## 13.3 Gemini CLI

仓库：

```text
google-gemini/gemini-cli
```

研究：

- generateContent；
- streamGenerateContent；
- Candidate；
- Content Parts；
- FunctionCall；
- FinishReason；
- Safety Block；
- SSE / JSON Stream；
- Google API Error。

## 13.4 OpenCode

仓库：

```text
anomalyco/opencode
```

研究：

- Provider-agnostic Agent 通信；
- Client/Server Event；
- 多 Provider 接入；
- Tool Call 归一化；
- 流式 Part 状态机；
- 错误与 Session 状态分层。

Doctor 不调用 OpenCode，只借鉴协议归一化和事件建模。

---

# 14. 同类工具源码

## 14.1 TestModelAlive

仓库：

```text
MarvekG/TestModelAlive
```

可借鉴：

- 多客户端测试矩阵；
- 模型 Variant 管理；
- 任务进度；
- 结果模型；
- Tauri 桌面 UI；
- 用真实客户端作为 Ground Truth 的测试思想。

禁止照搬：

- 启动 CLI；
- 临时写 CLI 配置；
- 触碰登录态。

## 14.2 AI Key Manage

仓库：

```text
Yoan98/ai-key-manage
```

可借鉴：

- CC Switch 数据结构理解；
- Provider 分类；
- 批量选择；
- 高密度列表；
- 状态色和信息层级。

禁止照搬：

- SQL 导入导出工作流；
- LocalStorage 持久化；
- 保存 Key。

## 14.3 Model Tester

仓库：

```text
yuanzhi-yw/model-tester
```

可借鉴：

- OpenAI-compatible 请求；
- Tool Calling 测试；
- 错误展示；
- 结果卡片。

---

# 15. Parser 架构：Adapter Registry

禁止继续在一个大函数中无边界追加：

```text
if content
if answer
if response
if result
```

改为：

```rust
trait ResponseAdapter {
    fn protocol(&self) -> ProtocolKind;

    fn parse_non_stream(
        &self,
        envelope: &ResponseEnvelope,
    ) -> ParseOutcome;

    fn parse_stream_event(
        &self,
        event: &SseEvent,
        state: &mut StreamState,
    ) -> StreamParseOutcome;
}
```

Adapters：

```text
AnthropicAdapter
OpenAiChatAdapter
OpenAiResponsesAdapter
GeminiAdapter
```

每个 Adapter 独立包含：

- Native Parser；
- Explicit Variant Parser；
- Tool Call Parser；
- Reasoning Parser；
- Usage Parser；
- Error Parser；
- Streaming State Machine；
- Fixture Corpus。

Wrapper 和 LooseField 只能在最后一层运行，不能混入 Native Parser。

---

# 16. 协议 Fixture Corpus

新增：

```text
tests/fixtures/protocols/
├─ anthropic/
├─ openai-chat/
├─ openai-responses/
├─ gemini/
├─ ccs-route/
├─ wrappers/
├─ errors/
└─ malformed/
```

每个协议至少包含：

```text
标准非流式成功
标准流式成功
Tool Call
纯 Reasoning
空文本但合法完成
usage 缺失
usage 为 null
output 为 null
error 为 null
HTTP 200 业务错误
HTML/WAF
截断 SSE
NDJSON
跨协议响应
Wrapper
非标准 Content Part
```

用户真实案例脱敏后沉淀：

```text
elysiver-connect-failed.json
new-xkool-response-shape.json
volcengine-coding-response-shape.json
anthropic-billing-usage-success.json
```

严禁提交：

- 真实 Key；
- 完整带敏感参数的 URL；
- 用户隐私；
- 未脱敏响应。

---

# 17. Source-Grounded Parser 门禁

增加：

```text
npm run protocol:verify
```

或：

```text
cargo test protocol_fixture_corpus
```

门禁要求：

1. 所有 Fixture 必须得到确定结果。
2. Native Fixture 必须判为 Native。
3. Cross Protocol Fixture 不能判为 Native。
4. Loose Field Fixture 不能变成 Current Config OK。
5. HTTP 200 Error Fixture 不能误判成功。
6. 标准成功 Fixture 不能被关键词分类覆盖。
7. 新增 Parser 分支必须新增 Fixture。
8. 禁止出现：
   ```text
   || true
   unwrap_or(success)
   unknown → GENERATE_OK
   ```

---

# 18. CCS 路由 Profile 不能散落硬编码

Compatibility Manifest 增加：

```json
{
  "routingProfiles": [
    {
      "ccSwitchCommit": "...",
      "releaseRange": "...",
      "placeholderToken": "PROXY_MANAGED",
      "claudeClientModels": {
        "haiku": "...",
        "sonnet": "...",
        "opus": "...",
        "fable": "..."
      },
      "routes": {
        "claude": ["/v1/messages"],
        "codex": ["/v1/responses", "/v1/chat/completions"],
        "gemini": ["/v1beta/models/{model}:generateContent"]
      }
    }
  ]
}
```

运行时：

- 匹配已验证 Profile；
- 未知版本不发真实路由业务请求；
- 提示升级 Doctor；
- Upstream Watch 监控相关源文件 SHA。

---

# 19. UI/CSS 也必须参考成熟源码

第一参考仍是：

```text
farion1231/cc-switch
```

重点研究：

```text
src/index.css
Provider Card 组件
Proxy Panel
Status Badge
ScrollArea
DropdownMenu
Tooltip
ProviderCardLayout.test.ts
```

当前 CC Switch 前端技术栈包含 React、TypeScript、Tailwind CSS、shadcn/ui、Radix UI、TanStack Virtual、Lucide 等，Doctor 应优先使用相同风格与交互模式，而不是重新手搓不一致组件。

可借鉴：

- 当前 Provider / 路由 Provider 的边框语义；
- 紧凑卡片密度；
- Hover 操作；
- Radix Dropdown / Popover 外部点击关闭；
- ScrollArea；
- 大列表虚拟化；
- Badge Token；
- CSS 变量；
- 键盘焦点态。

禁止：

- 直接复制品牌 Logo 或受限资源；
- 重写整个 UI 框架；
- 引入大量依赖只使用一个小组件；
- UI 修改影响诊断后端；
- 未检查许可证直接复制源码。

Source Review 必须记录：

```text
参考组件
参考 CSS Token
Doctor 对应实现
是否复制代码
许可证与 Attribution
```

---

# 20. 路由状态 UI 修复

Provider 主 Badge：

- 只显示 `primary_outcome`。
- 不显示 `NotCurrentTarget`。
- 不显示 `NotRequested`。
- 不显示 `NotRunning` 作为 Provider 主错误。

Provider 卡片可增加中性小提示：

```text
路由未验证
```

Tooltip：

```text
该 Provider 不是当前 CCS 路由目标
```

ResultCard：

```text
诊断结论
网络连接失败

上游直连
连接阶段失败，未收到 HTTP 响应

CCS 路由
未执行
原因：不是当前路由目标
```

---

# 21. 状态优先级

## 21.1 路由未执行

```text
primary = direct.status
```

## 21.2 路由成功

```text
route success + direct native success
→ CCS_ROUTE_OK_DIRECT_NATIVE_OK

route success + direct variant
→ CCS_ROUTE_OK_DIRECT_VARIANT

route success + direct parse/network failure
→ CCS_ROUTE_OK_DIRECT_FAILED
```

## 21.3 路由失败

```text
route failed + direct success
→ CCS_ROUTE_FAILED_DIRECT_OK

route failed + direct failed
→ CCS_ROUTE_AND_DIRECT_FAILED
```

## 21.4 永远不是 PrimaryOutcome 的 RouteDisposition

```text
NotRequested
NotConfigured
NotRunning
NotCurrentTarget
UnsupportedApp
BlockedNonLoopback
```

---

# 22. 回归测试矩阵

## 22.1 P0 状态组合

必须新增：

```text
NotCurrentTarget + NETWORK_UNREACHABLE
→ primary=NETWORK_UNREACHABLE
→ route.disposition=NotCurrentTarget

NotCurrentTarget + AUTH_INVALID
→ primary=AUTH_INVALID

NotCurrentTarget + CURRENT_CONFIG_OK
→ primary=CURRENT_CONFIG_OK

DirectOnly + direct failure
→ primary=direct failure
→ route.disposition=NotRequested

NotRunning + direct success
→ primary=direct success
→ route.disposition=NotRunning

NotRunning + direct failure
→ primary=direct failure
→ route.disposition=NotRunning

BlockedNonLoopback + direct failure
→ primary=direct failure

只有 route.attempted=true
→ 才允许组合 Route 与 Direct
```

## 22.2 Route 状态刷新

```text
before provider=A
route request
after provider=B
→ ROUTE_TARGET_MISMATCH

failover_count 2 → 3
→ Evidence 显示发生故障转移

/status after 请求失败
→ Route 业务结果保留
→ confidence 降低
```

## 22.3 模型别名

```text
已验证 CCS Profile
→ 使用 Profile 中 Alias

未知 CCS Profile
→ 不发送 Route 业务请求

源码中不得再散落旧日期模型名
```

## 22.4 Route Single-Flight

```text
并发三个同 App 任务
→ 只有一个真实 Route Leader
→ 其余等待或复用
```

## 22.5 UI

```text
直连 NETWORK_UNREACHABLE 时主 Badge 不能是 CCS_ROUTE_NOT_APPLICABLE
非当前 Provider 只显示中性 Route Hint
ResultCard 分别展示 Primary / Direct / Route
缓存复用默认折叠汇总
默认 Claude 筛选仍有效
Provider 默认未勾选
三点菜单不退化
双向定位不退化
```

---

# 23. 推荐提交顺序

## Commit 1

```text
fix(v0.1.7): keep route disposition from overriding direct outcome
```

只修 P0 主状态。

## Commit 2

```text
refactor(outcome): split primary, direct and route summaries
```

只重构结果数据，不改协议 Parser。

## Commit 3

```text
fix(routing): refresh status and make per-app route probe single-flight
```

修复快照和并发去重。

## Commit 4

```text
research: document upstream protocol and UI source review
```

提交源码调研文档。

## Commit 5

```text
fix(routing): source client aliases and routes from compatibility profiles
```

删除旧硬编码。

## Commit 6

```text
refactor(protocol): introduce source-grounded adapter registry and fixtures
```

仅在调研完成后执行。

## Commit 7

```text
fix(ui): separate primary outcome from route disposition
```

只改结果展示与缓存折叠。

## Commit 8

```text
test(v0.1.7): add status matrix, route and protocol corpus regressions
```

补齐测试。

## Commit 9

```text
release: prepare v0.1.7
```

仅版本、CHANGELOG、Release Notes。

---

# 24. 禁止事项

- 禁止继续在 Parser 中无依据追加字段名。
- 禁止以“常见接口大概如此”作为依据。
- 禁止引用博客二手代码代替官方或上游源码。
- 禁止用 CLI 运行成功为理由突破 Doctor 安全边界。
- 禁止调用 CLI、写临时登录配置、复制用户 Token。
- 禁止把 RouteDisposition 当作 Provider 主错误。
- 禁止把 CrossProtocol / LooseField 当成 Native。
- 禁止硬编码未绑定 CCS 版本的模型别名。
- 禁止大范围重写 UI。
- 禁止删除旧回归测试来让 CI 通过。
- 禁止 CI 未绿时发布。

---

# 25. 发布前强制验证

```bash
npm ci
npm run format:check
npm run lint
npm run typecheck
npm test
npm run build
npm run security:verify

cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --bundles nsis
```

额外门禁：

```text
docs/research/v0.1.7-source-review.md 存在
Protocol Fixture Corpus 全通过
Route 状态组合矩阵全通过
没有真实 Key 出现在序列化快照
proxy_config 与 DB SHA256 前后不变
无 Process Spawn
无保护目录读取
```

---

# 26. Release 资产

```text
CC-Switch-Doctor-v0.1.7-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.7-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release 继续说明：

```text
Unsigned build
Windows SmartScreen may warn
CCS route verification can generate normal CCS logs/statistics
Doctor never starts/stops/switches or modifies CCS routing
```

---

# 27. 最终汇报格式

完成后只输出：

```text
1. 修复提交列表
2. P0 状态覆盖 Bug 的根因与修复
3. Primary / Direct / Route 新数据模型
4. Source Review 文档和参考仓库/Commit
5. CC Switch 模型别名来源与一致性测试
6. Route /status 前后快照验证
7. Route Single-Flight 测试
8. 协议 Fixture Corpus 测试
9. UI 状态修复截图
10. 本地测试结果
11. GitHub Actions 状态
12. v0.1.7 Tag SHA
13. Release 资产大小与 SHA-256
14. git status --short（必须为空）
```

---

# 28. 直接交给 AI 工具的执行指令

```text
严格阅读并执行仓库中的 CC-Switch-Doctor-v0.1.7-Source-Grounded-Outcome-Routing-Regression-Safe-Spec.md。

基于 main/v0.1.6 做回归安全修复，不得推倒现有 Provider 扫描、数据库只读、安全边界、诊断 Planner 和双栏 UI。

第一优先级修复：
- CCS_ROUTE_NOT_APPLICABLE、CCS_ROUTE_NOT_RUNNING、DirectOnly Skip 等路由辅助状态不能覆盖 NETWORK_UNREACHABLE、AUTH_INVALID、QUOTA_EXHAUSTED 等真实直连结果。
- 只有真实发送过 CCS 路由业务请求时，路由结果才允许参与 Provider 主状态组合。
- 未执行路由时，主状态必须完全等于 direct_status。

本轮禁止继续凭经验猜测模型返回结构、Streaming 事件、CLI 客户端协议、CCS 模型别名或 CSS。
修改 Parser、Route Client Profile 或 UI 前，必须先阅读文档列出的成熟 GitHub 源码，并提交 docs/research/v0.1.7-source-review.md，记录仓库、Commit SHA、源文件、借鉴内容、许可证和未采用内容。

特别核对：
- farion1231/cc-switch 的 stream_check、proxy handlers、response_processor、transform、streaming、proxy takeover、server、types 和前端 Provider/CSS 实现；
- openai/codex 的 codex-api、codex-client 和 app-server；
- anthropics/anthropic-sdk-typescript；
- google-gemini/gemini-cli；
- anomalyco/opencode；
- MarvekG/TestModelAlive；
- Yoan98/ai-key-manage；
- yuanzhi-yw/model-tester。

不得调用任何 AI CLI，不得读取其登录目录，不得写入 CC Switch 配置。

按照小提交顺序执行，每组修改立即运行相关测试。全部旧测试、新测试、安全门禁、Windows 构建和远程 CI 成功后发布 v0.1.7；CI 或 Release 失败时继续修复，不能提前结束。
```
