# CC Switch Doctor v0.1.5 回归安全诊断与紧凑 UI 修复规范

> 目标版本：v0.1.5  
> 基线版本：当前 `main / v0.1.4`  
> 任务类型：定向缺陷修复、诊断准确性修复、安全补强、紧凑 UI 优化  
> 核心原则：**最小范围修改，不得推倒现有架构，不得修复后面的功能时破坏前面已经通过的功能。**

---

## 0. 执行总原则

本任务不是重新设计产品，也不是大规模重构。

AI 开发工具必须遵守：

1. 基于当前 `main / v0.1.4` 做增量修复。
2. 每一组修复独立提交，独立运行测试。
3. 不得修改已经通过的页面信息架构、数据库只读策略、CLI 隔离策略和同源安全策略。
4. 不得为了“代码更漂亮”重写 Provider 扫描、诊断引擎或 UI 主框架。
5. 新增修复必须伴随回归测试，不能只手工验证。
6. 任何旧测试失败、旧功能退化或安全门禁失败，都不允许发布 v0.1.5。
7. 不得删除、降低或绕过现有安全测试。
8. 不得自动修改 CC Switch 配置。
9. 不得启动 Codex、Claude Code、OpenCode、Gemini CLI、CC Switch 或任何 Shell 子进程。
10. 不得读取 `.codex`、`.claude`、OpenCode、Gemini 登录目录。

---

# 1. 必须冻结、不得改坏的既有功能

以下项目在 v0.1.4 已经通过，必须视为冻结功能：

## 1.1 左侧 Provider 区域

- 默认应用筛选为 `Claude`。
- `全部 / Claude / Codex / Gemini / OpenCode` 核心筛选始终存在。
- Provider 行默认不自动勾选。
- 用户手动勾选 Provider 后才能开始诊断。
- `选择 CCS 当前配置` 只能作为三点菜单中的显式操作。
- `user_version=13` 精确指纹兼容时，Provider 能正常读取并展示。
- 未知 Schema 必须停止读取敏感字段和测试。
- 刷新配置后：
  - 筛选恢复为 Claude；
  - Provider 勾选清空；
  - 旧结果清空；
  - 旧 Run 状态清空。

## 1.2 三点菜单

- 点击菜单外部自动关闭。
- 按 `Esc` 自动关闭。
- 点击菜单项后自动关闭。
- 组件卸载时清理事件监听。
- 不允许再次出现菜单悬浮后无法关闭的问题。

## 1.3 诊断控制栏

- 快速验证、智能诊断、深度兼容三种模式保持现有含义。
- 并发 1 / 2 / 3 可见且可修改。
- 默认智能诊断。
- 默认并发 1。
- 诊断中禁止刷新、换 DB、重复启动。
- 取消后必须等待匹配的 `run_finished` 完成收尾。

## 1.4 安全边界

- CC Switch DB 只读打开。
- 不写 DB。
- 不启动 AI CLI。
- 不读取登录目录。
- 完整 API Key 不进入前端、日志、剪贴板和诊断摘要。
- 禁止跨 Host 重定向携带凭据。
- 同一 Host 会话级最多 30 次真实请求。
- 两次连续限流后停止该 Host。
- Actions 使用固定 Commit SHA。
- Windows Release 仍为 unsigned，并明确 SmartScreen 风险。

---

# 2. P0：成功响应被错误判定为额度不足

## 2.1 用户实际复现

配置在命令行中可正常使用，Doctor 返回：

```text
QUOTA_EXHAUSTED 200
```

实际响应：

```json
{
  "id": "f9bbb78d-17ae-94fb-8230-b4c6ad4c0f4f",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "CCS_DOCTOR_OK"
    }
  ],
  "stop_reason": "end_turn",
  "model": "grok-4.5-build-free",
  "usage": {
    "input_tokens": 206,
    "output_tokens": 6,
    "billing_usage": {
      "source": "oai_chat",
      "semantic": "openai"
    }
  }
}
```

这是明确的成功响应：

- HTTP 200；
- Anthropic Message 结构有效；
- `content[].text` 包含 `CCS_DOCTOR_OK`；
- 有正常 token usage；
- 无错误对象。

Doctor 误判原因很可能是错误分类器在正常成功 JSON 中匹配到：

```text
billing
billing_usage
```

然后错误返回 `QUOTA_EXHAUSTED`。

## 2.2 强制判定顺序

必须改为：

```text
1. 安全检查
2. HTTP 状态处理
3. 按目标协议解析成功响应
4. 兼容协议解析
5. 成功标记检测
6. 只有成功解析全部失败后，才运行错误启发式分类
```

**正常协议成功解析具有最高优先级。**

以下条件成立时，绝不能再运行余额、Key、WAF 关键词分类并覆盖成功结果：

- Anthropic `content[].text` 存在；
- OpenAI Chat `choices[].message.content` 存在；
- OpenAI Responses `output_text` 或有效 output item 存在；
- Gemini `candidates[].content.parts[].text` 存在；
- Tool Call 结构有效；
- 流式响应解析到有效文本或 Tool Call；
- 文本包含 `CCS_DOCTOR_OK`。

## 2.3 删除过宽关键词

不得使用单独的：

```text
billing
usage
credit
payment
```

作为额度不足证据。

`billing` 只能在明确负面语境下判定，例如：

```text
billing error
billing disabled
payment required
insufficient balance
quota exhausted
credits exhausted
余额不足
额度不足
欠费
无可用额度
```

`billing_usage`、`usage`、`token_usage`、`openai_usage` 是正常计量字段，不是错误证据。

## 2.4 结构化错误优先

对于 HTTP 2xx，只有检测到明确错误 Envelope 时，才在成功解析前提前判错，例如：

```json
{"error": {...}}
{"success": false, "message": "..."}
{"ok": false, "code": "..."}
{"status": "error", "message": "..."}
```

不得因为正常 JSON 中出现某个模糊单词就判错。

## 2.5 必须新增测试

必须使用与用户样本等价的 Fixture：

```rust
#[test]
fn anthropic_success_with_billing_usage_is_not_quota_error()
```

断言：

```text
classification = GENERATE_OK
ok = true
extracted_text = CCS_DOCTOR_OK
error_evidence = []
```

额外测试：

```text
200 + 正常 content + billing_usage
→ GENERATE_OK

200 + 正常 choices + usage
→ GENERATE_OK

200 + {"success":false,"message":"余额不足"}
→ QUOTA_EXHAUSTED

200 + {"error":{"message":"insufficient balance"}}
→ QUOTA_EXHAUSTED
```

---

# 3. P0：命令行可用，但 Doctor 无法解析返回格式

## 3.1 用户实际复现

命令行可正常使用，但 Doctor 输出：

```text
#1 RESPONSE_FORMAT_MISMATCH 200 https://new.xkool.cfd/v1/messages
HTTP 成功但无法按协议解析文本

#3 UNKNOWN_ERROR 200 https://new.xkool.cfd/v1/chat/completions
响应结构成功但无文本

#6 STREAMING_UNSUPPORTED 200 https://new.xkool.cfd/v1/messages
流式响应未解析到文本增量
```

还存在完全相同 URL、状态、延迟重复出现的情况。

## 3.2 解析策略必须分层

HTTP 2xx 后，采用以下顺序：

### 第一层：目标协议原生解析

- Anthropic Messages
- OpenAI Chat Completions
- OpenAI Responses
- Gemini Native

### 第二层：兼容协议解析

中转站可能在 `/v1/messages` 返回 OpenAI 结构，或者在 `/chat/completions` 返回 Anthropic 结构。

当原生解析失败时，按安全顺序尝试其他已知成功结构：

```text
Anthropic → OpenAI Chat → OpenAI Responses → Gemini
OpenAI Chat → OpenAI Responses → Anthropic → Gemini
OpenAI Responses → OpenAI Chat → Anthropic → Gemini
Gemini → OpenAI Chat → Anthropic → OpenAI Responses
```

命中后必须记录：

```text
目标协议：Anthropic Messages
实际返回结构：OpenAI Chat Completions
状态：RESPONSE_PROTOCOL_VARIANT_OK
```

不能直接返回 `RESPONSE_FORMAT_MISMATCH`。

### 第三层：已知 Wrapper 解析

支持常见包装层：

```json
{"data": {...}}
{"result": {...}}
{"response": {...}}
{"message": {...}}
{"payload": {...}}
```

只允许有限白名单路径，不得无边界递归扫描任意 JSON。

### 第四层：已知文本字段

安全检查以下已知字段：

```text
text
content
message
output_text
response
answer
result
data
```

但必须限制：

- 最大递归深度；
- 最大节点数量；
- 最大字符串长度；
- 不遍历 Key、Token、Header 等敏感字段。

找到有效文本但不包含成功标记时：

```text
PARTIAL_TEXT
```

不能返回 `UNKNOWN_ERROR`。

## 3.3 Streaming 兼容

流式接口必须兼容：

- 标准 SSE：`data: {...}`
- `event:` + `data:`
- OpenAI Chat delta
- OpenAI Responses event
- Anthropic `content_block_delta`
- Gemini SSE JSON
- JSON Lines / NDJSON
- 上游忽略 `stream=true`，直接返回完整 JSON

如果未解析到 SSE 增量，但收到了完整合法 JSON，必须回退到非流式解析器。

只有以下条件全部成立，才能返回 `STREAMING_UNSUPPORTED`：

- HTTP 成功；
- 未解析到任何 SSE / NDJSON 文本；
- 完整缓冲区也无法解析为已知协议；
- 无错误 Envelope；
- 无 WAF/HTML；
- 无有效文本字段。

## 3.4 重复尝试去重

用户日志中同一 URL、同一状态、同一延迟重复出现。

必须区分：

- 真正发送的新请求；
- 缓存复用；
- 认证方式变体；
- Token 字段回退；
- 同一 Planner 重复计划。

要求：

1. 完全相同的 BuiltRequest 只发送一次。
2. 缓存复用必须标记：
   ```text
   [复用缓存]
   ```
3. 相同请求不能在尝试链中伪装成多次真实发送。
4. Planner 输出后先执行稳定去重。
5. `attempt_started` 只对真实发送或明确变体展示。
6. UI 的“真实请求数”不得把缓存复用算进去。

## 3.5 必须新增测试

至少覆盖：

```text
Anthropic endpoint 返回 OpenAI Chat JSON
→ RESPONSE_PROTOCOL_VARIANT_OK

OpenAI endpoint 返回 Anthropic JSON
→ RESPONSE_PROTOCOL_VARIANT_OK

200 + wrapper.data.content
→ 能提取文本

stream=true 但返回完整 JSON
→ 使用完整 JSON 解析成功

SSE 无 data 前缀但为 NDJSON
→ 能解析文本

相同 BuiltRequest 两次
→ 只真实发送一次，第二次 reusedFromCache=true
```

---

# 4. P0：错误分类必须由真实证据驱动

## 4.1 真实成功优先于错误猜测

最终优先级：

```text
有效成功响应
> 明确结构化错误
> HTTP 状态
> 精确错误关键词
> WAF/HTML
> 返回格式不匹配
> 协议不兼容
> 未知错误
```

不得再出现：

```text
成功正文包含 billing_usage
→ QUOTA_EXHAUSTED
```

## 4.2 `UNSUPPORTED_PROTOCOL` 使用条件

只有在以下条件全部成立时，才允许最终返回：

```text
UNSUPPORTED_PROTOCOL
```

- 已尝试计划中的所有必要协议；
- 每个协议都真实发送或明确复用；
- 没有成功响应；
- 没有 Key、权限、额度、限流、模型、WAF、网络、TLS 证据；
- 返回内容确实与所有支持协议结构不兼容；
- 不是因为解析器缺少兼容格式；
- 不是因为猜测模型错误；
- 不是因为响应体过大或被截断。

单次解析失败应优先使用：

```text
RESPONSE_FORMAT_MISMATCH
```

而不是立即升级成 `UNSUPPORTED_PROTOCOL`。

## 4.3 ErrorEvidence 必须完整接入前端

Rust 已存在 `ErrorEvidence`，必须补齐 TypeScript 类型：

```ts
export interface ErrorEvidence {
  source: string;
  code?: string | null;
  message?: string | null;
  matchedKeyword?: string | null;
}
```

`AttemptResult` 增加：

```ts
errorEvidence: ErrorEvidence[];
```

右侧结果卡片增加“判定依据”：

```text
判定依据
- HTTP 状态：402
- 响应字段：success=false
- 命中关键词：余额不足
```

不得只展示固定模板“可能原因”。

---

# 5. P0：URL Path 中的 Key 必须彻底脱敏

当前 Query 参数值已做遮盖，但 URL Path 仍可能包含完整 Key：

```text
https://api.example.com/proxy/sk-real-secret/v1
```

必须统一使用注册了完整 Provider Key 的 `SecretRedactor` 清洗所有 URL：

- Provider 卡片 Base URL
- Attempt URL
- `attempt_started` 事件 URL
- `attempt_finished` URL
- 成功组合 URL
- 建议文本 URL
- 调试日志 URL
- 复制摘要 URL
- 缓存键 canonical URL
- 重定向 Location
- 错误信息中的 URL

要求：

```text
完整 Key 永不进入前端序列化对象。
完整 Key 永不进入 Debug 输出。
完整 Key 永不进入缓存键。
```

新增测试：

```text
Key 位于 Query
Key 位于 Path
Key 位于 userinfo
Key 位于错误正文
Key 位于 redirect Location
```

全部断言前端 JSON 中不存在完整 Key。

---

# 6. P1：Content-Type 必须进入分类器

读取 Response Headers 后保存：

```rust
Content-Type
Content-Length
Retry-After
```

所有错误分类调用必须传入真实 `Content-Type`。

例如：

```text
HTTP 200 + text/html
→ GATEWAY_OR_WAF

HTTP 403 + text/html
→ GATEWAY_OR_WAF 或 AUTH_PERMISSION_DENIED，并附带证据

HTTP 200 + application/json
→ 进入 JSON 成功/错误解析
```

---

# 7. P1：Provider 级真实请求预算

必须在 Host 30 次限制之外，增加单 Provider 真实发送限制：

```text
Quick：最多 2 次真实 HTTP Send
Smart：最多 12 次真实 HTTP Send
Deep：最多 16 次真实 HTTP Send
```

以下全部计入：

- URL 变体
- 协议变体
- 认证变体
- `max_completion_tokens → max_tokens`
- Streaming
- Tool Calling
- 稳定性复测
- Gemini Query Key 变体

缓存复用不计入真实发送数。

达到上限时：

```text
PROVIDER_BUDGET_EXHAUSTED
```

UI 的“预计最多请求”必须与真实上限一致。

---

# 8. P1：非流式响应体增量限制

不得再使用：

```rust
response.bytes().await
```

完整下载后才判断 2MB。

应：

1. 先检查 `Content-Length`；
2. 使用 `bytes_stream()` 分块读取；
3. 累计超过 2MB 立即停止；
4. 错误正文最多保留安全截断摘要；
5. 不能因超大响应导致内存暴涨。

---

# 9. P1：猜测模型不能标记为当前配置成功

当 CCS 配置中没有明确模型时：

- 不得将默认模型标记为 `is_current_config=true`；
- 不得显示“当前配置可直接使用”；
- 应标记：
  ```text
  model_is_guessed=true
  ```
- 成功状态：
  ```text
  MODEL_GUESS_OK
  ```
- UI 文案：
  ```text
  使用 Doctor 推测模型测试成功，但不能证明 CC Switch 当前模型配置可用。
  ```

---

# 10. P1：Gemini Query Key 兼容

实现两种认证：

```text
x-goog-api-key Header
?key= Query
```

优先复现当前配置；失败后再尝试安全变体。

缓存键必须区分两种认证方式。

测试：

```text
Header Key 成功
Query Key 成功
Header 失败 Query 成功
/v1beta 不重复
streamGenerateContent?alt=sse 正确
```

---

# 11. P1：v13 完整扫描回归测试

新增纯虚拟 Fixture：

```text
compatibility/fixtures/synthetic-v13.sql
```

必须覆盖真实流程：

```text
open_readonly
→ fingerprint v13 compatible
→ load providers
→ load endpoints
→ normalize provider
→ ProviderScanView.providers 非空
→ Claude Provider 可见
→ 完整 Key 不在前端 View
→ DB SHA256 前后不变
```

前端测试必须使用 v13 Scan Mock，验证：

- 默认 Claude 标签；
- Claude Provider 卡片存在；
- Provider 默认未勾选；
- 开始诊断默认禁用；
- 勾选后启用。

---

# 12. P1：Manifest 与运行时规则保持单一真相源

当前存在：

```text
compatibility/manifest.json
Rust SCHEMA_ALLOWLIST
```

两份规则。

必须采用以下任一方案：

## 推荐方案

编译期读取：

```rust
include_str!("../../../compatibility/manifest.json")
```

运行时由 Manifest 构建 Schema Allowlist。

## 最低要求

增加自动测试，比较：

- Manifest 中的 Schema 指纹；
- Rust Allowlist；
- Doctor 版本；
- verified / compatible 状态；
- requiredTables；
- requiredColumns。

任何不一致 CI 失败。

---

# 13. UI 紧凑化要求

用户反馈：当前轻量工具页面尺寸和文字区域偏大。

当前 900px 左右高度下：

- 左侧只能看到约 2.5 个 Provider；
- 右侧只能看到约 1～2 个结果；
- 顶部区域占用过高；
- 结果卡片存在较多留白；
- 诊断日志默认展开时占用过大。

本次只做密度优化，不重做布局。

## 13.1 默认紧凑密度

默认使用 Compact Density：

```text
正文：12.5～13px
次级文字：11.5～12px
卡片标题：14px
按钮高度：32～34px
Chip 高度：30～32px
Provider 卡片：约 96～112px
结果摘要卡片：尽量控制在 150～190px
```

## 13.2 顶部区域

压缩：

- App Header 上下 Padding；
- SessionControlBar 高度；
- 模式说明占用；
- 状态 Chip 间距；
- 按钮高度。

目标：

```text
Header + ControlBar 总高度减少约 20%～30%
```

不能删除模式说明，但可以改成单行短说明 + Tooltip。

## 13.3 Provider 卡片

目标：

- 900px 高窗口中至少可同时看到 4 个完整 Provider；
- 768px 高窗口中至少看到 3 个完整 Provider；
- 不牺牲 Key 脱敏、Host、模型、协议和状态信息。

调整：

- 减少上下 Padding；
- Provider 名与状态同一行；
- App / masked Key 放同一行；
- Host / 模型放同一行；
- 协议和详情按钮放底部紧凑行；
- 状态 Badge 缩小；
- 避免无意义空白。

## 13.4 结果卡片

默认折叠以下内容：

- 尝试链；
- 判定依据详情；
- 调试日志；
- 原始响应摘要。

首屏只展示：

```text
Provider 名
状态 Badge
一句结论
关键证据 1～2 条
建议 1～2 行
成功组合（如有）
```

目标：

- 900px 高窗口中至少显示 3 个结果摘要卡片；
- 768px 高窗口中至少显示 2 个结果摘要卡片。

## 13.5 调试日志

不得在结果卡片内部默认展开大块日志。

要求：

- 默认关闭；
- 独立折叠区域；
- 最大高度 140～180px；
- 等宽字体 11～12px；
- 原始响应仅显示脱敏后的摘要；
- 提供复制调试摘要按钮；
- 不显示完整巨型 JSON。

## 13.6 滚动体验

- 左右两列继续独立滚动；
- 顶部控制栏固定；
- 不能让整个窗口只有一个大滚动条；
- 不允许卡片内部再嵌套多个高滚动区域；
- 结果日志展开后不能把操作按钮挤出可视区。

## 13.7 可选密度切换

可以增加会话级：

```text
紧凑 / 舒适
```

但：

- 默认紧凑；
- 不写本地持久化；
- 不是 v0.1.5 必须项；
- 不得因此增加大范围重构。

---

# 14. 新增回归测试矩阵

## 14.1 Rust

必须新增或补齐：

```text
成功 Anthropic 响应包含 billing_usage 不误判
成功 OpenAI 响应包含 usage 不误判
2xx 结构化余额不足正确识别
2xx invalid key 正确识别
2xx HTML + Content-Type 正确识别
Anthropic endpoint 返回 OpenAI JSON 的兼容解析
OpenAI endpoint 返回 Anthropic JSON 的兼容解析
stream=true 返回完整 JSON 的回退解析
NDJSON 流式解析
Provider 真实请求预算
2MB 非流式增量停止
Path Key 脱敏
缓存键区分不同 Query Value
缓存键不包含 Path Key
Gemini Header / Query Key
v13 完整扫描
Manifest 与运行时规则一致
```

## 14.2 前端

必须新增：

```text
默认 Claude 筛选仍然有效
Provider 默认不勾选
核心筛选始终显示
三点菜单外部点击关闭
Esc 关闭
Provider 卡片紧凑密度
结果卡片默认折叠日志
真实 ErrorEvidence 展示
QUOTA_EXHAUSTED 显示明确证据
RESPONSE_PROTOCOL_VARIANT_OK 显示实际返回结构
900x768 下关键控件可见
```

## 14.3 安全门禁

必须全部继续通过：

```text
no process spawn
no protected paths
version sync
Actions pinned
Release version sync
完整 Key 不进入前端
DB SHA256 前后不变
跨 Host 阻断
```

---

# 15. 推荐提交顺序

必须按小提交执行：

## Commit 1

```text
fix(classifier): never override valid success with quota heuristics
```

只修成功响应误判和测试。

## Commit 2

```text
fix(parser): add cross-protocol and stream fallback parsing
```

只修返回格式兼容。

## Commit 3

```text
fix(security): redact URL path secrets and harden cache keys
```

只修 Key 脱敏和缓存键。

## Commit 4

```text
fix(diagnostics): add evidence, provider budgets and bounded body reads
```

只修证据、预算和响应限制。

## Commit 5

```text
test(schema): add synthetic v13 end-to-end regression
```

只补 v13 和 Manifest 测试。

## Commit 6

```text
style(ui): compact provider and result density without layout rewrite
```

只做紧凑 UI，不碰诊断后端。

## Commit 7

```text
release: prepare v0.1.5
```

只更新：

- package version；
- Cargo version；
- Tauri version；
- Manifest doctorVersion；
- CHANGELOG；
- Release Notes。

---

# 16. 发布前强制验收

以下全部通过才允许创建 `v0.1.5`：

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

GitHub Actions：

- CI 全绿；
- Release 全绿；
- Upstream Watch 不退化；
- Tag 与 main 对应提交一致；
- Release 资产非零；
- SHA256SUMS 文件名和 Hash 一致。

---

# 17. 最终交付资产

```text
CC-Switch-Doctor-v0.1.5-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.5-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release 继续明确：

```text
Unsigned build
Windows SmartScreen may warn
Source and CI are public
SHA-256 checksums provided
```

---

# 18. 最终汇报格式

完成后只输出：

```text
1. 修复提交列表
2. 成功响应误判修复说明
3. 返回格式兼容修复说明
4. ErrorEvidence 展示说明
5. URL Path Key 脱敏验证
6. Provider / Host 请求预算验证
7. v13 完整扫描测试结果
8. UI 紧凑化截图：
   - 1366x768
   - 1440x900
9. 本地测试结果
10. GitHub Actions 状态
11. v0.1.5 Tag Commit SHA
12. Release 三个资产名称、大小和 SHA-256
13. git status --short（必须为空）
```

不得只说“已完成”，必须提供可核对证据。

---

# 19. 直接交给 AI 工具的执行指令

```text
严格阅读并执行仓库中的 CC-Switch-Doctor-v0.1.5-Regression-Safe-Diagnosis-UI-Fix-Spec.md。

这是基于 main/v0.1.4 的回归安全修复任务。必须最小范围修改，禁止推倒 UI、Provider 扫描、诊断架构和安全边界。

尤其注意：
1. 真实成功响应永远优先于余额、Key、WAF 等关键词猜测；
2. billing_usage、usage 等正常计量字段绝不能触发 QUOTA_EXHAUSTED；
3. 命令行可用但返回结构非标准时，应进行安全的跨协议兼容解析，不能直接判 UNSUPPORTED_PROTOCOL；
4. 修复后面的诊断功能时，不得改坏默认 Claude 筛选、Provider 展示、默认不勾选、三点菜单关闭、v13 兼容、并发控件和三种模式说明；
5. UI 只做紧凑密度优化，不重做布局；
6. 每组修复独立提交并立即运行相关测试；
7. 全部旧测试、新测试、安全门禁、Windows 构建和远程 CI 成功后，发布 v0.1.5；
8. 远程 CI 或 Release 失败时继续修复，不能提前结束。
```
