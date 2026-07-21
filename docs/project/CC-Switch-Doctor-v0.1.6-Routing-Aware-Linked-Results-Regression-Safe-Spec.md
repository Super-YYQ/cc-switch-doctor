# CC Switch Doctor v0.1.6 路由感知、结果联动与解析准确性回归安全修复规范

> 目标版本：v0.1.6  
> 基线版本：当前 `main / v0.1.5`  
> 任务类型：定向缺陷修复、CC Switch 路由链验证、诊断证据分级、结果联动和窗口尺寸优化  
> 核心原则：**最小范围修改；冻结 v0.1.5 已通过功能；不得为了实现路由验证而修改、启动、停止或切换 CC Switch 的任何路由配置。**

---

## 0. 执行总原则

1. 基于当前 `main / v0.1.5` 增量修复，不重写应用。
2. 不得推倒 Provider 扫描、SQLite 只读、诊断 Planner、双栏 UI 和安全边界。
3. 每组修复独立提交、独立测试。
4. 后续修复不得破坏前面已经通过的功能。
5. 路由验证只使用已经运行的 CCS 本地 HTTP 路由。
6. 不启动、停止、重启或重新配置 CCS 路由。
7. 不写 `proxy_config`、Provider、设置、健康状态等 CC Switch 数据表。
8. 不调用 Provider 切换命令。
9. 不读取 `.claude`、`.codex`、Gemini、OpenCode 等登录或 Live 配置目录。
10. 不启动任何 AI CLI 或 Shell 子进程。
11. 路由链成功不得冒充非当前 Provider 已验证成功。
12. 所有路由请求计入独立请求预算。
13. 任何旧测试、安全门禁或 UI 回归失败，都不得发布 v0.1.6。

---

## 1. 冻结功能清单

必须保留：

- 默认应用筛选为 `Claude`。
- `全部 / Claude / Codex / Gemini / OpenCode` 始终显示。
- Provider 行默认不勾选。
- 三点菜单外部点击、Esc、执行菜单项后关闭。
- `user_version=13` 精确指纹兼容。
- 未知 Schema 不读取敏感字段、不发送请求。
- 快速验证、智能诊断、深度兼容含义不变。
- 并发 1 / 2 / 3 可见可改，默认智能诊断、并发 1。
- Run ID 隔离和取消收尾逻辑不退化。
- `billing_usage` / `usage` 不再误判额度不足。
- 非流式跨协议、Wrapper 和兼容字段解析保持有效。
- Provider 真实请求预算、Host 30 次预算保持有效。
- 非流式 2MB 增量读取保持有效。
- Gemini Header / Query Key 保持有效。
- 猜测模型不能冒充当前配置。
- ErrorEvidence 前端展示保持有效。
- URL Query/Path 脱敏不得退化。
- 调试日志默认折叠。
- 紧凑 UI 不得恢复为早期大卡片版本。

---

## 2. 当前审查结论

v0.1.5 仍存在：

1. 跨协议或宽松字段成功，可能被错误升级为 `CURRENT_CONFIG_OK`。
2. 未读取和验证 CCS 当前路由状态。
3. CCS 路由开启时，命令行可用与上游直连不一致，Doctor 无法正确解释。
4. Streaming 跨协议解析不完整。
5. Streaming 非 2xx 正文未受 2MB 限制。
6. `{"error": null}` 等成功结构可能被错误拦截。
7. 非典型 Path Key 仍需精确 Redactor 兜底。
8. Provider 与结果缺少双向定位。
9. 初始窗口稍大。
10. 两个真实可用 Provider 仍返回 `RESPONSE_FORMAT_MISMATCH / UNKNOWN_ERROR / STREAMING_UNSUPPORTED`。

---

# 3. P0：双通道诊断模型

新增：

```rust
enum DiagnosisChannel {
    DirectUpstream,
    CcsLocalRoute,
}
```

### 直连上游

```text
Doctor → Provider 原始 Base URL
```

用于判断原始 URL、Key、模型、认证、原生协议和响应格式。

### CCS 本地路由

```text
Doctor → 已运行的 CCS 本地路由 → CCS 当前目标/故障转移链 → 上游
```

用于验证用户命令行实际使用链路，包括 CCS 模型映射、请求转换、响应转换和故障转移。

两个通道必须拥有独立：

- Attempt；
- 缓存键；
- 请求预算；
- 结果状态；
- Evidence；
- UI 区域。

---

# 4. P0：只读读取 CCS 路由配置

兼容 Schema 下只读查询 `proxy_config`。

读取全局字段：

```text
proxy_enabled
listen_address
listen_port
enable_logging
```

读取应用字段：

```text
app_type
enabled
auto_failover_enabled
max_retries
streaming_first_byte_timeout
streaming_idle_timeout
non_streaming_timeout
```

表不存在或字段不兼容时：

- Provider 扫描不能失败；
- 显示“路由状态不可用”；
- 只执行直连诊断。

新增前端安全视图：

```rust
struct RoutingStatusView {
    config_detected: bool,
    global_enabled: bool,
    listen_address: Option<String>,
    listen_port: Option<u16>,
    health_reachable: bool,
    server_running: bool,
    apps: Vec<AppRoutingStatusView>,
    warning: Option<String>,
}
```

通过本地：

```text
GET /health
GET /status
```

确认服务实际运行，并读取 `active_targets` 与 `failover_count`。

安全规则：

- 只允许 loopback。
- `0.0.0.0` 映射到 `127.0.0.1`。
- `::` 映射到 `::1`。
- 非 loopback 地址默认禁止自动测试。
- 不携带 Provider 真实 Key。
- 禁止重定向。
- 探测超时 1～2 秒。

---

# 5. P0：路由请求模拟客户端协议，而不是上游协议

这是图1问题的根本修复。

例如：

```text
应用：Claude Code
Provider 上游协议：OpenAI Responses
CCS 路由：开启
```

Claude Code 实际向本地 CCS 发送 Anthropic Messages，由 CCS 转成 OpenAI Responses。

因此：

### Claude Code

```text
POST http://127.0.0.1:<port>/v1/messages
```

必须发送 Anthropic Messages 请求，不能因为上游是 Responses 就向本地路由发 `/v1/responses`。

### Codex

按当前 CCS `wire_api` 语义使用：

```text
/v1/responses
或
/v1/chat/completions
```

无法确认时优先 Responses，再按路由预算尝试 Chat。

### Gemini

```text
/v1beta/models/<client-model>:generateContent
```

其他应用只有在当前 CCS 上游明确提供稳定入口时才开放，禁止猜测。

---

# 6. P0：本地路由认证和模型语义

Doctor 不读取客户端 Live 配置。

必须根据当前 CC Switch 上游版本复现其接管占位凭据和客户端模型语义。当前基线使用无价值占位 Token：

```text
PROXY_MANAGED
```

实现前必须重新核对当前 CCS 默认分支，不得永久无版本约束硬编码。

要求：

- 路由请求绝不发送 Provider 真实 Key。
- 使用 CCS 当前版本认可的占位凭据。
- Claude 路由测试使用客户端可见模型/稳定角色别名，不盲目使用上游真实模型。
- 无法可靠确定客户端模型时，失败不得判为 Provider 不可用。
- 路由规则写入兼容 Manifest，未知 CCS 版本不静默测试。

---

# 7. 路由验证副作用边界

Doctor 可以保证：

- 不修改路由配置；
- 不启动/停止路由；
- 不主动切换 Provider；
- 不写 CC Switch DB。

但真实路由请求可能由 CCS 自身产生：

- 请求日志；
- 统计计数；
- 健康状态；
- 熔断器状态；
- 自动重试；
- 自动故障转移；
- active target 变化。

UI 必须明确提示：

> 本工具不会修改或切换 CCS 路由配置；但真实路由验证会被 CCS 视为一次正常请求，可能写入日志/统计，并可能触发已配置的重试或故障转移。

自动故障转移开启时，结果必须称为：

```text
当前 CCS 路由链验证
```

不能称为固定 Provider 验证。

---

# 8. 当前 Provider 与实际路由目标关联

只有满足：

```text
Provider.is_current = true
且
/status active_targets 对应 app 的 provider_id == Provider source_id
```

路由结果才能归属于该 Provider。

自动故障转移时，在请求前后读取 `/status`：

```text
expected_provider_id
actual_provider_id
actual_provider_name
failover_count_before
failover_count_after
```

不一致时：

```text
ROUTE_TARGET_MISMATCH
```

文案：

> CCS 路由请求成功，但实际由另一 Provider 处理；本结果验证的是当前路由链，不代表所选 Provider 已通过。

用户勾选多个 Provider 时：

- 非当前 Provider 继续直连；
- 路由通道显示“不适用：不是当前路由目标”；
- 不重复发送相同路由请求；
- 不计为失败。

---

# 9. P0：证据等级和状态重构

新增：

```rust
enum ResponseCompatibility {
    Native,
    CrossProtocol,
    LooseField,
}
```

结果携带：

```text
channel
response_compatibility
requested_protocol
matched_protocol
```

建议状态：

```text
DIRECT_NATIVE_OK
DIRECT_PROTOCOL_VARIANT_OK
DIRECT_LOOSE_TEXT_OK

CCS_ROUTE_OK
CCS_ROUTE_OK_DIRECT_NATIVE_OK
CCS_ROUTE_OK_DIRECT_VARIANT
CCS_ROUTE_OK_DIRECT_PARSE_FAILED

CCS_ROUTE_NOT_RUNNING
CCS_ROUTE_NOT_APPLICABLE
CCS_ROUTE_TARGET_MISMATCH
CCS_ROUTE_FAILED_DIRECT_OK
CCS_ROUTE_AND_DIRECT_FAILED
```

只有同时满足以下条件才能显示“当前配置可直接使用”：

- 直连通道；
- 目标协议 Native 成功；
- 模型不是 Doctor 猜测；
- 当前认证方式；
- 当前 URL。

CrossProtocol 不能设置 `current_ok=true`。

LooseField 只能显示：

```text
LOOSE_RESPONSE_TEXT_OK
```

不能升级为当前配置成功。

---

# 10. 图1：路由场景下错误提示“切换协议后可用”

场景：

```text
Claude Code
上游协议 OpenAI Responses
CCS 路由已开启
命令行正常
Doctor 提示切换协议后可用
```

正确结果：

```text
CCS_ROUTE_OK_DIRECT_VARIANT
当前 CCS 路由链可用
```

建议文案：

> 无需修改当前 CC Switch 配置。上游协议与 Claude Code 客户端协议不同，当前由 CCS 路由完成转换。

不得再建议用户调整一个已经正常工作的配置。

---

# 11. 缩小初始窗口

当前：

```text
1180 × 820
```

调整为：

```text
1100 × 740
```

保留：

```text
minWidth: 960
minHeight: 640
resizable: true
center: true
```

验收：

- 1100×740 默认启动不溢出；
- 960×640 可操作；
- 1366×768、1440×900 清晰；
- 不得通过隐藏核心按钮适配。

---

# 12. Provider 与结果双向联动

基于 `opaqueId` 建立联动。

### 点击 Provider 卡片

- 设置 `activeProviderId`；
- 右侧滚动到对应 ResultCard；
- ResultCard 高亮；
- 不改变 Checkbox。

### 点击左侧状态 Badge

直接跳转到对应结果。

### 点击 ResultCard

- 左侧滚动到对应 Provider；
- Provider 高亮；
- 不改变勾选状态。

### 结果索引

右侧顶部增加紧凑定位控件：

```text
当前结果：[Provider 名称 ▼]
上一条 / 下一条
```

要求：

- 可搜索 Provider；
- 显示状态色；
- 不增加第三个大面板；
- 结果默认按左侧 Provider 排序；
- 新结果到达不抢滚动焦点；
- 重新扫描 opaqueId 变化时清理旧 active ID。

测试：

```text
Provider → Result scrollIntoView
Result → Provider scrollIntoView
Checkbox 不触发跳转
查看详情不触发跳转
筛选后 active Provider 消失时安全清理
```

---

# 13. 图2、图3：命令行可用但返回格式不一致

真实样本：

```text
new.xkool.cfd/v1/messages
RESPONSE_FORMAT_MISMATCH 200
UNKNOWN_ERROR 200 /v1/chat/completions
STREAMING_UNSUPPORTED 200
```

```text
ark.cn-beijing.volces.com/api/coding/v1/messages
RESPONSE_FORMAT_MISMATCH 200
UNKNOWN_ERROR 200 /v1/chat/completions
STREAMING_UNSUPPORTED 200
```

对于 HTTP 200 解析失败，调试日志必须展示脱敏结构证据：

```text
Content-Type
Content-Length / 实际长度
顶层 JSON 类型
顶层字段名
choices 数量
content 类型
output 类型
finish/stop reason
reasoning_content 是否存在
tool_calls 是否存在
SSE / NDJSON / 完整 JSON
脱敏响应摘要（最大 1～2KB）
```

扩展解析：

### Anthropic

```text
content 数组
content 字符串
content[].text
content[].type=text
```

### OpenAI Chat

```text
choices[].message.content 字符串
choices[].message.content 文本 Part 数组
choices[].text
choices[].message.reasoning_content
choices[].message.tool_calls
```

### OpenAI Responses

```text
output_text
output[].content[].text
output[].content[].type=output_text
output[].type=message
```

### Wrapper

```text
data
result
response
payload
```

允许对已知 Wrapper 字段做一次受限 JSON 字符串解码：

```text
最大深度 2
最大长度 64KB
禁止无限递归
```

---

# 14. 空文本自适应复测

当：

- HTTP 200；
- 响应结构看似成功；
- 没有明确错误；
- 文本为空；
- stop reason 为长度限制，或存在 reasoning 内容；

允许一次受预算控制的复测：

```text
输出上限 32 → 128
```

记录：

```text
EMPTY_OUTPUT_RETRY
```

不得无限增加 Token。

若直连仍无法解析但 CCS 路由真实成功，整体结论必须是：

```text
当前 CCS 路由链可用
```

直连解析失败只能作为次级技术证据。

---

# 15. P0：Streaming 跨协议和完整缓冲

必须同时维护：

```rust
line_buffer
raw_bounded_buffer
```

`line_buffer` 用于拆行，`raw_bounded_buffer` 保存最多 2MB 完整响应供最终回退。

每个 SSE/NDJSON 事件依次尝试：

1. 当前协议流式解析器；
2. OpenAI Chat；
3. OpenAI Responses；
4. Anthropic；
5. Gemini；
6. 完整 JSON 跨协议解析。

跨协议流成功：

```text
STREAM_PROTOCOL_VARIANT_OK
```

不能算原生 `STREAM_OK`。

超过 2MB：

```text
RESPONSE_BODY_TOO_LARGE
```

不能显示 `STREAMING_UNSUPPORTED`。

---

# 16. Streaming 非 2xx 受限读取

不得使用：

```rust
response.text().await
```

必须：

- 检查 Content-Length；
- 增量读取最多 2MB；
- 传递 Content-Type；
- 返回 ErrorEvidence；
- 识别 HTML/WAF；
- 记录 Retry-After。

---

# 17. 错误 Envelope 修复

以下不能视为错误：

```json
{"error": null}
{"error": false}
{"error": ""}
{"error": {}}
{"error": []}
```

只有以下才是明确错误：

- 非空错误字符串；
- 包含 message/code/type 的非空对象；
- 明确错误数组；
- `success=false`；
- `ok=false`；
- `status=error/failed`。

有效成功响应优先，不允许非关键 `error` 字段覆盖成功内容。

---

# 18. 精确 Key 脱敏

通用 Path 外观判断只能做补充。

提取真实 Key 后必须注册 `SecretRedactor`，所有前端 URL 统一走：

```rust
sanitize_url_with_redactor()
```

覆盖：

- Provider safeBaseUrl；
- 直连和路由 Attempt URL；
- successUrl；
- suggestion；
- evidence；
- debug log；
- response excerpt；
- redirect Location；
- cache key debug。

测试：

```text
myPrivateToken
abcXYZSecret
短于 24 字符的 Path Key
不以 sk- 开头的 Token
```

---

# 19. 路由诊断 UI

顶部显示：

```text
CCS 路由：未开启
CCS 路由：已配置但未运行
CCS 路由：运行中
Claude：已接管
自动故障转移：开启
```

控制栏增加：

```text
验证方式：
自动
仅直连
直连 + CCS 路由
```

默认 `自动`：

- App 路由关闭：仅直连。
- App 路由开启且服务可达：直连 + 路由。
- 自动故障转移开启：本会话首次提示副作用。

同一结果卡片分为：

```text
实际使用链路（CCS 路由）
上游直连
```

路由结果优先，直连是技术细节。

---

# 20. 路由请求预算

每个 App 每次诊断会话最多：

```text
2 次真实路由请求
```

- 非流式最小验证 1 次；
- 深度模式可加 Streaming 1 次；
- 选择多个 Provider 不重复发送相同路由请求；
- 复用必须明确标记。

直连预算继续：

```text
Quick 2
Smart 12
Deep 16
Host 30
```

---

# 21. 路由 Fixture 和 Mock Server

新增：

```text
synthetic-routing-enabled.sql
```

包含：

```text
proxy_enabled=1
listen_address=127.0.0.1
listen_port=<mock>
claude enabled=1
auto_failover_enabled=0/1
current Provider
failover Provider
```

Mock CCS：

```text
GET /health
GET /status
POST /v1/messages
POST /v1/responses
```

测试场景：

1. 直连 Native 成功，路由成功。
2. 直连 CrossProtocol 成功，路由成功。
3. 直连格式不匹配，路由成功。
4. 路由不可达，直连成功。
5. 路由实际 Provider 与所选不同。
6. 自动故障转移计数变化。
7. Streaming 跨协议。
8. 路由只收到占位 Token，没有真实 Provider Key。

---

# 22. 回归测试矩阵

## Rust

```text
CrossProtocol 不设置 current_ok
LooseField 不设置 current_ok
error:null 不拦截成功
Streaming 完整 bounded buffer
Streaming 跨协议成功
Streaming 非 2xx 受 2MB 限制
路由配置只读
路由 health/status
仅允许 loopback
真实 Key 不发送到路由
路由占位 Token
路由目标匹配/不匹配
自动故障转移提示
路由预算
非典型 Path Key 脱敏
空文本增大输出后成功
```

## 前端

```text
默认 Claude 筛选
Provider 默认未勾选
核心筛选存在
三点菜单关闭
并发与模式说明
路由状态 Chip
Auto / Direct / Direct+Route
Provider 与 Result 双向定位
路由成功优先于直连格式不匹配
非当前 Provider 不显示路由成功
1100×740 无溢出
960×640 可操作
```

---

# 23. 推荐提交顺序

```text
1. fix(evidence): separate native, cross-protocol and loose success
2. fix(stream): cross-protocol streaming and bounded error bodies
3. feat(routing): read-only CCS route discovery and localhost health
4. feat(routing): verify current CCS route chain without changing config
5. fix(parser): response structure diagnostics and empty-output retry
6. feat(ui): link providers to results and reduce initial window
7. test(v0.1.6): routing/parser/evidence/UI regression matrix
8. release: prepare v0.1.6
```

---

# 24. 发布前验证

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

必须确认：

- DB SHA256 前后不变；
- `proxy_config` 无写操作；
- 无 process spawn；
- 无登录目录读取；
- 真实 Key 未发送到 localhost；
- 未启动、停止、切换 CCS；
- Actions 全绿；
- Tag 与 main 一致；
- Release 资产非零。

---

# 25. Release 资产

```text
CC-Switch-Doctor-v0.1.6-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.6-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release 必须披露：

```text
Unsigned build
Windows SmartScreen may warn
Route verification sends real minimal requests through an already-running CCS route
Doctor does not modify or start/stop the route
CCS may record logs/statistics and may perform its configured failover behavior
```

---

# 26. 最终汇报格式

完成后必须输出：

```text
1. 修复提交列表
2. 直连 / CCS 路由双通道说明
3. proxy_config 只读与无写入证明
4. 路由实际目标匹配测试
5. 自动故障转移副作用说明
6. 图1场景修复结果
7. new.xkool.cfd 类型样本解析结果
8. Volcengine coding 类型样本解析结果
9. Streaming 跨协议测试
10. Provider/Result 双向联动截图
11. 1100×740、960×640 截图
12. 本地测试结果
13. Actions 状态
14. v0.1.6 Tag SHA
15. Release 资产大小和 SHA-256
16. git status --short（必须为空）
```

---

# 27. 直接交给 AI 工具的指令

```text
严格阅读并执行仓库中的 CC-Switch-Doctor-v0.1.6-Routing-Aware-Linked-Results-Regression-Safe-Spec.md。

基于 main/v0.1.5 最小范围修复，不得推倒现有 UI、Provider 扫描、诊断 Planner、数据库只读和安全边界。

特别注意：
1. 区分直连上游和 CCS 本地路由两个通道；
2. 路由开启时按应用客户端协议测试，不按 Provider 上游协议测试；
3. 只读读取 proxy_config，只连接已经运行的 loopback 路由，不启动、停止、切换或修改 CCS；
4. 路由请求不得携带 Provider 真实 Key，只使用当前 CCS 版本认可的占位凭据；
5. 自动故障转移时结果属于当前路由链，不能冒充固定 Provider；
6. 路由成功、直连格式不匹配时，整体结论必须是“当前 CCS 路由链可用”；
7. CrossProtocol 和 LooseField 不得设置 CURRENT_CONFIG_OK；
8. 修复 Streaming 跨协议、完整 bounded buffer 和非 2xx 大响应；
9. 实现 Provider 与结果双向定位，默认窗口改为 1100×740；
10. 冻结 v0.1.5 已通过的默认 Claude、Provider 默认不勾选、菜单关闭、并发、模式说明、v13 和 Key 安全；
11. 每组修复独立提交并立即测试；
12. 全部旧测试、新测试、安全门禁、Windows 构建和远程 CI 成功后发布 v0.1.6；
13. CI 或 Release 失败时继续修复，不能提前结束。
```
