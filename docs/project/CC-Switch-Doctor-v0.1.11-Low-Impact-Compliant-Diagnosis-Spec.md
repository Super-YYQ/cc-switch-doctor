# CC Switch Doctor v0.1.11 低扰动合规诊断与请求指纹治理规范

> 目标版本：v0.1.11  
> 基线版本：当前 `main / v0.1.10`  
> 审查基线提交：`05c9b7c9b90fd6aa8ea8c080d9b48c6285d78e30`  
> 任务类型：诊断请求低扰动化、固定产品指纹清理、模式安全边界补强  
> 核心原则：**减少误判、配额消耗和无必要的自动化特征，但不得伪装官方客户端、规避供应商风控或承诺“不会被检测”。**

---

# 0. 结论与合规边界

部分 API 供应商、中转站或订阅服务可能会识别：

- 固定测活提示词；
- 固定工具名；
- 高频重复请求；
- 批量模型或协议枚举；
- 同一 Host 短时间并发；
- 明确的诊断工具 User-Agent；
- Streaming、Tool Calling 等能力探测组合。

CC Switch Doctor 无法保证任何请求“不被检测”。

本轮允许做的是：

```text
降低请求扰动
减少固定产品专属 Prompt 指纹
减少默认请求数量
避免无必要的批量能力探测
让用户明确知道 Smart / Deep 会发送更多请求
保持透明和符合供应商服务条款
```

本轮绝对禁止：

```text
伪装成 Claude Code、Codex、Gemini CLI 或 OpenCode
复制官方 SDK 私有 Header 以冒充官方客户端
随机轮换 User-Agent
随机化 Prompt 以规避检测
修改 TLS / HTTP 指纹
代理或 IP 轮换
请求时间抖动用于逃避风控
隐藏自动化来源
绕过供应商的测活、反滥用或封禁机制
```

产品和文档中禁止使用：

```text
隐身模式
防检测模式
绕过测活检测
伪装官方请求
```

统一称为：

```text
低扰动验证
Provider 友好验证
合规诊断
```

---

# 1. 最新仓库审查结果

## 1.1 v0.1.10 前端修复已落地

当前 `main` 已完成：

- 删除 Provider “查看详情”按钮；
- 只有存在诊断结果时 Provider 才可跳转；
- Provider 三行紧凑布局；
- ResultCard 结论去重；
- Direct / Route 紧凑摘要；
- 内部控件不会触发跨栏跳转。

本轮不得再次修改这些 UI 结构，除非增加非常小的模式风险提示。

## 1.2 普通生成请求具有固定产品指纹

当前生产代码定义：

```rust
PROMPT_ZH = "只输出字符串 CCS_DOCTOR_OK，不要输出其他内容。"
PROMPT_EN = "Reply with exactly CCS_DOCTOR_OK and nothing else."
SUCCESS_MARKER = "CCS_DOCTOR_OK"
```

所有主要协议都使用同一个固定 Prompt：

```text
Anthropic Messages
OpenAI Chat Completions
OpenAI Responses
Gemini Native
```

响应只有包含：

```text
CCS_DOCTOR_OK
```

才判定为完整生成成功。

这会造成：

1. 每次请求内容高度一致；
2. 请求直接暴露产品名称和用途；
3. 某些中转站可能把它归类为测活请求；
4. 模型正常返回其他有效文本时被降级成 `PARTIAL_TEXT`；
5. 普通生成成功过度依赖固定字符串，而不是协议级有效响应。

## 1.3 Deep Tool Calling 具有固定诊断标识

当前 Deep Tool Calling 固定使用：

```text
工具名：ccs_doctor_echo
描述：Echo a value for connectivity testing
Prompt：Call the ccs_doctor_echo tool with value "ok"
```

这是明确的能力探测请求。

本轮不需要隐藏或伪装该工具调用。

正确处理是：

- Tool Calling 继续只存在于 Deep；
- UI 明确提示 Deep 是能力测试，容易被供应商识别为自动诊断；
- 不把 Tool Calling 加入 Quick；
- 不通过改名、随机名或复制真实业务工具来绕过检测。

## 1.4 HTTP User-Agent 已透明标识 Doctor

当前默认 HTTP Client 使用：

```text
CC-Switch-Doctor/<version> (+https://github.com/Super-YYQ/cc-switch-doctor)
```

这是透明、诚实的客户端标识。

不得为了“不被检测”而：

- 删除 User-Agent；
- 随机 User-Agent；
- 改成 Claude Code / Codex / curl / 浏览器；
- 复制官方 SDK User-Agent；
- 添加伪造的官方客户端 Header。

Provider 在 CC Switch 配置中已有 `customUserAgent` 时，Doctor 可以继续按现有配置发送，因为它属于当前 Provider 配置的一部分；但 Doctor 不得自动生成、修改或推荐冒充官方客户端的 UA。

## 1.5 默认模式和请求预算偏重

当前前端默认：

```text
mode = smart
concurrency = 1
```

当前真实请求上限：

```text
Quick：每 Provider 2
Smart：每 Provider 12
Deep：每 Provider 16
同一 Host：单次会话最多 30
同一 Host 连续两次限流后停止
```

虽然实际缓存、成功提前停止和 Planner 会减少请求，但默认 Smart 仍可能在失败配置上快速尝试：

- URL；
- 协议；
- 认证；
- 模型；
- Streaming；
- 字段回退。

对共享订阅或严格中转站而言，默认行为偏重。

---

# 2. 本轮开发目标

只完成以下内容：

1. 将默认诊断模式改为 Quick。
2. 将 Quick 明确定义为“低扰动验证”：
   - 每 Provider 最多 1 次真实上游请求；
   - 不执行协议、URL、认证、模型变体；
   - 不执行 Streaming；
   - 不执行 Tool Calling；
   - 默认有效并发固定为 1。
3. 移除普通 Generate / Stream 请求中的 `CCS_DOCTOR_OK` 产品专属标记。
4. 原生协议返回可解析的非空有效文本时，判定为 Generate 成功。
5. Cross Protocol 和 Loose Field 继续保持较低证据等级。
6. 保留透明 Doctor User-Agent，不做任何客户端伪装。
7. 在 Smart / Deep UI 中增加清晰但不打扰的请求风险说明。
8. 更新安全说明，明确不能保证供应商不识别自动化请求。
9. 增加完整回归测试，确保请求更少但诊断语义不失真。

---

# 3. 开发边界

## 3.1 允许修改

```text
src/App.tsx
src/components/SessionControlBar.tsx
src/components/SafetyDrawer.tsx
src/lib/utils.ts
src/types/index.ts（仅必要的现有类型说明）

src-tauri/src/protocols/types.rs
src-tauri/src/protocols/anthropic.rs
src-tauri/src/protocols/openai_chat.rs
src-tauri/src/protocols/openai_responses.rs
src-tauri/src/protocols/gemini.rs
src-tauri/src/protocols/http_executor.rs

src-tauri/src/diagnostics/planner.rs
src-tauri/src/diagnostics/session_budget.rs
src-tauri/src/diagnostics/engine.rs（只允许最小结果语义适配）

相关测试、Fixtures、CHANGELOG、版本和 Release Notes
```

## 3.2 禁止修改

除本规范明确要求外，不得修改：

```text
CC Switch 数据库读取
Schema Capability
Provider 归一化
模型 [1M] 语义
模型候选来源
错误分类规则
CCS 路由发现和路由请求
Primary / Direct / Route 结果分层
URL 安全策略
重定向安全策略
Key 脱敏
CLI 隔离
```

不得新增：

- 定时测活；
- 后台自动检测；
- 请求历史持久化；
- Prompt 模板设置页面；
- User-Agent 设置页面；
- 代理设置；
- 随机延迟策略；
- 随机 Prompt 池；
- 客户端模拟器；
- 官方 CLI 请求复刻；
- Provider 自动切换。

---

# 4. Quick 重新定义为低扰动验证

## 4.1 默认模式

前端初始值从：

```ts
useState<DiagnosisMode>("smart");
```

改为：

```ts
useState<DiagnosisMode>("quick");
```

刷新数据库、重新打开应用时仍默认 Quick。

不保存上次模式，不新增持久化。

## 4.2 Quick 请求计划

Quick 只生成一个 Direct Attempt：

```text
当前 Base URL
当前协议
当前认证
当前或等价归一化模型
非流式 Generate
```

禁止 Quick 生成：

```text
URL Variant
Protocol Variant
Auth Variant
Model Variant
Streaming
Tool Calling
Token Field 二次回退
Gemini Header → Query Key 二次回退
CCS 路由业务请求
```

关于 CCS Route：

- Quick 的 `verifyMode=auto` 仍可以读取并展示路由配置状态；
- Quick 不发送 CCS Route 业务请求；
- 用户明确选择“直连+路由”时，UI 应提示：
  ```text
  快速验证不会执行路由链业务请求；请切换智能诊断。
  ```
- 不要静默升级 Quick 为 Smart。

## 4.3 Quick 请求预算

调整：

```rust
Quick => 1
```

并增加测试：

```text
任何 Quick Provider：
真实上游请求数 <= 1
```

即使当前请求返回：

- 参数不兼容；
- Gemini Header Auth 失败；
- 404；
- 模型错误；

Quick 也不自动发送第二次请求。

结果中可以提示：

```text
快速验证只测试当前配置；切换智能诊断可尝试兼容变体。
```

## 4.4 Quick 并发

Quick 模式有效并发固定为 1。

UI 可以：

- 将 2 / 3 按钮禁用；
- 或保留显示但标注 Quick 实际使用并发 1。

推荐禁用 2 / 3，Tooltip：

```text
低扰动验证固定串行执行，避免同一时间对多个 Provider 或 Host 发起探测。
```

切换 Smart / Deep 后恢复 1 / 2 / 3。

---

# 5. 普通 Prompt 去除产品专属标记

## 5.1 删除生产常量

删除普通生成路径中的：

```rust
PROMPT_ZH
PROMPT_EN
SUCCESS_MARKER
```

不得使用新的产品专属标记替代，例如：

```text
DOCTOR_OK
CCS_OK
PING_OK
MODEL_ALIVE
HEALTH_CHECK_OK
```

不得使用随机挑战字符串来隐藏探测用途。

## 5.2 新的普通生成 Prompt

使用短小、正常、无产品名的标准 Prompt。

建议统一：

```text
Reply briefly.
```

或者：

```text
Provide a brief response.
```

只选择一个固定、普通、无产品标识的 Prompt。

不得建立 Prompt 池，不得随机切换语言或内容。

原因：

- 目标是减少产品专属指纹；
- 不是通过随机化规避供应商识别；
- 保持测试可复现。

建议定义：

```rust
pub const BASIC_GENERATE_PROMPT: &str = "Reply briefly.";
```

## 5.3 Token 上限

普通 Generate 保持较小上限：

```text
16 tokens
```

不需要为了自然回答提高到很大。

若部分模型在 16 Token 下无法形成可解析文本，Smart 可以按现有变体体系诊断；Quick 不追加请求。

---

# 6. 生成成功判定调整

## 6.1 原生协议

对于：

```text
HTTP 2xx
没有明确 Structured Error Envelope
响应结构匹配请求协议
成功提取非空文本
```

直接判定：

```text
GENERATE_OK
ok = true
partial = false
```

不再要求文本包含固定 Marker。

示例：

```text
"Hello."
"OK"
"Sure."
"你好"
```

都可以是原生 Generate 成功。

## 6.2 Cross Protocol

如果请求 Anthropic，但返回 OpenAI Chat 结构，且成功提取非空文本：

```text
RESPONSE_PROTOCOL_VARIANT_OK
```

继续：

```text
ok = true
current_config_ok = false
```

不得因为取消 Marker 就错误升级为原生当前配置成功。

## 6.3 Loose Field

只从非标准宽松字段提取文本：

```text
LOOSE_RESPONSE_TEXT_OK
partial = true
ok = false
```

保持不变。

## 6.4 空文本

以下不能成功：

```text
空字符串
全空白
只有 null
只有 reasoning 且当前规则不接受 reasoning 为完整回答
结构存在但没有可消费内容
```

仍按现有准确状态处理。

## 6.5 Confidence

普通原生非空文本成功可以继续视为当前配置成功，但 Confidence 文案应说明证据：

```text
HTTP 2xx + 原生协议结构 + 非空生成文本
```

不得声称：

```text
已证明模型完全支持所有 CLI 能力
```

Tool Calling、Streaming 仍需 Deep 单独验证。

---

# 7. Streaming 判定

Streaming 请求只存在于 Smart / Deep。

成功条件：

```text
HTTP 2xx
流式 Content-Type 或事件格式可识别
按目标协议解析到至少一个非空文本增量
流没有明显协议错误
```

不再要求流式拼接文本包含 `CCS_DOCTOR_OK`。

结果：

```text
STREAM_OK
```

Cross Protocol 流式解析成功：

```text
STREAM_PROTOCOL_VARIANT_OK
```

只有连接成功但无文本增量时，继续使用现有准确失败状态。

---

# 8. Tool Calling 保持显式诊断

Tool Calling 继续：

```text
仅 Deep
```

本轮不要求删除：

```text
ccs_doctor_echo
```

也不要求把它改成随机或业务化名称。

理由：

- Tool Calling 本身就是明确能力测试；
- 试图隐藏工具名属于规避识别；
- 用户选择 Deep 即表示接受诊断请求可被识别。

UI Deep Tooltip：

```text
深度兼容会测试 Streaming、Tool Calling 和稳定性，属于明显的自动化能力诊断，可能被供应商识别或计费。
```

---

# 9. User-Agent 合规规则

## 9.1 默认 User-Agent

必须保留：

```text
CC-Switch-Doctor/<version> (+repository URL)
```

可以调整格式一致性，但不能隐藏工具身份。

## 9.2 Provider customUserAgent

现有 CC Switch Provider 已配置 `customUserAgent` 时：

- 可以继续使用；
- 仅使用数据库中现有值；
- 不在 Doctor 中提供编辑；
- 不自动猜测；
- 不自动替换；
- 不根据 AppType 生成 Claude/Codex/Gemini UA。

## 9.3 禁止 Header 模拟

禁止新增：

```text
x-stainless-*
openai-client-user-agent
anthropic-client-*
claude-code-version
codex-version
sec-ch-ua
浏览器 Cookie
官方请求 ID 格式
```

除非未来官方公开协议明确要求，并另行审查。

本版本不开发这些能力。

---

# 10. Smart / Deep 请求提示

## 10.1 Smart

文案改为：

```text
智能诊断：当前配置失败后，才尝试同 Host 的 URL、协议、认证和模型兼容变体。可能发送多次请求并被识别为自动诊断。
```

## 10.2 Deep

文案改为：

```text
深度兼容：在智能诊断基础上增加 Streaming、Tool Calling 和稳定性测试。请求最多、可能产生更多计费，也最容易被识别为能力测试。
```

## 10.3 开始按钮附近

当模式为 Smart / Deep 且选中 Provider > 0 时，显示中性提示：

```text
此模式可能发送多次自动化诊断请求；请确认供应商允许此类使用。
```

不需要弹窗，不需要二次确认，不新增持久化。

## 10.4 Safety Drawer

增加：

```text
Doctor 无法保证供应商不会识别自动化请求。
本工具不会伪装官方客户端或绕过供应商风控。
快速验证默认仅发送一次标准生成请求。
```

---

# 11. 请求预算

## 11.1 本版本必须修改

```text
Quick 每 Provider：1
Quick 有效并发：1
```

## 11.2 Smart / Deep

本版本不要大幅重写 Planner 或预算架构。

现有上限可以暂时保留：

```text
Smart：12
Deep：16
Host：30
```

但 UI 必须显示实际预估。

同时补充以下提前停止原则测试：

```text
当前配置成功 → Smart 不继续发普通兼容变体
明确 Auth Invalid / Quota Exhausted → 停止
连续限流 → 停止
缓存复用 → 不增加真实发送数
```

后续如真实数据证明 Smart/Deep 上限过高，再独立调整；不要在本轮同时重做所有诊断策略。

---

# 12. 结果和日志文案

## 12.1 普通成功

Evidence：

```text
HTTP 200
原生 Anthropic Messages 响应
成功解析非空文本
```

不得继续显示：

```text
检测到 CCS_DOCTOR_OK 标记
```

## 12.2 非空但 Cross Protocol

明确显示：

```text
返回了有效文本，但响应结构属于 OpenAI Chat，不是配置的 Anthropic Messages。
```

## 12.3 Quick 失败

建议：

```text
快速验证只测试当前配置，未自动尝试兼容变体。切换智能诊断可继续检查 URL、协议、认证或模型组合。
```

## 12.4 请求可识别性

调试日志可以显示：

```text
诊断模式：Quick / Smart / Deep
请求用途：Generate / Stream / ToolCall
```

但不要使用：

```text
隐身
规避检测成功
未被识别
```

---

# 13. 防止错误的“低扰动”实现

## 13.1 Prompt 随机化

禁止：

```text
每次从多个自然问题中随机选一个
随机生成业务问题
根据 Provider 自动变化 Prompt
```

## 13.2 时间随机化

禁止：

```text
随机等待 1～10 秒
模拟真人输入间隔
随机分布请求时间
```

允许的只有确定性安全控制：

```text
Quick 串行
请求预算
遇限流停止
```

## 13.3 客户端伪装

禁止复制：

- Claude Code Header；
- Codex SDK Header；
- Gemini CLI Header；
- 浏览器 UA；
- 官方客户端版本。

## 13.4 网络规避

禁止：

- 代理池；
- IP 轮换；
- 节点轮换；
- 域名前置；
- TLS 指纹修改；
- HTTP/2 指纹模拟。

---

# 14. 必须新增的测试

## 14.1 默认模式

```text
App 初始化
→ mode=quick
→ concurrency=1
```

## 14.2 Quick 计划

```text
Quick Provider
→ Planner 仅 1 个 Attempt
→ 非流式 Generate
→ 当前 URL
→ 当前协议
→ 当前模型
```

确认不存在：

```text
URL Variant
Protocol Variant
Auth Variant
Model Variant
Stream
ToolCall
```

## 14.3 Quick 请求预算

```text
当前请求失败
→ 不发送第二次请求
```

```text
Gemini Header Auth 失败
→ Quick 不自动 Query Key 回退
```

```text
OpenAI max_completion_tokens 不支持
→ Quick 不自动 max_tokens 回退
```

这些回退只在 Smart / Deep 执行。

## 14.4 普通原生文本

```text
HTTP 200
Anthropic 原生 content text = "Hello."
→ GENERATE_OK
→ ok=true
```

同样覆盖：

- OpenAI Chat；
- OpenAI Responses；
- Gemini Native。

## 14.5 空文本

```text
HTTP 200
原生结构但 text=""
→ 不得 GENERATE_OK
```

## 14.6 Cross Protocol

```text
请求 Anthropic
返回 OpenAI Chat 非空文本
→ RESPONSE_PROTOCOL_VARIANT_OK
→ current_config_ok=false
```

## 14.7 Loose Field

```text
只从宽松字段提取
→ LOOSE_RESPONSE_TEXT_OK
→ ok=false
→ partial=true
```

## 14.8 Streaming

```text
非空原生文本增量
→ STREAM_OK
```

```text
连接成功但无文本增量
→ 不得 STREAM_OK
```

## 14.9 User-Agent

无 Provider custom UA：

```text
User-Agent 以 CC-Switch-Doctor/ 开头
```

有 Provider custom UA：

```text
精确使用当前配置值
```

并断言代码中没有生成：

```text
Claude Code
Codex CLI
Gemini CLI
浏览器 UA
```

## 14.10 固定 Marker 清理

生产协议代码中：

```text
普通 Generate / Stream 不包含 CCS_DOCTOR_OK
```

允许旧文档或历史 Fixture 存在，但新生产代码和新请求 Fixture 不得包含。

Tool Calling 的 `ccs_doctor_echo` 可以保留，仅 Deep 使用。

## 14.11 UI 风险说明

Quick：

```text
显示“低扰动 / 1 次当前配置请求”
```

Smart / Deep：

```text
显示可能发送多次并被识别为自动诊断的说明
```

## 14.12 冻结回归

继续通过：

```text
v0.1.10 Provider / Result 紧凑 UI
v0.1.9 模型 [1M] 语义
v0.1.8 Schema Capability
路由状态不覆盖 Direct
SQLite 只读
Key 脱敏
无 CLI spawn
无登录目录读取
```

---

# 15. 建议代码结构

不建立复杂 Prompt Registry。

最小实现：

```rust
pub const BASIC_GENERATE_PROMPT: &str = "Reply briefly.";

pub fn evaluate_native_text(text: &str) -> bool {
    !text.trim().is_empty()
}
```

若保留现有 `evaluate_text()`，必须确保：

- 仅 Native Adapter 使用它决定 `GENERATE_OK`；
- Cross Protocol 仍映射到 Variant；
- Loose Field 仍为 Partial；
- Tool Call 不使用普通文本判定；
- Structured Error Envelope 优先级不变。

不要引入：

```text
PromptProfile Registry
动态模板
远程规则
随机挑战
Provider 特例库
```

---

# 16. 推荐提交顺序

```text
1. fix(probe): remove product-specific marker from normal generate requests
2. fix(quick): make quick diagnosis a single-request low-impact path
3. fix(ui): default to quick and explain smart/deep request impact
4. docs(safety): document transparent client identity and non-evasion boundary
5. test(v0.1.11): add low-impact request and fingerprint regressions
6. release: prepare v0.1.11
```

---

# 17. 禁止事项

- 禁止承诺“不会被供应商检测”。
- 禁止添加“防检测”开关。
- 禁止随机 Prompt。
- 禁止随机延迟。
- 禁止 UA 轮换。
- 禁止官方客户端伪装。
- 禁止复制官方私有 Header。
- 禁止代理或 IP 轮换。
- 禁止后台定时测活。
- 禁止删除 Doctor 默认透明 UA。
- 禁止把非空 Loose Field 升级成当前配置成功。
- 禁止让 Quick 自动执行任何第二次上游请求。
- 禁止为了减少请求而删除 Smart / Deep。
- 禁止修改 CC Switch 数据库、路由或 Provider。
- 禁止扩大到新的协议和模型规则。
- 禁止 CI 未通过就发布。

---

# 18. 发布前验证

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

额外检查：

```bash
rg -n "CCS_DOCTOR_OK" src-tauri/src/protocols
rg -n "claude-code|codex-cli|gemini-cli|Mozilla/5.0" src-tauri/src
rg -n "thread_rng|jitter|rotate.*user.?agent|proxy.*pool" src-tauri/src
```

验收：

- 第一条在普通 Generate / Stream 生产路径无结果；
- `ccs_doctor_echo` 仅存在 Deep Tool Calling；
- 没有官方客户端伪装代码；
- 没有随机化规避逻辑；
- Quick 真实发送最多 1 次；
- Smart / Deep 风险提示可见。

---

# 19. Release 资产

```text
CC-Switch-Doctor-v0.1.11-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.11-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release Notes：

```text
- Quick validation is now the default low-impact mode.
- Quick sends at most one current-configuration request per Provider.
- Removed the product-specific CCS_DOCTOR_OK marker from normal generation requests.
- Native protocol responses with non-empty valid text now count as generation success.
- Smart and Deep modes clearly disclose that they may send multiple automated diagnostic requests.
- Kept a transparent CC-Switch-Doctor User-Agent; no official-client spoofing or anti-detection behavior was added.
- Added regression tests for request count, response semantics, and non-evasion boundaries.
```

---

# 20. 最终验收案例

## 20.1 默认启动

```text
模式：快速验证
并发：1
说明：仅发送 1 次当前配置标准生成请求
```

## 20.2 Quick 成功

请求：

```text
POST 当前配置 Endpoint
Prompt：Reply briefly.
max tokens：16
```

返回原生协议非空文本。

结果：

```text
CURRENT_CONFIG_OK
GENERATE_OK
原生协议响应
真实请求：1
```

## 20.3 Quick 失败

当前 Endpoint 返回 404。

结果：

```text
ENDPOINT_NOT_FOUND
真实请求：1
建议：切换智能诊断可尝试 URL / 协议变体
```

不得自动发第二次请求。

## 20.4 Smart

UI 显示：

```text
可能发送多次自动化诊断请求；请确认供应商允许此类使用。
```

## 20.5 Deep

UI 显示：

```text
将测试 Streaming、Tool Calling 和稳定性；可能被供应商识别为能力测试并产生更多计费。
```

---

# 21. 最终汇报格式

```text
1. 修复提交列表
2. 固定 Prompt 指纹清理说明
3. Native / CrossProtocol / LooseField 新判定矩阵
4. Quick 单请求实现和测试
5. Smart / Deep 风险提示截图
6. User-Agent 透明性测试
7. 无随机化、无伪装代码检查
8. 全部前端测试
9. 全部 Rust 测试
10. Security Verify
11. Windows 构建
12. GitHub Actions 状态
13. v0.1.11 Tag SHA
14. Release 资产大小和 SHA-256
15. git status --short（必须为空）
```

---

# 22. 直接交给 AI 工具的执行指令

```text
严格阅读并执行：

docs/project/CC-Switch-Doctor-v0.1.11-Low-Impact-Compliant-Diagnosis-Spec.md

这是基于 main/v0.1.10 的低扰动合规诊断修复。

目标不是“绕过供应商检测”，不得实现任何隐身、风控规避或官方客户端伪装。

只完成：
1. 默认模式改为 Quick；
2. Quick 每 Provider 最多 1 次当前配置请求，有效并发固定 1；
3. Quick 不执行 URL、协议、认证、模型、Streaming、Tool Call 或字段二次回退；
4. 移除普通 Generate / Stream 中的 CCS_DOCTOR_OK 产品专属标记；
5. 原生协议 2xx + 可解析非空文本判定 GENERATE_OK；
6. Cross Protocol 和 Loose Field 继续保持 Variant / Partial 语义；
7. 保留透明 CC-Switch-Doctor User-Agent；
8. Smart / Deep 显示可能发送多次并被识别为自动诊断的说明；
9. Safety Drawer 明确不保证不可识别，也不会伪装或绕过风控；
10. 补齐请求次数、响应判定、UA 和非规避边界测试。

禁止：
- Prompt 随机化；
- 随机延迟；
- User-Agent 轮换；
- Claude/Codex/Gemini 官方客户端伪装；
- 官方私有 Header 模拟；
- 代理/IP 轮换；
- 后台定时测活；
- 新设置和无关功能；
- 修改数据库、路由、Schema、模型语义或错误分类。

冻结 v0.1.10 UI、v0.1.9 模型语义、v0.1.8 Schema Capability、Primary/Direct/Route 分层、SQLite 只读、Key 脱敏和 CLI 隔离。

按照文档的小提交顺序执行。全部旧测试、新测试、安全门禁、Windows 构建和 GitHub Actions 成功后发布 v0.1.11。
```
