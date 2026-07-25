# CC Switch Doctor v0.1.9 模型语义对齐与错误分类回归安全修复规范

> 目标版本：v0.1.9  
> 基线：当前 `main / v0.1.8`  
> 当前审查 HEAD：`b9db17935403cf2d9740a82f32da637442838c0d`  
> v0.1.8 核心合并提交：`c9b4518dc1db142df1b3a1b3fbffccb48c48d8f9`  
> 任务类型：小范围诊断准确性修复  
> 核心原则：**围绕 CC Switch Doctor 的目标功能修复，不重写架构，不增加无关功能，不再猜测模型语义。**

---

# 0. 本次问题背景

用户在 CC Switch 中配置：

```text
Provider：LD公益-国模abrdns
应用：Claude Code
协议：Anthropic Messages
配置模型：GLM-5.2[1M]
```

该 Provider 在 Claude Code 命令行中可以正常使用。

CC Switch Doctor v0.1.8 的诊断结果却显示：

```text
primary=MODEL_VARIANT_OK
direct=MODEL_VARIANT_OK
route.disposition=not_running
routeStatus=CCS_ROUTE_NOT_RUNNING
```

UI 文案：

```text
更换模型后可用
```

调试日志：

```text
#1 UNKNOWN_ERROR 503 https://new-api.abrdns.com/v1/messages
{"error":{"code":"model_not_found","message":"No available channel for model GLM-5.2[1M] ..."}}

#3 UNKNOWN_ERROR 503 https://new-api.abrdns.com/v1/chat/completions
{"error":{"code":"model_not_found","message":"No available channel for model GLM-5.2[1M] ..."}}

#6 GENERATE_OK 200 https://new-api.abrdns.com/v1/messages
```

这个结论不准确。

---

# 1. 已确认根因

## 1.1 `[1M]` 是本地能力标记，不是上游模型 ID 的一部分

CC Switch 上游已经明确实现：

```text
GLM-5.2[1M]
→ 发送上游前剥离 [1M]
→ GLM-5.2
```

CC Switch 源码说明：

```text
Claude Code 通过 [1M] 后缀声明 100 万上下文能力；
上游 API 通常不接受这个本地能力标记；
转发前需要剥离。
```

首要事实来源：

```text
仓库：farion1231/cc-switch
Commit：878c26f31e012ba32b9772bd080bd4fa9e7d495e
文件：src-tauri/src/proxy/model_mapper.rs
函数：
- strip_one_m_suffix_for_upstream
- strip_one_m_suffix_for_upstream_from_body
- apply_model_mapping
```

CC Switch 前端也将 `[1M]` 作为独立显示/编辑标记处理：

```text
文件：src/components/providers/forms/hooks/useModelState.ts
函数：
- hasClaudeOneMMarker
- stripClaudeOneMMarker
- setClaudeOneMMarker
```

因此：

```text
配置模型：GLM-5.2[1M]
实际发往上游：GLM-5.2
```

属于当前配置的正常运行语义，不是“换了一个模型”。

---

## 1.2 Doctor 当前把模型字符串差异直接判断成“更换模型”

Doctor 当前逻辑大致为：

```text
success_model != configured_model
→ model_changed = true
→ MODEL_VARIANT_OK
→ 更换模型后可用
```

这没有区分：

- 本地标记归一化；
- 当前 Provider 自带模型映射；
- 上游模型别名；
- 真正更换为另一个模型；
- Doctor 自己猜测模型。

因此 `GLM-5.2[1M] → GLM-5.2` 被误判为模型变体。

---

## 1.3 503 中的明确 `model_not_found` 被提前归为 UNKNOWN_ERROR

上游返回：

```json
{
  "error": {
    "code": "model_not_found",
    "message": "No available channel for model GLM-5.2[1M] ..."
  }
}
```

但当前分类器在进入完整模型错误匹配前，对 `500..=599` 提前返回：

```text
UNKNOWN_ERROR
```

同时当前模型错误关键词未覆盖：

```text
model_not_found
no available channel for model
no available provider for model
model unavailable
```

所以日志错误显示为：

```text
UNKNOWN_ERROR 503
```

正确分类应为：

```text
MODEL_NOT_FOUND
```

中文解释：

```text
当前模型名或当前分组没有可用渠道
```

---

# 2. 本次开发目标

本轮只完成以下六项：

1. 按 CC Switch 上游规则归一化 Claude `[1M]` 模型标记。
2. 让当前配置诊断使用真正的上游 Wire Model。
3. 区分“等价归一化”“配置内模型映射”“真正更换模型”“Doctor 猜测模型”。
4. 修复结构化 5xx 模型错误分类。
5. 修复成功证据优先级和 UI 文案。
6. 修复 Provider 卡片 URL 的真实 Key 脱敏旁路。

---

# 3. 明确不做的内容

本轮不得：

- 重写整个 Planner。
- 重写协议 Adapter。
- 新增新的 AI CLI。
- 调用 Claude Code、Codex、Gemini CLI、OpenCode。
- 修改 CC Switch 数据库或 Provider。
- 启动、停止或切换 CCS 路由。
- 重做 UI 布局。
- 增加新的大规模模型注册中心。
- 增加远程模型元数据库。
- 扩展不相关协议。
- 推翻 v0.1.8 的 Schema Capability 架构。
- 修改已经稳定的 Primary / Direct / Route 分层。

---

# 4. 冻结功能清单

以下功能不得退化。

## 4.1 数据和安全

- SQLite `mode=ro`。
- `SQLITE_OPEN_READ_ONLY`。
- `query_only=ON`。
- 数据库读取前后 SHA-256 不变。
- 不读取 `.claude`、`.codex`、Gemini、OpenCode 登录目录。
- 不调用 Shell 或子进程。
- 完整 Key 不进入前端、日志、缓存键或剪贴板。
- 跨 Host 重定向继续阻断。
- CCS 路由只允许 loopback。

## 4.2 Schema 兼容

- 精确版本只决定 Verified。
- 结构能力检测决定能否运行。
- 未知 `user_version` 但核心结构兼容时继续工作。
- 单 Provider 异常不影响其他 Provider。
- 路由结构异常不影响 Provider 和 Direct Diagnosis。

## 4.3 UI 和诊断

- 默认筛选 Claude。
- Provider 默认不勾选。
- 并发 1 / 2 / 3 可见可改。
- 快速验证、智能诊断、深度兼容含义不变。
- 自动 / 仅直连 / 直连+路由不变。
- Provider 与 Result 双向定位不变。
- 三点菜单点击外部和 Esc 关闭。
- 路由辅助状态不覆盖直连主结果。
- 缓存复用不计入真实发送次数。
- 请求预算不退化。

---

# 5. 最小模型语义数据结构

不要建立复杂的全局模型数据库。

只需将当前裸字符串候选：

```rust
Vec<String>
```

改为小型、明确的数据结构：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCandidate {
    /// UI 中展示的模型配置值。
    pub display_model: String,

    /// 实际放入上游 HTTP 请求 body 的模型值。
    pub wire_model: String,

    /// 候选来源。
    pub source: ModelCandidateSource,

    /// 是否与当前 CC Switch 配置在运行语义上等价。
    pub equivalent_to_current: bool,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCandidateSource {
    /// 当前配置原始模型，且无需归一化。
    ConfiguredModel,

    /// 当前配置仅移除了本地能力标记，例如 [1M]。
    LocalMarkerNormalized,

    /// 当前 Provider 配置中明确存在的角色模型映射。
    ConfiguredRoleMapping,

    /// 从同一 Host /models 得到的模型或别名。
    DiscoveredModel,

    /// Doctor 内置的保守猜测。
    DoctorGuess,
}
```

不要增加更多枚举，除非有真实 Fixture 和明确来源证明必要。

---

# 6. `[1M]` 模型归一化

## 6.1 实现位置

新增一个小型模块，建议：

```text
src-tauri/src/ccs_adapter/model_semantics.rs
```

核心函数：

```rust
pub fn strip_claude_one_m_marker(model: &str) -> Cow<'_, str>
```

行为必须与 CC Switch 上游保持一致：

```text
GLM-5.2[1M]     → GLM-5.2
GLM-5.2[1m]     → GLM-5.2
GLM-5.2[1M]     → GLM-5.2
GLM-5.2 [1M]    → GLM-5.2
GLM-5.2         → GLM-5.2
MODEL[1M]-X     → 不处理
```

只允许处理：

```text
字符串末尾的 [1M]，忽略大小写和尾部空白
```

禁止：

- 删除模型名中间的 `[1M]`。
- 删除其他括号内容。
- 删除 `[128K]`、`[200K]` 等未验证标记。
- 用正则泛化删除任意上下文标记。

---

## 6.2 当前配置计划

对于 Claude / Claude Desktop Provider：

```text
display_model = 原始配置模型
wire_model = strip_claude_one_m_marker(display_model)
source =
  两者相同：ConfiguredModel
  两者不同：LocalMarkerNormalized
equivalent_to_current = true
```

当前配置首个请求必须直接发送：

```text
wire_model
```

不得先发送已知会被 CC Switch 剥离的原始 `[1M]` 模型，再把无标记模型当作回退。

这样 Quick 模式才真正验证：

```text
当前 CC Switch / Claude Code 会实际发往上游的请求
```

而不是验证一个 CCS 实际不会发送的错误请求。

---

## 6.3 日志保留两个模型值

AttemptResult 增加可选字段：

```rust
pub configured_model_display: Option<String>,
pub outbound_model: Option<String>,
pub model_transform: Option<String>,
```

或使用等价的小型结构，避免重复字段。

UI 日志示例：

```text
模型：GLM-5.2[1M] → GLM-5.2
处理：CCS 本地 [1M] 上下文标记归一化
```

普通日志不得把这种情况写成：

```text
模型候选 GLM-5.2
更换模型后成功
```

---

# 7. Provider 配置中的模型映射

CC Switch `model_mapper.rs` 会根据当前 Provider 的配置执行：

```text
Claude 客户端角色模型
→ Provider 配置中的实际模型
```

常见配置字段：

```text
ANTHROPIC_MODEL
ANTHROPIC_DEFAULT_HAIKU_MODEL
ANTHROPIC_DEFAULT_SONNET_MODEL
ANTHROPIC_DEFAULT_OPUS_MODEL
ANTHROPIC_DEFAULT_FABLE_MODEL
CLAUDE_CODE_SUBAGENT_MODEL
```

Doctor 已经读取这些字段，但当前只把它们作为无来源字符串列表。

本轮需要保留来源：

```text
当前主模型
当前 Sonnet 映射
当前 Opus 映射
当前 Haiku 映射
当前 Fable 映射
当前 Subagent 映射
```

## 7.1 结果语义

角色模型映射成功时，不应显示：

```text
更换模型后可用
```

应显示：

```text
当前配置中的模型映射可用
```

建议新增状态：

```text
CONFIGURED_MODEL_MAPPING_OK
```

中文：

```text
当前 Provider 配置中的模型映射可用
```

建议：

```text
无需修改配置。该成功模型来自当前 CC Switch Provider 的角色模型映射。
```

这不等于当前主模型已经成功，因此不能一律升级为：

```text
CURRENT_CONFIG_OK
```

只有 `ConfiguredModel` 或 `LocalMarkerNormalized` 成功时，才能设置：

```text
current_config_ok = true
```

---

# 8. 最终状态判定规则

## 8.1 当前配置成功

以下成功都属于当前配置：

```text
ConfiguredModel 成功
LocalMarkerNormalized 成功
```

最终：

```text
CURRENT_CONFIG_OK
```

`LocalMarkerNormalized` 成功时附加模型转换说明。

---

## 8.2 配置内映射成功

```text
ConfiguredRoleMapping 成功
```

最终：

```text
CONFIGURED_MODEL_MAPPING_OK
```

不得提示用户更换模型。

---

## 8.3 真正模型变体成功

只有满足以下条件才返回：

```text
MODEL_VARIANT_OK
```

条件：

- 成功模型与当前配置不是等价归一化关系；
- 成功模型不是当前 Provider 明确配置的角色映射；
- 成功模型不是简单大小写差异；
- 成功模型不是已验证的本地标记剥离；
- 成功模型来自发现结果或用户明确的其他模型。

UI：

```text
当前模型不可用，其他模型可用
```

建议中明确列出：

```text
当前模型：X
成功模型：Y
```

不能只写模糊的“更换模型后可用”。

---

## 8.4 Doctor 猜测模型成功

```text
DoctorGuess 成功
```

最终：

```text
MODEL_GUESS_OK
```

不能表示当前配置通过。

---

# 9. 成功证据排序

当前代码不应只记录“第一次成功”。

建立一个简单评分函数，不需要复杂优化器：

```rust
fn success_evidence_rank(candidate: &ModelCandidate, plan: &PlannedAttempt) -> u32
```

优先级从高到低：

```text
1. 当前 URL + 当前协议 + ConfiguredModel
2. 当前 URL + 当前协议 + LocalMarkerNormalized
3. 当前 URL + 当前协议 + ConfiguredRoleMapping
4. URL 修正成功
5. 认证变体成功
6. 协议变体成功
7. DiscoveredModel 成功
8. DoctorGuess 成功
9. LooseField / CrossProtocol 宽松成功
```

最终 `success_plan` 应选择：

```text
证据质量最高的成功
```

而不是：

```text
时间上最早的成功
```

这项修改应保持小范围，只用于最终结论选择，不重写请求执行顺序。

---

# 10. 结构化错误分类修复

## 10.1 顺序原则

非 2xx 响应分类必须按以下顺序：

```text
1. 解析结构化 Error Envelope
2. 读取明确 error.code / type / message
3. 使用强语义映射
4. 再使用 HTTP 状态兜底
5. 最后才 UNKNOWN_ERROR
```

不得在 `500..=599` 分支提前丢弃明确的业务错误。

---

## 10.2 错误代码映射

新增明确映射：

```text
model_not_found
model_not_available
model_unavailable
no_available_channel
no_available_provider
```

统一归为：

```text
MODEL_NOT_FOUND
```

消息匹配补充：

```text
no available channel for model
no available provider for model
model unavailable
model is unavailable
unsupported model
```

中文补充：

```text
模型不可用
没有可用渠道
无可用渠道
当前分组无可用渠道
```

---

## 10.3 503 的处理

例如：

```json
{
  "error": {
    "code": "model_not_found",
    "message": "No available channel for model GLM-5.2[1M] under group ..."
  }
}
```

必须得到：

```text
classification=MODEL_NOT_FOUND
httpStatus=503
evidence.source=error_envelope
evidence.code=model_not_found
```

不得得到：

```text
UNKNOWN_ERROR
```

---

## 10.4 不得误判成功响应

结构化错误修复必须继续遵守：

```text
error: null
error: false
error: ""
error: {}
error: []
```

都不是错误。

普通成功响应中的：

```text
billing_usage
usage
credit_cost
model
```

不能触发余额或模型错误。

---

# 11. 失败与成功同时存在时的说明

本案例中可能保留历史失败尝试或缓存尝试。

最终结果区应按如下方式表达：

```text
结论：当前配置可用

模型处理：
GLM-5.2[1M] → GLM-5.2
已按 CC Switch 的本地上下文标记规则归一化。

辅助失败：
带 [1M] 的原始上游模型值被服务端拒绝；
该值不是 CCS 实际转发值，因此不影响当前配置可用结论。
```

不过修复后首个真实请求应直接发送归一化模型，因此正常情况下不再产生这条无意义失败。

---

# 12. CCS 路由状态保持辅助语义

当前案例：

```text
route.disposition=not_running
routeStatus=CCS_ROUTE_NOT_RUNNING
```

这是辅助信息。

最终仍应：

```text
primary=CURRENT_CONFIG_OK
direct=CURRENT_CONFIG_OK
route.disposition=not_running
```

UI：

```text
当前配置可用
辅助：CCS 路由已配置但未运行，本次仅完成上游直连验证。
```

禁止再次让：

```text
CCS_ROUTE_NOT_RUNNING
```

覆盖模型归一化后的直连主结果。

---

# 13. UI 文案

## 13.1 Provider 卡片

本案例应显示：

```text
当前配置可直接使用
```

可选小型辅助标记：

```text
模型已归一化
```

Tooltip：

```text
配置模型 GLM-5.2[1M]；
发送上游时按 CC Switch 规则使用 GLM-5.2。
```

---

## 13.2 Result Card

建议结构：

```text
诊断结论
当前配置可用

上游直连
Anthropic Messages / GLM-5.2 / 200

模型语义
配置值：GLM-5.2[1M]
上游值：GLM-5.2
规则：剥离 Claude/CCS 本地 [1M] 上下文标记

CCS 路由
未验证
CCS 路由已配置但未运行
```

---

## 13.3 状态文案调整

保留：

```text
MODEL_VARIANT_OK
```

但中文改得更明确：

```text
当前模型不可用，其他模型可用
```

新增：

```text
CONFIGURED_MODEL_MAPPING_OK
当前 Provider 配置中的模型映射可用
```

`CURRENT_CONFIG_OK` 保持：

```text
当前配置可直接使用
```

---

# 14. Provider 卡片 URL 的 Key 脱敏修复

当前 Attempt URL 使用注册了 Provider Key 的 `SecretRedactor`，但 Provider 卡片的：

```text
safe_base_url
```

仍使用通用 URL Sanitizer。

本轮修复：

```rust
let mut redactor = SecretRedactor::new();
redactor.register_key(&api_key);
let safe_base_url = sanitize_url_with_redactor(&base_url, &redactor);
```

或者等价的安全封装。

必须覆盖：

```text
Query 参数中的 Key
Path 中的 Key
URL 编码后的 Key
非 sk- 前缀 Key
短 Key
```

不得把完整 Key 暴露给前端。

---

# 15. 源码参考要求

本轮不需要新增大型研究阶段，但实现前必须核对并记录以下上游代码：

```text
farion1231/cc-switch
Commit：878c26f31e012ba32b9772bd080bd4fa9e7d495e

src-tauri/src/proxy/model_mapper.rs
src/components/providers/forms/hooks/useModelState.ts
src-tauri/src/proxy/forwarder.rs
```

在以下文件追加本次研究记录：

```text
docs/research/v0.1.9-model-semantics-review.md
```

至少记录：

```text
- [1M] 标记的真实语义
- CC Switch 剥离规则
- 模型映射顺序
- Provider 配置字段
- Doctor 采用的最小实现
- 没有复制哪些代理功能
- 上游 Commit SHA
- MIT 许可证说明
```

禁止凭经验扩展其他上下文后缀。

---

# 16. 必须新增的 Fixture

目录建议：

```text
tests/fixtures/models/
```

新增：

```text
claude-one-m-uppercase.json
claude-one-m-lowercase.json
claude-one-m-trailing-space.json
model-not-found-503.json
model-no-channel-503.json
configured-role-mapping.json
doctor-guess-success.json
provider-url-key-in-path.json
```

所有 Fixture：

- 不含真实 Key。
- 不含真实私人 Host。
- 不含个人请求 ID。
- 必须使用虚拟域名和虚拟凭据。

---

# 17. 必须新增的测试

## 17.1 `[1M]` 归一化

```text
GLM-5.2[1M]
→ wire_model=GLM-5.2
→ equivalent_to_current=true
```

```text
GLM-5.2[1m]
→ GLM-5.2
```

```text
GLM-5.2[1M] + 尾部空格
→ GLM-5.2
```

```text
GLM-[1M]-TEST
→ 不处理
```

```text
GLM-5.2[128K]
→ 不处理
```

---

## 17.2 当前配置状态

```text
configured=GLM-5.2[1M]
outbound=GLM-5.2
GENERATE_OK
→ primary=CURRENT_CONFIG_OK
→ direct=CURRENT_CONFIG_OK
→ current_config_ok=true
→ 不得 MODEL_VARIANT_OK
```

---

## 17.3 路由未运行

```text
LocalMarkerNormalized 成功
route.disposition=not_running
→ primary=CURRENT_CONFIG_OK
→ routeStatus=CCS_ROUTE_NOT_RUNNING
→ 路由只作为辅助信息
```

---

## 17.4 结构化 503 模型错误

```text
HTTP 503
error.code=model_not_found
→ MODEL_NOT_FOUND
```

```text
HTTP 503
message=No available channel for model ...
→ MODEL_NOT_FOUND
```

```text
HTTP 503
普通未知网关错误
→ UNKNOWN_ERROR 或 GATEWAY_OR_WAF
```

不得把所有 503 都判成模型错误。

---

## 17.5 配置内模型映射

```text
当前主模型失败
当前 Provider 中明确配置的 Sonnet 模型成功
→ CONFIGURED_MODEL_MAPPING_OK
→ 不得 MODEL_VARIANT_OK
→ 不得提示修改 Provider
```

---

## 17.6 真正模型变体

```text
当前模型 model-a 失败
从 /models 发现 model-b 成功
→ MODEL_VARIANT_OK
→ 文案明确 model-a → model-b
```

---

## 17.7 Doctor Guess

```text
无配置模型
DoctorGuess 成功
→ MODEL_GUESS_OK
→ current_config_ok=false
```

---

## 17.8 成功证据排序

```text
DoctorGuess 先成功
LocalMarkerNormalized 后成功
→ 最终选择 LocalMarkerNormalized
→ CURRENT_CONFIG_OK
```

```text
协议变体先成功
当前配置等价模型后成功
→ CURRENT_CONFIG_OK
```

---

## 17.9 Key 脱敏

```text
https://example.com/sk-secret-123/path
→ 前端不得出现完整 Key
```

```text
https://example.com/api?token=abc123
→ 前端不得出现完整 token
```

```text
URL 编码后的 Key
→ 仍需脱敏
```

---

# 18. 推荐提交顺序

## Commit 1

```text
fix(model): normalize Claude one-million context marker for upstream
```

只实现 `[1M]` 归一化和测试。

## Commit 2

```text
refactor(model): preserve candidate source and current-equivalence
```

把裸字符串候选改为最小 `ModelCandidate`。

## Commit 3

```text
fix(classifier): classify structured 5xx model errors before status fallback
```

修复 `model_not_found` 和 “No available channel”。

## Commit 4

```text
fix(outcome): distinguish configured mapping from true model variant
```

修复最终状态与成功证据排序。

## Commit 5

```text
fix(ui): explain configured and outbound model semantics
```

只修改相关 Badge、Result Card 和日志。

## Commit 6

```text
fix(security): redact provider key from displayed base URLs
```

修复卡片 URL 脱敏。

## Commit 7

```text
test(v0.1.9): add model semantics and structured error regressions
```

补齐回归矩阵。

## Commit 8

```text
release: prepare v0.1.9
```

仅版本、CHANGELOG 和 Release Notes。

---

# 19. 禁止事项

- 禁止只把 `MODEL_VARIANT_OK` 文案改成“当前配置可用”。
- 禁止不改变请求模型就强行改最终状态。
- 禁止用正则删除所有方括号后缀。
- 禁止猜测 `[128K]`、`[200K]` 等其他后缀。
- 禁止把所有 503 判定为 `MODEL_NOT_FOUND`。
- 禁止用裸字符串列表继续表达全部模型语义。
- 禁止让配置内角色映射变成 Doctor Guess。
- 禁止让 Doctor Guess 设置 `current_config_ok=true`。
- 禁止把路由未运行变成主状态。
- 禁止为了此修复重写 Planner 或 Adapter Registry。
- 禁止调用任何 CLI 验证。
- 禁止修改 CC Switch 配置。
- 禁止删除旧回归测试。
- 禁止 CI 未通过就发布。

---

# 20. 本地验证命令

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

```text
旧测试全部通过
新模型语义测试通过
结构化错误测试通过
Schema v16 / Future Schema 测试不退化
Route Outcome 测试不退化
数据库 SHA-256 不变
无子进程调用
无登录目录读取
无完整 Key 进入 DOM
```

---

# 21. 发布资产

```text
CC-Switch-Doctor-v0.1.9-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.9-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release Notes 必须包含：

```text
- Align Claude `[1M]` model marker handling with CC Switch upstream behavior.
- Treat local marker normalization as the current configuration, not a model replacement.
- Classify structured `model_not_found` 5xx responses correctly.
- Distinguish configured role mappings, discovered model variants, and Doctor guesses.
- Improve model evidence and outbound model display.
- Harden Provider URL key redaction.
```

---

# 22. 最终验收案例

输入：

```text
App：Claude Code
配置模型：GLM-5.2[1M]
协议：Anthropic Messages
Base URL：https://new-api.example.com
CCS 路由：已配置但未运行
```

上游：

```text
GLM-5.2 请求成功
```

最终必须显示：

```text
Primary：CURRENT_CONFIG_OK
Direct：CURRENT_CONFIG_OK
CurrentConfigOk：true

配置模型：GLM-5.2[1M]
实际上游模型：GLM-5.2
模型处理：剥离 CCS/Claude 本地 [1M] 上下文标记

RouteDisposition：not_running
RouteStatus：CCS_ROUTE_NOT_RUNNING
```

UI：

```text
当前配置可直接使用
```

建议：

```text
当前配置可用。`[1M]` 是 Claude/CC Switch 的本地上下文能力标记，
发送上游时已按 CC Switch 规则使用 `GLM-5.2`。无需更换模型或修改配置。
辅助：CCS 路由已配置但未运行，本次仅完成上游直连验证。
```

绝对不能显示：

```text
MODEL_VARIANT_OK
更换模型后可用
```

---

# 23. 最终汇报格式

完成后只输出：

```text
1. 修复提交列表
2. [1M] 归一化实现及上游源码依据
3. ModelCandidate 最小数据结构
4. 配置模型 / Outbound Model 结果截图
5. 503 model_not_found 分类测试
6. 配置内角色映射测试
7. 真正模型变体测试
8. Doctor Guess 测试
9. 成功证据排序测试
10. Key URL 脱敏测试
11. 全部旧测试结果
12. Windows 构建结果
13. GitHub Actions 状态
14. v0.1.9 Tag SHA
15. Release 资产大小和 SHA-256
16. git status --short（必须为空）
```

---

# 24. 直接交给 AI 工具的执行指令

```text
严格阅读并执行：

docs/project/CC-Switch-Doctor-v0.1.9-Model-Semantics-and-Error-Classification-Regression-Safe-Spec.md

基于当前 main/v0.1.8 做小范围回归安全修复，不得扩大项目范围。

首要问题：

Claude / CC Switch 配置中的 `[1M]` 是本地 100 万上下文能力标记，不是上游模型 ID 的一部分。必须按 farion1231/cc-switch 的 model_mapper.rs 规则，在发送上游前剥离模型末尾的 `[1M]`，并把该成功视为 CURRENT_CONFIG_OK，而不是 MODEL_VARIANT_OK。

必须同时修复：

1. `GLM-5.2[1M] → GLM-5.2` 等价归一化；
2. ModelCandidate 来源与 equivalent_to_current；
3. 配置内角色模型映射不能显示“更换模型后可用”；
4. 503 中 error.code=model_not_found 和 “No available channel for model” 必须分类为 MODEL_NOT_FOUND；
5. 结构化 Error Envelope 必须先于通用 5xx 兜底；
6. 最终成功证据按语义质量排序，不再只取第一次成功；
7. UI 显示配置模型与实际发往上游模型；
8. Provider 卡片 Base URL 使用真实 Key Redactor。

不得重写 Planner、协议 Adapter、Schema Capability、路由架构或 UI 主布局。
不得启动任何 AI CLI，不得读取登录目录，不得修改 CC Switch 配置或数据库。

严格冻结 v0.1.8 已通过功能：
- Schema 结构能力兼容；
- SQLite 只读；
- 默认 Claude 筛选；
- Provider 默认不勾选；
- Primary / Direct / Route 分层；
- 路由辅助状态不覆盖直连；
- 双向结果定位；
- 请求预算和 Key 脱敏。

按文档的小提交顺序执行，每组修改立即运行相关测试。
全部旧测试、新测试、安全门禁、Windows 构建和 GitHub Actions 成功后发布 v0.1.9。
```
