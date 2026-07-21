# CC Switch Doctor v0.1.3 代码审查、交互与诊断准确性修复任务文档

> 目标仓库：`Super-YYQ/cc-switch-doctor`  
> 当前基线：`v0.1.2`  
> 目标版本：`v0.1.3`  
> 文档性质：可直接交给 Codex、Claude Code 或其他 AI 开发工具执行的定向修复规格  
> 核心原则：**不得推倒现有 UI 和安全架构，只修复本文件列出的缺陷并完成测试、CI、Release。**

---

## 0. 给 AI 工具的直接执行指令

请先完整阅读本文件，再审查仓库当前 `main` 分支。

执行要求：

1. 基于现有 v0.1.2 继续开发，不重新创建项目；
2. 保留现有 CC Switch 风格 UI，不大范围改版；
3. 先修复 P0/P1，再处理 P2/P3；
4. 不允许为了通过测试删除、弱化或绕过安全限制；
5. 必须补齐单元测试、前端测试、安全门禁和 Windows 构建；
6. 完成后发布 `v0.1.3`，不得覆盖已有 Tag 或 Release；
7. 最终必须给出修改文件、测试结果、Actions 状态、Release 资产和已知限制；
8. 若远程 CI 或 Release 失败，继续定位并修复，不能在失败状态下宣布完成。

---

# 一、必须继续遵守的安全红线

以下约束不可回退：

- CC Switch 数据库只读打开：`mode=ro`、`SQLITE_OPEN_READ_ONLY`、`query_only=ON`；
- 不写入 CC Switch 数据库，不切换 Provider；
- 不启动 Codex、Claude Code、Claude、OpenCode、Gemini CLI、CC Switch；
- 不启动 Shell、PowerShell、CMD 或任意外部进程；
- 不读取 `.codex`、`.claude`、`.gemini`、OpenCode 等登录目录；
- 不读取 Codex Plus、Claude 订阅或其他官方登录缓存；
- 完整 API Key 只能存在 Rust 内存中，不得发送到前端；
- 不保存诊断结果、选择、历史、日志或 Key；
- 不使用 `localStorage`、IndexedDB 或持久化 Store 保存业务数据；
- 自动 URL 变体只允许同 Origin；
- 不跟随跨 Host 重定向，不向新 Host 转发凭据；
- 默认并发为 1，最大并发不得超过 3；
- 同一 Origin 单次诊断会话最多 30 次真实 HTTP 请求；
- 同一 Origin 连续两次真实限流后停止继续请求；
- 未知 Schema 必须安全停止，不得猜测凭据字段。

---

# 二、P0 阻断级修复

## P0-1：修复请求缓存键，确保 URL、认证方式等变体真正发送

### 当前问题

当前会话缓存键没有完整覆盖请求语义。至少缺少：

- 最终请求 URL/Path；
- HTTP Method；
- 认证方式（Bearer、x-api-key、x-goog-api-key、Query Key 等）；
- 影响请求行为的自定义 User-Agent；
- 请求 Body 的关键差异；
- 相关 Header 差异。

这会导致同一个 Host 下不同 `/v1` Path、不同认证方式、不同请求体错误复用前一次结果。

例如：

```text
https://api.example.com/messages       → 404
https://api.example.com/v1/messages    → 本应真实发送，但错误命中缓存并复用 404
```

这会直接破坏本工具最核心的 URL 修正、协议变体和认证变体诊断能力。

### 修改要求

缓存键应在 `BuiltRequest` 完整构造后生成，而不是仅依据 Planner 字段生成。

建议结构：

```rust
struct RequestCacheKey {
    origin: OriginKey,
    method: String,
    canonical_url: String,
    protocol: ProtocolKind,
    model: String,
    purpose: RequestPurpose,
    stream: bool,
    tool_call: bool,
    token_limit_field: Option<TokenLimitField>,
    auth_scheme: AuthScheme,
    user_agent_fingerprint: Option<String>,
    relevant_headers_fingerprint: String,
    request_body_fingerprint: String,
    key_fingerprint: String,
}
```

要求：

- `canonical_url` 必须包含最终 Path 和非敏感 Query 结构；
- 不能把完整 Key 或 Authorization Header 值放入缓存键；
- 认证 Header 只保留 `AuthScheme`；
- Body 使用稳定 JSON 序列化后 SHA-256 指纹；
- Header 只对真正影响请求语义的非敏感 Header 做稳定指纹；
- 完全相同请求才允许复用；
- 不同 URL、认证方式、User-Agent、Body 均不得复用。

### 并发去重

多个并发 Provider 发起完全相同请求时，最好实现 single-flight：

```text
第一个请求真实发送
其他相同请求等待第一个完成
完成后复用结果
```

至少不能出现三个相同请求同时穿透缓存。

### 验收测试

必须覆盖：

- 同 Host、不同 Path 不复用；
- `/messages` 与 `/v1/messages` 不复用；
- Bearer 与 x-api-key 不复用；
- 不同 User-Agent 不复用；
- 不同 Token 字段不复用；
- 完全相同请求正常复用；
- 缓存键及日志中不出现完整 Key；
- 并发完全相同请求最多真实发送一次，或明确记录当前暂不支持 single-flight。

---

# 三、P1 必须修复

## P1-1：正确读取 CC Switch 自定义数据目录

### 当前问题

CC Switch 当前使用：

```text
Store 文件：app_paths.json
键：app_config_dir_override
Tauri identifier：com.ccswitch.desktop
```

Doctor 当前读取的 `settings.json`、`app-store.json`、`store.json` 及旧键名不能覆盖真实机制。

用户在 CC Switch 设置自定义数据目录后，Doctor 可能自动找不到数据库。

### 修改要求

1. 根据 CC Switch 当前 Tauri Identifier 定位应用 Store 目录；
2. 读取 `app_paths.json`；
3. 精确读取 `app_config_dir_override`；
4. 支持：
   - `~`
   - `~/path`
   - `~\path`
   - Windows 盘符路径
   - UNC 路径
5. 拼接 `cc-switch.db` 后确认文件存在；
6. 只允许探测明确已知位置，不递归扫描整个用户目录；
7. 保留手动选择 DB 作为兜底。

### 自动定位优先级

```text
1. CC_SWITCH_DOCTOR_DB 临时环境变量（测试/调试）
2. CC Switch app_paths.json / app_config_dir_override
3. 真实用户目录 ~/.cc-switch/cc-switch.db
4. 仅当默认位置不存在时，兼容 Windows HOME 旧路径
5. 明确的已知便携位置
6. 用户手动选择
```

### 验收测试

- 标准默认目录；
- 自定义盘符目录；
- `~` 展开；
- UNC 目录；
- Store 文件不存在；
- Store JSON 损坏；
- Override 目录不存在时安全回退。

---

## P1-2：修复旧 Run 事件污染和取消语义

### 当前问题

事件中已有 `runId`，但前端未过滤当前 Run。用户取消任务 A 后立即刷新或启动任务 B，A 的迟到事件可能覆盖 B 或新扫描结果。

后端取消接口也忽略传入的 `runId`，直接取消当前所有任务。

### 修改要求

前端：

```ts
const activeRunIdRef = useRef<string | null>(null);

function handleEvent(event: DiagnosisEvent) {
  if (event.runId !== activeRunIdRef.current) return;
  // 正常处理
}
```

要求：

- `startDiagnosis` 返回 Run ID 后立即设置当前 ID；
- 取消期间显示“正在停止”；
- 点击取消后不要立即把 `running=false`；
- 等匹配的 `run_cancelled` 或 `run_finished` 到达后再解除运行状态；
- 新扫描/选择 DB 时清除当前 Run ID；
- 旧 Run 的所有迟到事件必须忽略。

后端：

- 实现 `cancel_run(run_id)`；
- 只取消匹配的活动 Run；
- Run 完成后调用 `complete_run(run_id)` 清理活动状态；
- 不匹配的取消请求返回明确错误或 no-op；
- 不允许旧任务完成时清除新任务的 CancellationToken。

### 验收测试

- 取消 A 后立刻启动 B，A 结果不会进入 B；
- 取消 A 后刷新数据库，A 结果不会重新出现；
- 传入错误 Run ID 不取消当前 Run；
- 双击开始不会启动两个活动 Run；
- 取消按钮状态与后端完成状态一致。

---

## P1-3：收紧 Schema 兼容门禁

### 当前问题

不能仅凭 `user_version` 位于一个宽泛区间、以及少量关键列存在，就把未知 Schema 标记为 Compatible 并读取 `settings_config`。

### 修改要求

改为 Manifest 驱动：

```text
Verified
= 精确 user_version + 已审核表结构 + 已审核关键字段指纹

Compatible
= compatibility/manifest.json 中明确记录并人工审核过的指纹

Unknown/Unsupported
= 只读取非敏感 Schema 元信息；不读取 settings_config；不提取 Key；不发送 HTTP
```

删除或停用以下宽泛逻辑：

```rust
(12..=20).contains(&user_version)
```

不得因为缺失 `provider_endpoints` 就自动判定为 Compatible，除非该结构已在 Manifest 明确审核。

### 验收测试

- v15 已验证指纹 → Verified；
- 仅多一个无关字段但 Manifest 已允许 → Compatible；
- user_version 新增但未登记 → Unknown；
- providers 关键字段缺失 → Unsupported；
- Unknown 时不得调用 Credential Extractor；
- Unknown 时前端不可勾选或开始诊断。

---

## P1-4：区分 Anthropic Token 与 API Key 的真实认证方式

### 当前问题

`ANTHROPIC_AUTH_TOKEN` 与 `ANTHROPIC_API_KEY` 被统一当成 x-api-key，导致正确配置可能先失败，再被错误标记为“认证变体成功”。

### 修改要求

Credential Extractor 必须记录凭据来源和首选认证方式：

```text
ANTHROPIC_AUTH_TOKEN → Authorization: Bearer <token>
ANTHROPIC_API_KEY    → x-api-key: <key>
OPENROUTER_API_KEY   → 根据 Provider/API Format 决定，通常 Bearer
```

建议增加：

```rust
struct ExtractedCredential {
    secret: SecretString,
    source: CredentialSource,
    preferred_auth: AuthScheme,
}
```

当前配置测试必须复现真实认证方式；其他认证方式只能作为明确标记的诊断变体。

### UI 表现

结果中显示：

```text
当前认证：Bearer（来自 ANTHROPIC_AUTH_TOKEN）
```

只显示来源字段名和认证类型，不显示 Secret。

### 验收测试

- `ANTHROPIC_AUTH_TOKEN` 首次请求使用 Bearer；
- `ANTHROPIC_API_KEY` 首次请求使用 x-api-key；
- Bearer 与 x-api-key 缓存不互相复用；
- UI 与日志无完整 Key。

---

## P1-5：修复 UTF-8 截断 Panic

### 当前问题

不能对 Rust UTF-8 字符串直接使用非字符边界的 `&s[..max]` 或 `truncate(max)`。

### 修改要求

建立统一函数：

```rust
fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}
```

所有以下位置统一使用：

- HTTP 错误正文；
- response excerpt；
- 流式文本摘要；
- UI 诊断摘要；
- URL 或日志摘要。

### 验收测试

- 超长中文；
- Emoji；
- 中英文混合；
- 截断点处于 2、3、4 字节字符中间；
- 空字符串和极小长度；
- 测试不得 Panic。

---

## P1-6：确保任何形式的完整 Key 都不进入前端

### 当前问题

URL 清洗只识别有限查询参数。以下形式可能泄露：

```text
?api-key=<key>
?x-api-key=<key>
?x-goog-api-key=<key>
?signature=<key>
/path/<key>
```

### 修改要求

前端展示用 URL 应采用更保守策略：

1. 删除 userinfo；
2. 删除 fragment；
3. 查询参数默认保留参数名，但全部隐藏参数值；
4. 或至少所有疑似 Secret 参数值统一替换为 `***`；
5. 在拥有 Provider 完整 Key 的 Rust 层，再执行一次已注册 Key 的全文替换；
6. 对 URL Path 中完整 Key 做替换；
7. 前端、事件、错误、日志、复制摘要均只接受 Safe DTO。

建议展示：

```text
https://example.com/v1?key=***&region=***
```

不要在前端显示真实 Query 值，即使其看起来不敏感。

移除 `SecretRedactor` 不必要的 `Serialize/Deserialize` 派生。

### 验收测试

- Key 位于已知查询参数；
- Key 位于未知查询参数；
- Key 位于 Path；
- Key 位于错误正文；
- Key 位于 Header 回显；
- Key 位于嵌套 JSON；
- 所有前端事件序列化结果均无完整 Key。

---

# 四、本次新增的 UI 与使用体验要求

## UX-1：恢复“并发数”展示和修改能力

### 当前问题

顶部只显示“并发 1”文本，用户不能修改，也不清楚为什么固定为 1。

### 目标

并发数必须：

- 可见；
- 可修改；
- 默认值为 1；
- 仅允许 1、2、3；
- 运行中锁定；
- 直接传入后端 `StartDiagnosisRequest.concurrency`；
- 不得绕过 Host 预算和连续 429 停止机制。

### 推荐 UI

在模式选择右侧增加紧凑选择器：

```text
并发  [ 1 | 2 | 3 ]  ⓘ
```

或：

```text
并发数  [ 1 ▼ ]
```

说明 Tooltip：

```text
同时诊断的 Provider 数量。默认 1 最稳妥；2–3 更快，但更容易触发中转站限流。无论并发多少，同一 Host 每次会话仍最多发送 30 次真实请求。
```

### 交互要求

- 运行中 Disabled；
- 用户修改后仅在当前会话内生效，不持久化；
- 刷新 DB 后恢复默认 1，或保留本次会话值均可，但不得写盘；
- 顶部预估请求数和运行状态同步更新；
- 选择 3 时显示轻提示，不需要阻止。

### 验收测试

- 可切换 1/2/3；
- 运行中不能修改；
- 后端实际收到正确值；
- 超过 3 的请求被后端拒绝；
- 并发 3 也不会突破 Host 30 次预算。

---

## UX-2：明确解释“快速验证 / 智能诊断 / 深度兼容”

### 当前问题

三个模式只有名称，没有说明。用户无法判断耗时、Token 消耗和测试内容差异。

### 必须实现的说明方式

至少实现以下两种中的一种，推荐同时实现：

1. 模式按钮 Hover/Focus Tooltip；
2. 模式区域旁增加 `?` 帮助按钮，打开简短说明 Popover。

安全边界 Drawer 中也应增加“诊断模式说明”章节。

### 模式定义

#### 快速验证

```text
只优先测试当前配置的 URL、协议、认证方式和模型。
速度最快、Token 消耗最低，适合日常确认配置是否还能用。
不进行大范围 URL、协议和认证变体尝试。
```

预期行为：

- 当前配置真实推理请求；
- 可选一个非常有限的必要字段兼容回退；
- 不测试 Streaming；
- 不测试 Tool Calling；
- 不做稳定性复测；
- UI 显示实际预估请求数，不硬编码错误数字。

#### 智能诊断

```text
先测试当前配置；失败后按错误类型尝试同 Host 的安全 URL、协议、认证和模型变体。
适合排查 /v1、协议格式、认证 Header、模型名等常见配置问题。
```

预期行为：

- 当前配置；
- `/v1` 增删和重复 Path 修正；
- Chat / Responses / Anthropic / Gemini 的合理协议候选；
- 认证方式变体；
- 模型候选；
- 最多 12 次真实请求/Provider；
- 动态 fallback 必须计入 12 次预算。

#### 深度兼容

```text
在智能诊断基础上继续测试 Streaming、Tool Calling 和稳定性复测。
耗时和 Token 消耗最高，适合确认高级能力与复杂兼容性。
```

预期行为：

- 包含智能诊断；
- Streaming；
- Tool Calling；
- 稳定性复测；
- 最多 16 次真实请求/Provider；
- 仍遵守同 Host 30 次会话上限。

### 视觉要求

当前选中模式下方或旁边显示一行短说明，例如：

```text
智能诊断：失败后自动尝试同 Host 的 URL、协议、认证和模型变体。
```

不得让用户必须打开文档才能理解模式。

### 验收测试

- 鼠标和键盘 Focus 均可看到说明；
- 三种模式文案与实际 Planner 行为一致；
- 模式改变后预估请求数更新；
- Tooltip 不遮挡主要操作；
- 说明中明确 Token/耗时差异。

---

## UX-3：默认选中 CC Switch 当前配置

### 用户目标

扫描完成后，默认勾选 CC Switch 当前正在使用的第三方 Provider，避免每次手动选择。

### 默认选择规则

每次成功扫描、刷新配置或重新选择 DB 后：

```text
自动选择所有：is_current == true && selectable == true
```

说明：

- CC Switch 可能每个 App Type 各有一个当前 Provider，因此可以默认选中多个；
- 官方 OAuth、托管账号、无静态 Key、不可测试配置不得选中；
- Schema Unknown 时不选择任何项；
- 用户手动修改选择后，不自动反复覆盖；
- 下次刷新时重新根据最新 `is_current` 状态初始化选择。

### 与旧状态清理的关系

`applyFreshScan()` 应：

1. 清除旧结果、旧 Run、旧选择；
2. 从新扫描结果中重新计算 `defaultSelected`；
3. 设置默认选择；
4. 可将第一个当前配置设为 `activeId`，也可以保持未激活。

示例：

```ts
const defaults = new Set(
  view.providers.filter((p) => p.isCurrent && p.selectable).map((p) => p.opaqueId),
);
setSelected(defaults);
```

### 增加批量操作

批量菜单中增加：

```text
选择 CCS 当前配置
```

方便用户修改选择后快速恢复。

### UI 提示

顶部可显示：

```text
已自动选中 3 个 CCS 当前配置
```

使用一次性 Toast，不持久化。

### 验收测试

- 一个当前配置自动选中；
- 多 App 当前配置全部选中；
- 当前配置为 OAuth 时不选中；
- 刷新后按新 `is_current` 重建选择；
- 非当前 Provider 不自动选中；
- 用户手动取消后不会被普通渲染重新选中。

---

## UX-4：改进状态文案，避免直接展示机器枚举

当前结果中大量直接展示：

```text
UNSUPPORTED_PROTOCOL
```

用户无法理解，也会怀疑是否真实原因是 Key、权限或余额。

### 修改要求

结果卡主标题和状态 Badge 应使用中文用户文案：

```text
协议或响应格式不兼容
```

机器码放在详情中：

```text
技术状态：UNSUPPORTED_PROTOCOL
```

结果卡必须优先展示：

1. 中文结论；
2. 主要证据；
3. 置信度；
4. 可能原因；
5. 建议动作；
6. 技术状态码和尝试链。

---

# 五、重点重做：UNSUPPORTED_PROTOCOL 与错误分类

## CLASS-1：`UNSUPPORTED_PROTOCOL` 只能作为排除性结论

### 当前风险

`UNSUPPORTED_PROTOCOL` 可能掩盖以下真实问题：

- Key 无效；
- Key 无权限；
- 余额不足；
- 额度耗尽；
- 限流；
- WAF/Cloudflare 页面；
- 模型不存在；
- 错误 Body 使用非标准结构；
- HTTP 200 中嵌套错误；
- 中转站把错误包装成文本或 HTML。

### 强制规则

只有满足以下条件，才可判定 `UNSUPPORTED_PROTOCOL`：

1. HTTP 请求确实到达服务端；
2. 没有鉴权、权限、额度、限流、模型、端点、网关或 WAF 错误证据；
3. HTTP 状态成功，或上游明确表示响应格式/协议不支持；
4. 响应无法按预期协议解析，且尝试其他合理协议后存在明确对比证据；
5. 结论置信度通常为 low 或 medium，不能轻易标 high。

不能把“解析失败”直接等同于“协议不支持”。

---

## CLASS-2：错误分类优先级

建议建立结构化优先级：

```text
1. SECURITY_BLOCKED
2. AUTH_INVALID
3. AUTH_PERMISSION_DENIED
4. QUOTA_EXHAUSTED
5. RATE_LIMITED
6. MODEL_NOT_FOUND
7. ENDPOINT_NOT_FOUND
8. GATEWAY_OR_WAF
9. TLS_ERROR
10. NETWORK_UNREACHABLE
11. TIMEOUT
12. INVALID_REQUEST_PARAMETER
13. RESPONSE_FORMAT_MISMATCH
14. UNSUPPORTED_PROTOCOL
15. UNKNOWN_ERROR
```

最终摘要不得简单取“第一个失败”；应选择优先级最高、证据最强的失败。

---

## CLASS-3：统一解析标准和非标准错误结构

对每个响应先执行统一 Error Probe，再执行协议正文解析。

### 需要识别的字段

递归检查：

```text
error
error.message
error.type
error.code
message
msg
detail
code
status
statusCode
error_code
error_description
```

### 需要识别的鉴权关键词

```text
invalid api key
incorrect api key
unauthorized
authentication failed
invalid token
token expired
signature invalid
api key not valid
invalid x-api-key
permission denied
forbidden
```

### 需要识别的余额/额度关键词

```text
insufficient_quota
quota exceeded
quota exhausted
insufficient balance
balance insufficient
no balance
credit exhausted
credits exhausted
billing
payment required
余额不足
额度不足
欠费
无可用额度
```

### 需要识别的限流关键词

```text
rate limit
too many requests
requests per minute
tokens per minute
retry after
限流
请求过于频繁
```

### 需要识别的模型错误

```text
model not found
unknown model
invalid model
model does not exist
no access to model
模型不存在
无权访问模型
```

### WAF/网关识别

若 Content-Type 是 HTML 或正文包含：

```text
cloudflare
access denied
captcha
just a moment
nginx
bad gateway
web application firewall
```

应判定：

```text
GATEWAY_OR_WAF
```

不得判为 `UNSUPPORTED_PROTOCOL`。

---

## CLASS-4：HTTP 状态与 Body 联合判断

### 401

优先：

```text
AUTH_INVALID
```

### 403

根据 Body：

- quota/balance/billing → `QUOTA_EXHAUSTED`
- permission/forbidden → `AUTH_PERMISSION_DENIED`
- WAF/Cloudflare → `GATEWAY_OR_WAF`
- 无明确证据 → `AUTH_PERMISSION_DENIED`，置信度 medium

### 402

```text
QUOTA_EXHAUSTED
```

### 404

- Body 明确提到模型 → `MODEL_NOT_FOUND`
- 否则 → `ENDPOINT_NOT_FOUND`

### 429

- Body 明确余额/配额 → `QUOTA_EXHAUSTED`
- 否则 → `RATE_LIMITED`

### 2xx + error 字段

不能视为成功。按照 Error Probe 分类。

### 2xx + HTML

```text
GATEWAY_OR_WAF 或 RESPONSE_FORMAT_MISMATCH
```

### 2xx + JSON 但协议字段无法解析

先检查是否含嵌套错误；没有错误后再判：

```text
RESPONSE_FORMAT_MISMATCH
```

只有跨协议对比证据充分时，最终摘要才升级为：

```text
UNSUPPORTED_PROTOCOL
```

---

## CLASS-5：结果卡应展示“可能原因”而不是武断结论

对于低置信度响应格式问题，显示：

```text
未能确认可用协议

可能原因：
- 当前接口路径或协议格式不匹配；
- 上游返回了非标准错误结构；
- Key、权限或余额错误未使用标准 HTTP 状态；
- 中转站返回了网关/WAF 页面。
```

建议：

```text
展开尝试链，重点查看 HTTP 状态、响应摘要和错误分类证据。
```

不能只显示：

```text
UNSUPPORTED_PROTOCOL
请查看尝试链。
```

---

## CLASS-6：诊断证据结构化

`AttemptResult` 建议新增：

```rust
pub struct ErrorEvidence {
    pub source: String,          // http_status / json_path / text_keyword / content_type
    pub code: Option<String>,
    pub message: Option<String>,
    pub matched_keyword: Option<String>,
}

pub struct AttemptResult {
    // existing fields...
    pub classification: String,
    pub confidence: Confidence,
    pub evidence: Vec<ErrorEvidence>,
    pub content_type: Option<String>,
}
```

前端尝试链展示：

```text
403 · AUTH_PERMISSION_DENIED
证据：HTTP 403；error.message 包含 "permission denied"
```

或：

```text
200 · QUOTA_EXHAUSTED
证据：JSON error.code=insufficient_balance
```

---

## CLASS-7：错误分类测试矩阵

必须增加模拟服务器/Fixture 测试：

- 401 标准 OpenAI error；
- 403 permission denied；
- 403 quota；
- 402 payment required；
- 429 insufficient_quota；
- 429 普通 rate limit；
- 200 + `{ "error": ... }`；
- 200 + 非标准 `{ "code": 1008, "msg": "余额不足" }`；
- 200 + HTML Cloudflare；
- 200 + 正常 JSON 但协议字段不匹配；
- 404 model not found；
- 404 endpoint not found；
- 中文错误正文；
- Key 失效但中转返回 200；
- 余额不足但中转返回 403；
- 真正的协议不兼容。

断言：Key、余额和 WAF 场景均不得输出 `UNSUPPORTED_PROTOCOL`。

---

# 六、P2 诊断引擎完善

## P2-1：建立 Provider 级真实请求预算

Planner 数量不等于真实请求数。动态 Token 字段回退也会发送请求。

必须建立真实 Send 预算：

```text
快速验证：按实际模式定义限制
智能诊断：每 Provider 最多 12 次真实 HTTP Send
深度兼容：每 Provider 最多 16 次真实 HTTP Send
```

所有以下请求都计入：

- URL 变体；
- 协议变体；
- 认证变体；
- Token 字段回退；
- Streaming；
- Tool Calling；
- 稳定性复测。

缓存复用不计入真实请求数。

前端显示：

```text
已发送 7 / Provider 上限 12；Host 会话 18 / 30
```

可在高级日志展示，不必占据主界面。

---

## P2-2：非流式响应也必须增量限制大小

不能先 `response.bytes().await` 下载完整正文再判断 2MB。

要求：

- 先检查 `Content-Length`；
- 使用 `bytes_stream()` 分块读取；
- 累计超过 2MB 立即停止；
- 错误正文最多保存限定摘要；
- 不允许异常服务端造成无限内存增长。

---

## P2-3：最终状态选择应使用证据优先级

不能简单选择第一个失败。

建立：

- 分类优先级；
- 置信度；
- 是否为当前配置；
- 是否在修正 URL 后仍复现；
- 是否得到多个尝试支持。

示例：

```text
尝试 1：404 endpoint
尝试 2：401 invalid key
```

最终应优先提示 Key/认证问题，而不是 Endpoint。

---

## P2-4：未配置模型时不能声称“当前配置可用”

当 Provider 没有配置模型时，Doctor 使用默认模型只能标记为推测测试。

要求：

- `is_current_config` 必须准确；
- 猜测模型不能得出 `CURRENT_CONFIG_OK`；
- 状态改为：

```text
MODEL_GUESS_OK
使用推测模型测试成功，不能代表当前配置已完整验证
```

优先方案：

1. 通过 `/models` 或协议原生模型列表获取候选；
2. 用户临时选择模型，内存使用、不持久化；
3. 最后才使用内置推测模型。

---

## P2-5：修复 Gemini `/v1beta` 重复拼接

需要处理：

```text
Base 已含 /v1beta
Base 已含 /v1
Base 已含 /models
Base 已含完整 generateContent endpoint
```

不得产生：

```text
/v1beta/v1beta/models/...
```

使用结构化 URL Path 拼接，不使用简单字符串拼接。

同时测试：

- `x-goog-api-key`；
- Query Key；
- `alt=sse` Query 合并；
- URL 已存在 Query 参数。

---

# 七、P3 CI 与供应链完善

## P3-1：Windows Actions 不要直接执行未校验的 rustup-init.exe

优先使用 Runner 已有 rustup：

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc `
  --profile minimal `
  --component rustfmt `
  --component clippy
rustup default stable-x86_64-pc-windows-msvc
```

如果必须下载 `rustup-init.exe`：

- 固定下载地址；
- 校验固定 SHA-256；
- 校验失败立即终止。

继续保持第三方 Actions 固定到完整 Commit SHA。

---

# 八、UI 结果状态映射

建议状态文案：

| 机器状态                       | 中文主文案                  |
| ------------------------------ | --------------------------- |
| `CURRENT_CONFIG_OK`            | 当前配置可直接使用          |
| `PROTOCOL_FALLBACK_OK`         | 切换协议后可用              |
| `CORRECTED_BASE_PATH_OK`       | 修正接口路径后可用          |
| `AUTH_VARIANT_OK`              | 切换认证方式后可用          |
| `MODEL_VARIANT_OK`             | 更换模型后可用              |
| `LOCAL_ROUTING_REQUIRED`       | 需要 CC Switch 本地路由转换 |
| `AUTH_INVALID` / `KEY_INVALID` | API Key 无效或已失效        |
| `AUTH_PERMISSION_DENIED`       | Key 有效性或权限不足        |
| `QUOTA_EXHAUSTED`              | 余额或额度不足              |
| `RATE_LIMITED`                 | 请求被限流                  |
| `MODEL_NOT_FOUND`              | 模型不存在或无访问权限      |
| `ENDPOINT_NOT_FOUND`           | 接口路径不存在              |
| `GATEWAY_OR_WAF`               | 网关或安全验证页面阻断      |
| `RESPONSE_FORMAT_MISMATCH`     | 返回格式与预期不一致        |
| `UNSUPPORTED_PROTOCOL`         | 未发现兼容的协议组合        |
| `NETWORK_UNREACHABLE`          | 网络不可达                  |
| `TLS_ERROR`                    | TLS 或证书错误              |
| `TIMEOUT`                      | 请求超时                    |
| `HOST_BUDGET_EXHAUSTED`        | 已达到本次 Host 请求上限    |
| `HOST_RATE_LIMIT_STOPPED`      | 连续限流，已停止继续请求    |

主界面优先中文，机器状态放在可展开详情中。

---

# 九、前端布局与交互验收

本次不要求重做现有整体 UI，只调整顶部控制区和结果表达。

必须满足：

- 模式选择器保留现有视觉风格；
- 并发选择器与模式选择器同一行，尺寸一致；
- 当前模式有短说明或 Tooltip；
- Provider 当前配置默认自动选中；
- `UNSUPPORTED_PROTOCOL` 不再作为大号主文案重复出现；
- 结果卡中结论、证据、可能原因和建议层级清晰；
- 运行中模式、并发、刷新、选择 DB 均正确锁定；
- 左右独立滚动不被破坏；
- 1366×768 和 1440×900 截图无挤压和遮挡。

---

# 十、测试与质量门禁

## 前端

必须运行：

```bash
npm run format:check
npm run lint
npm run typecheck
npm run test
npm run build
```

新增测试：

- 并发选择器 1/2/3；
- 运行中并发 Disabled；
- 三模式 Tooltip/Popover；
- 默认选择 CCS 当前 Provider；
- 刷新后重新选择新的 Current；
- 旧 Run 事件忽略；
- 中文状态映射；
- 低置信度协议问题展示可能原因。

## Rust

必须运行：

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

新增测试至少覆盖：

- RequestCacheKey 语义完整性；
- 自定义数据目录 Store；
- Run ID 取消和清理；
- Manifest Schema 门禁；
- Anthropic Token/Header；
- UTF-8 安全截断；
- URL 全面脱敏；
- Provider 和 Host 请求预算；
- Error Probe 分类矩阵；
- 非流式响应体增量限制；
- Gemini Path 拼接。

## 安全门禁

继续运行：

```bash
npm run security:verify
```

并确保：

- 无 Shell Plugin；
- 无 `std::process` / `Command::new`；
- 无保护登录路径读取；
- 无完整 Key 前端序列化；
- 无数据库写入；
- 无跨 Host Credential 转发。

---

# 十一、v0.1.3 Release 要求

版本必须统一更新：

```text
package.json
src-tauri/Cargo.toml
src-tauri/tauri.conf.json
compatibility/manifest.json doctorVersion
```

必须发布：

```text
CC-Switch-Doctor-v0.1.3-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.3-Windows-x64-portable.zip
SHA256SUMS.txt
```

要求：

- Tag：`v0.1.3`；
- 不覆盖旧 Tag；
- Release 为非 Draft、非 Prerelease；
- 当前仍为 unsigned，Release 明确提示 SmartScreen；
- 三个资产大小大于 0；
- `SHA256SUMS.txt` 文件名与资产完全一致；
- Tag 必须指向包含全部修复的最终提交；
- CI 和 Release Workflow 全部成功。

Release Notes 至少写明：

- 修复请求缓存错误复用；
- 正确识别 CC Switch 自定义数据目录；
- 修复取消后旧事件污染；
- 收紧 Schema 兼容门禁；
- 修复 Anthropic Token 认证；
- 强化 Key 脱敏和 UTF-8 安全；
- 恢复可修改并发数；
- 增加三种诊断模式说明；
- 默认选中 CCS 当前 Provider；
- 改进 Key、权限、余额、限流、WAF 与协议错误分类。

---

# 十二、完成定义

只有全部满足以下条件才算完成：

- P0 请求缓存问题已修复并有测试；
- 六项 P1 全部完成；
- 四项新增 UX 要求全部完成；
- `UNSUPPORTED_PROTOCOL` 不再吞掉 Key、余额或 WAF 错误；
- 并发可选择 1/2/3；
- 三种模式用户能在 UI 直接理解；
- CCS 当前第三方 Provider 默认选中；
- 所有前端、Rust、安全测试通过；
- Windows Tauri 构建通过；
- v0.1.3 Release 和三个资产存在；
- Git 工作区干净；
- 不存在仍运行的后台 Shell；
- AI 任务正式结束，不处于 goal active 状态。

---

# 十三、最终汇报格式

AI 完成后必须严格按以下结构汇报：

```markdown
## 完成状态

- v0.1.3：完成 / 未完成
- 最终提交 SHA：
- Tag 指向 SHA：

## P0/P1 修复

- 请求缓存：
- 自定义数据目录：
- Run 隔离：
- Schema 门禁：
- Anthropic 认证：
- UTF-8 截断：
- Key 脱敏：

## UX 修复

- 并发选择：
- 模式说明：
- 默认选中 CCS 当前配置：
- 结果状态中文化：

## 错误分类

- Key/认证：
- 权限：
- 余额/额度：
- 限流：
- WAF/网关：
- UNSUPPORTED_PROTOCOL 判定：

## 测试结果

- format：
- lint：
- typecheck：
- frontend tests：
- frontend build：
- cargo fmt：
- cargo clippy：
- cargo test：
- security verify：
- Tauri Windows build：

## GitHub Actions

- CI：
- Release：
- Upstream Watch：

## Release 资产

- setup.exe：文件名、大小、SHA-256
- portable.zip：文件名、大小、SHA-256
- SHA256SUMS.txt：

## 安全确认

- 未启动 AI CLI：
- 未读取登录目录：
- DB 只读：
- 完整 Key 未进入前端或日志：
- 跨 Host 请求阻断：

## 已知限制

-
```

---

# 十四、可直接粘贴给 AI 的简化任务 Prompt

```text
严格阅读并执行仓库中的《CC Switch Doctor v0.1.3 代码审查、交互与诊断准确性修复任务文档》。

基于当前 main/v0.1.2 定向修复，不要推倒现有 UI 和安全架构。优先修复请求缓存键、自定义 CC Switch 数据目录、Run 事件隔离、Schema 门禁、Anthropic 认证、UTF-8 截断和 Key 脱敏。

同时完成以下用户体验要求：
1. 并发数重新展示并可选择 1/2/3，默认 1，运行中锁定；
2. 在 UI 内解释快速验证、智能诊断、深度兼容的具体区别、耗时和 Token 消耗；
3. 每次扫描后默认选中 CC Switch 当前且可测试的第三方 Provider；
4. 重做错误分类，UNSUPPORTED_PROTOCOL 只能作为排除性结论，不得掩盖 Key 无效、权限不足、余额不足、限流、WAF、模型或端点错误。

补齐全部测试和安全门禁，完成 Windows 构建，发布 v0.1.3 GitHub Release 与 setup.exe、portable.zip、SHA256SUMS.txt。远程 CI 或 Release 失败时继续修复，不能提前结束。
```
