# CC Switch Doctor 页面线框与组件实施规范

> 文档版本：1.0  
> 适用范围：Windows 10/11 x64，CC Switch Doctor v0.1.x  
> 本文是可执行 UI 实施规范，不是仅供参考的设计建议。  
> 与旧 UI 需求冲突时，以本文件和 `UI_UX_ADDENDUM.md` 为准。

---

# 1. 设计目标

把当前“功能可用但像调试页”的界面重构为：

- 轻量、现代、专业的桌面诊断工具；
- 视觉语言接近 CC Switch，具备同生态 Companion Tool 感；
- 用户打开后能迅速完成“选配置 → 选模式 → 开始诊断 → 看结论”；
- 普通用户先看到结论和建议，高级用户再展开尝试链和原始日志；
- 长 URL、模型名、错误信息不破坏版式；
- 多 Provider 结果易扫描、易比较、易复制。

设计关键词：

```text
Clean / Calm / Structured / Compact / Readable / Trustworthy
```

---

# 2. 窗口与总体结构

## 2.1 窗口规格

建议默认窗口：

```text
1440 × 900
```

建议最小窗口：

```text
1180 × 720
```

要求：

- 低于最小尺寸时阻止继续缩小，或确保主流程仍可用；
- 主页面本身不产生一个贯穿全部内容的长滚动条；
- Provider 区和结果区分别独立滚动；
- 顶部栏和测试控制栏固定，不随列表滚走；
- Windows 标题栏可使用原生标题栏，界面内部不重复绘制夸张标题栏。

## 2.2 页面区域

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ A. App Header / Status Toolbar                                      64-76px │
├────────────────────────────────────────────────────────────────────────────┤
│ B. Session Control Bar                                             56-68px │
├──────────────────────────────────────┬─────────────────────────────────────┤
│ C. Provider Workspace               │ D. Diagnosis Workspace              │
│ 58%（允许 54%-62%）                  │ 42%（允许 38%-46%）                 │
│ 独立滚动                             │ 独立滚动                            │
└──────────────────────────────────────┴─────────────────────────────────────┘
```

建议主内容容器：

```text
padding: 16px
column-gap: 14px
```

左右面板使用白色/轻染色卡片背景，边框柔和，圆角一致。

---

# 3. 主页面线框

## 3.1 默认连接成功、尚未选择

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  ⛑ CC Switch Doctor  v0.1.x       ● DB 已连接  Schema 已验证   [安全边界]  │
│  只读扫描 · 纯 HTTP · 不启动 AI CLI              [检查更新] [刷新] [选择DB] │
├──────────────────────────────────────────────────────────────────────────────┤
│ 模式 [快速验证 | 智能诊断● | 深度兼容]  已选 0  预计 0 请求  [开始诊断]    │
├───────────────────────────────────────┬──────────────────────────────────────┤
│ Provider 配置                         │ 诊断结果                             │
│ [全部][Claude][Codex][Gemini]...      │                                      │
│ [🔎 搜索供应商 / Host / 模型......]   │              ◇                       │
│ □ 仅看已选  [全选筛选] [取消全选]    │   选择左侧配置并开始诊断              │
│                                       │   这里将展示结论、建议和尝试链        │
│ ┌───────────────────────────────────┐ │                                      │
│ │□ Claude Code   DeepSeek     当前 │ │                                      │
│ │  sk-ce7…b100                     │ │                                      │
│ │  api.deepseek.com                │ │                                      │
│ │  deepseek-v4-pro   Anthropic Msg │ │                                      │
│ └───────────────────────────────────┘ │                                      │
│ ...                                   │                                      │
└───────────────────────────────────────┴──────────────────────────────────────┘
```

核心要求：

- Header 紧凑，安全说明不占据整个首屏；
- “开始诊断”为唯一主色强按钮；
- 空状态不是灰色输入框，也不是原始日志框；
- Provider 行不是硬表格，内容有清晰层级。

## 3.2 已选择、准备诊断

```text
模式 [快速验证 | 智能诊断● | 深度兼容]
已选 4 个配置 · 预计最多 31 个请求 · 并发 1
[开始诊断]
```

要求：

- 主按钮随选择状态启用；
- 显示请求预算；
- Hover/Tooltip 解释“预计最多”是上限，不一定全部发送；
- 官方/OAuth 配置不计入已选数量。

## 3.3 诊断运行中

```text
正在诊断 2 / 4  ·  linuxdo-20260720-1955
████████░░░░░░  50%     已发送 7 / 31 请求       [停止]
```

左侧 Provider 状态：

```text
等待中 / 运行中 / 已成功 / 有建议 / 失败 / 已跳过
```

右侧：

- 已完成结果立即显示；
- 当前运行项显示 skeleton 或轻量 loading；
- 不把实时每一行日志直接滚到用户面前；
- “高级调试日志”可在会话底部手动展开。

## 3.4 混合结果

右侧顶部显示摘要：

```text
本次诊断完成
2 当前配置可用 · 1 可通过协议变体使用 · 1 未发现可用组合
```

下方按优先级排序：

1. 需要用户修改的结果；
2. 失败结果；
3. 当前配置正常；
4. 已跳过。

可提供筛选：

```text
[全部] [需调整] [失败] [正常] [跳过]
```

---

# 4. 顶部 AppHeader

## 4.1 结构

组件名：

```text
<AppHeader />
```

左区：

- 产品图标：18~22px；
- 标题：`CC Switch Doctor`；
- 版本 Badge：`v0.1.x`；
- 次级一句话：`只读扫描 · 纯 HTTP · 不启动 AI CLI`。

中/右区状态：

- `DB 已连接` / `未连接`；
- `Schema 已验证` / `兼容` / `未知`；
- CC Switch 版本仅在可靠识别时显示；
- `安全边界` 文本按钮。

右侧操作：

- 检查更新；
- 刷新配置；
- 选择 DB。

## 4.2 主次规则

这些均为次级按钮，不使用主色实心大按钮。

建议按钮形式：

```text
icon + label
height 34-36px
```

窄窗口可仅保留图标并用 Tooltip 展示文字。

## 4.3 安全边界 Drawer

点击 `安全边界` 打开右侧 Drawer 或 Modal：

内容分组：

- 会做什么；
- 绝不会做什么；
- Key 如何处理；
- HTTP 测试能证明什么；
- HTTP 测试不能证明什么。

允许勾选：

```text
本次会话不再显示安全提示
```

只能保存在内存，关闭应用后恢复默认。

---

# 5. SessionControlBar

组件名：

```text
<SessionControlBar />
```

## 5.1 左侧：模式选择

使用 SegmentedControl：

```text
快速验证 | 智能诊断 | 深度兼容
```

每个模式有 Tooltip：

- 快速：只测当前配置；
- 智能：失败时尝试受控变体；
- 深度：额外测试流式与 Tool Calling，消耗更多请求。

默认选中智能诊断。

## 5.2 中部：预算摘要

显示：

```text
已选 N
预计最多 M 请求
并发 1
```

运行中改为：

```text
完成 X / N
请求 Y / M
当前：Provider Name
```

## 5.3 右侧：主操作

状态矩阵：

| 状态 | 主按钮 |
|---|---|
| 未选择 | `开始诊断` disabled |
| 已选择 | `开始诊断` enabled |
| 运行中 | `停止` secondary |
| 已完成 | `重新诊断` primary |

主按钮高度 38~40px，视觉上全页面最明显。

---

# 6. ProviderWorkspace

组件名：

```text
<ProviderWorkspace />
```

内部固定头部 + 独立滚动列表。

## 6.1 FilterChips

组件：

```text
<AppFilterChips />
```

Chips：

```text
全部 / Claude / Claude Desktop / Codex / Gemini / OpenCode / OpenClaw / Hermes / Grok
```

只显示当前数据中存在的 app type；全部始终显示。

要求：

- 选中为浅主色底 + 主色文字；
- 未选中白底/透明 + 中性文字；
- 不要使用粗黑边；
- Chips 多时可自然换行，行距统一；
- 不出现一半高一半低。

## 6.2 SearchAndBulkActions

一行：

```text
[🔎 搜索供应商 / Host / 模型................] [仅看已选] [···]
```

批量操作可放在 `···` 菜单中：

- 全选当前筛选；
- 取消全选；
- 只选当前 Provider；
- 清除搜索。

避免多个次级按钮横向抢空间。

## 6.3 ProviderList

组件：

```text
<ProviderList />
<ProviderRow />
```

ProviderRow 建议使用 CSS Grid：

```text
40px  minmax(180px, 1.25fr)  minmax(180px, 1.15fr)  minmax(150px, 1fr)  96px
```

窗口变窄时切换为两层卡片布局，不强行压缩成碎字。

### ProviderRow 第一视觉层

- Checkbox；
- Provider Name；
- 当前 Badge；
- 行尾状态 Badge。

### 第二视觉层

- App type；
- masked key；
- Host/Base URL；
- Model；
- Protocol。

### 推荐卡片式布局

```text
┌──────────────────────────────────────────────────────────────┐
│ □  DeepSeek                              [当前] [可诊断]       │
│    Claude Code · sk-ce7…b100                                  │
│    api.deepseek.com                    deepseek-v4-pro         │
│    Anthropic Messages                                          │
└──────────────────────────────────────────────────────────────┘
```

或精致 grid list，但视觉必须达到同等层级。

## 6.4 长文本处理

### Base URL

列表只显示：

- 优先 Host；
- 必要时显示简短 path；
- 单行 ellipsis；
- Tooltip 显示脱敏完整 URL；
- 点击复制显示 toast。

CSS 规则倾向：

```css
white-space: nowrap;
overflow: hidden;
text-overflow: ellipsis;
```

不得在列表中使用 `word-break: break-all`。

### Model

- 单行或最多两行；
- 使用正常词边界；
- `overflow-wrap: anywhere` 只在无自然断点且详情区域需要时使用；
- 列表优先 ellipsis。

### Key

仅显示后端生成的 masked string，不允许前端自行遮罩完整 Key。

## 6.5 Row 状态

必须覆盖：

- default；
- hover；
- selected；
- active/focused；
- disabled/managed；
- running；
- success；
- recommendation；
- failed；
- skipped。

Selected 采用轻主色背景或左侧强调线；不要用强烈整行蓝色。

Managed/OAuth 行：

- Checkbox disabled；
- 锁/盾牌图标；
- Badge：`官方登录，已跳过`；
- Tooltip 解释不会测试订阅登录态。

---

# 7. DiagnosisWorkspace

组件：

```text
<DiagnosisWorkspace />
```

## 7.1 EmptyState

组件：

```text
<DiagnosisEmptyState />
```

内容：

- 简洁线性图标；
- 标题：`尚未开始诊断`；
- 说明：`选择左侧配置并点击“开始诊断”，这里会展示结构化结论、建议修改项与尝试链。`；
- 不放假输入框；
- 不放原始日志占位。

## 7.2 SessionSummary

诊断后顶部固定摘要卡：

```text
<SessionSummary />
```

展示：

- 完成数；
- 正常数；
- 需调整数；
- 失败数；
- 跳过数；
- 总请求数和耗时；
- 结果筛选 Chips。

## 7.3 ResultList

组件：

```text
<ResultList />
<ResultCard />
```

卡片间距 12~14px。

默认展开：

- 标题；
- 状态；
- 一句话结论；
- 建议；
- 成功组合/主要失败原因。

默认折叠：

- 尝试链；
- 技术详情；
- 高级日志。

---

# 8. ResultCard 详细线框

## 8.1 当前配置成功

```text
┌──────────────────────────────────────────────────────────────┐
│ Claude Code / linuxdo-20260718-2209        [当前配置可用]     │
│ subgrok.example.com                                           │
│                                                              │
│ 当前配置可以正常调用。                                        │
│ 已验证：上游 Anthropic Messages · grok-4.5                    │
│                                                              │
│ 成功组合                                                      │
│ Base URL   https://subgrok.example.com/v1                     │
│ Endpoint   /messages                                          │
│ 协议       Anthropic Messages                                 │
│ 模型       grok-4.5                                           │
│ 耗时       758 ms                                             │
│                                                              │
│ [复制摘要] [展开尝试链 1]                                     │
└──────────────────────────────────────────────────────────────┘
```

## 8.2 协议回退成功

```text
┌──────────────────────────────────────────────────────────────┐
│ Claude Code / linuxdo-20260720-1955         [发现可用变体]    │
│ sub.example.com                                               │
│                                                              │
│ 当前协议未直接成功，但同一供应商、Key 和模型可通过             │
│ OpenAI Chat Completions 正常调用。                             │
│                                                              │
│ 建议在 CC Switch 中检查：                                     │
│ • API 格式改为 OpenAI Chat Completions                        │
│ • Base URL 使用 https://sub.example.com/v1                    │
│ • Codex 场景可能需要 Local Routing（仅推断，未端到端验证）     │
│                                                              │
│ [复制建议] [复制摘要] [展开尝试链 3]                           │
└──────────────────────────────────────────────────────────────┘
```

“仅推断”必须使用独立 EvidenceBadge，不得与“已验证”混淆。

## 8.3 失败

```text
┌──────────────────────────────────────────────────────────────┐
│ Claude Code / DouBaoSeed                    [未发现可用组合]   │
│ ark.cn-beijing.volces.com                                    │
│                                                              │
│ 当前协议不兼容，已在同一 Host 内尝试 9 个受控组合，             │
│ 暂未找到可稳定调用的协议和端点。                               │
│                                                              │
│ 主要证据                                                      │
│ • Anthropic Messages：404 Unsupported Protocol               │
│ • OpenAI Chat：403 Permission Denied                          │
│                                                              │
│ 建议                                                          │
│ 检查该 Key 的产品类型、允许的模型和官方接入文档。              │
│                                                              │
│ [复制摘要] [展开尝试链 9] [技术详情]                           │
└──────────────────────────────────────────────────────────────┘
```

不能只显示 `UNSUPPORTED_PROTOCOL` 大写枚举。

---

# 9. AttemptTimeline

组件：

```text
<AttemptTimeline />
```

使用 Accordion 内的垂直时间线或编号列表。

每个 Attempt 展示：

- 编号；
- 变体类型：当前配置 / URL 修正 / 协议回退 / 模型候选 / 参数回退；
- 协议；
- 脱敏 URL；
- 模型；
- HTTP 状态；
- 结果分类；
- 耗时；
- 是否消耗生成 Token；
- 简短脱敏错误摘要。

示例：

```text
1  当前配置
   Anthropic Messages · grok-4.5
   https://sub.example.com/v1/messages
   403 PERMISSION_DENIED · 758 ms

2  协议回退
   OpenAI Chat Completions · grok-4.5
   https://sub.example.com/v1/chat/completions
   成功 · 1087 ms
```

禁止：

- 默认展开所有 attempts；
- 把每个 JSON 响应原文直接展示；
- 展示请求 Header；
- 展示完整 Key；
- 使用整块不可读的 console dump。

---

# 10. AdvancedLogPanel

组件：

```text
<AdvancedLogPanel />
```

名称：

```text
调试日志（高级）
```

要求：

- 默认折叠；
- 明确提示日志已脱敏；
- 等宽字体；
- 固定最大高度；
- 独立纵向和必要的横向滚动；
- 支持复制脱敏日志；
- 不自动跟随滚动干扰用户；
- 不在首页顶部占据空间。

建议样式：浅中性背景，不使用纯黑终端主题破坏整体风格。

---

# 11. 状态与颜色

建立单一 `diagnosticStatusMap`，前后端枚举映射到统一 UI 语义。

| 机器状态 | 中文 Badge | 视觉语义 |
|---|---|---|
| `CURRENT_CONFIG_OK` | 当前配置可用 | success |
| `PROTOCOL_FALLBACK_OK` | 发现可用协议 | info-success |
| `URL_FALLBACK_OK` | 修正地址后可用 | info-success |
| `MODEL_FALLBACK_OK` | 更换模型后可用 | info |
| `AUTH_FAILED` | 认证失败 | danger |
| `PERMISSION_DENIED` | 权限不足 | danger |
| `RATE_LIMITED` | 已限流 | warning |
| `BALANCE_OR_QUOTA` | 额度或配额异常 | warning |
| `UNSUPPORTED_PROTOCOL` | 协议不兼容 | warning |
| `MODEL_NOT_FOUND` | 模型不可用 | warning |
| `HOST_UNREACHABLE` | 地址不可达 | danger |
| `TLS_ERROR` | TLS 连接失败 | danger |
| `OFFICIAL_SKIPPED` | 官方登录已跳过 | neutral |
| `UNKNOWN_SCHEMA` | 未知 Schema | special-neutral |
| `CANCELLED` | 已停止 | neutral |

Badge 使用：

- 浅色背景；
- 中等饱和文字；
- 1px 同色系边框；
- 配合图标或文字，不仅靠颜色。

---

# 12. EvidenceBadge

组件：

```text
<EvidenceBadge type="verified|inferred|not-tested" />
```

文案：

- `上游已验证`
- `配置建议，端到端未验证`
- `未测试`

用于解决以下误导风险：

```text
供应商 HTTP 成功 ≠ Codex/Claude Code 完整 Agent 流程已成功
```

所有涉及 CC Switch Local Routing、真实 CLI 行为的结论必须标为 inferred。

---

# 13. Design Tokens

建议使用 CSS Variables。

## 13.1 尺寸

```css
--radius-sm: 8px;
--radius-md: 12px;
--radius-lg: 16px;
--control-height-sm: 32px;
--control-height-md: 36px;
--control-height-lg: 40px;
--space-1: 4px;
--space-2: 8px;
--space-3: 12px;
--space-4: 16px;
--space-5: 20px;
--space-6: 24px;
```

## 13.2 字体

```css
font-family: Inter, "Segoe UI", "Microsoft YaHei UI", system-ui, sans-serif;
```

字号建议：

```text
App title       18-20px / 600
Panel title     16-18px / 600
Card title      15-16px / 600
Body            13-14px / 400
Secondary       12-13px / 400
Badge           11-12px / 500
Mono log        12px / 400
```

正文行高 1.5~1.6。

## 13.3 颜色方向

不要硬编码分散色值；使用 token：

```text
background
surface
surface-subtle
border
border-strong
text
text-secondary
text-muted
primary
primary-subtle
success
success-subtle
warning
warning-subtle
danger
danger-subtle
info
info-subtle
focus-ring
```

浅色主题：

- 页面背景：极浅灰蓝；
- Surface：白色；
- Border：低饱和灰蓝；
- Primary：CC Switch 感的柔和蓝；
- 不使用大面积高饱和纯色。

## 13.4 阴影

仅轻阴影：

```text
panel: 0 1px 2px rgba(...)
floating: 0 8px 24px rgba(...)
```

不要所有卡片都使用重阴影。

---

# 14. 基础组件清单

至少抽离：

```text
AppHeader
StatusChip
EvidenceBadge
SegmentedControl
SearchInput
FilterChip
PrimaryButton
SecondaryButton
GhostButton
IconButton
ProviderWorkspace
ProviderList
ProviderRow
ProviderRowSkeleton
SessionControlBar
ProgressSummary
DiagnosisWorkspace
DiagnosisEmptyState
SessionSummary
ResultFilterChips
ResultList
ResultCard
SuccessCombinationGrid
RecommendationBlock
AttemptTimeline
AdvancedLogPanel
CopyButton
Tooltip
Toast
Dialog
Drawer
Accordion
EmptyState
ErrorState
```

组件要求：

- 不重复散落样式；
- props 明确；
- 支持键盘与焦点；
- 语义状态由统一映射生成；
- 复制操作统一反馈；
- Tooltip 延迟与样式一致。

---

# 15. 页面状态线框

## 15.1 DB 未找到

右侧/主内容显示专用 EmptyState：

```text
未找到 CC Switch 数据库
Doctor 未在默认位置发现 cc-switch.db。
[选择 cc-switch.db]
```

附次级说明：

```text
所选路径只在本次会话中使用，不会保存。
```

不要显示红色灾难错误。

## 15.2 Schema 未知

显示明确安全停止页：

```text
当前 CC Switch 数据结构尚未验证
为避免错误读取凭据，Doctor 已停止供应商测试。

检测到的 Schema 指纹：<非敏感摘要>
支持范围：<manifest 摘要>

[复制兼容性信息] [检查 Doctor 更新]
```

不得提供“强制继续”按钮。

## 15.3 更新可用

使用非阻塞 Banner/Modal：

```text
发现 CC Switch 新版本，当前 Doctor 尚未验证其数据结构。
```

根据风险分级：

- Release 新但 schema 未变：提示检查，不阻断；
- schema 指纹未知：阻断测试；
- Doctor 自身有新版本：提供复制 Release 地址，不自动启动浏览器。

## 15.4 请求被停止

结果保留当前会话内已完成内容，顶部显示：

```text
诊断已停止 · 已完成 2 / 5
```

未完成项标为 `未测试`，不要标为失败。

---

# 16. Toast 与反馈

统一 Toast：

- `已复制诊断摘要`
- `已刷新配置`
- `数据库内容已变化`
- `诊断已停止`
- `无法读取数据库`

位置：右下角或右上角，不遮挡主按钮和结果标题。

复制按钮：

```text
复制摘要 → 已复制 ✓ → 1.5 秒后恢复
```

---

# 17. 滚动与性能

- Header 固定；
- SessionControlBar 固定；
- Provider list 独立滚动；
- Result list 独立滚动；
- ResultCard 内日志区独立滚动；
- 切换选中 Provider 时尽量保持列表位置；
- 大量 Provider 时考虑虚拟列表，但首版以稳定为优先；
- 不要在每个流式 token 到达时引发全页面重渲染；
- 实时日志采用节流批量更新；
- 诊断结果完成后再生成完整卡片内容。

---

# 18. 键盘与可访问性

最低要求：

- Tab 顺序合理；
- Space 勾选 Provider；
- Enter 触发主操作；
- Esc 关闭 Dialog/Drawer；
- Focus ring 清晰；
- Icon-only button 有 aria-label；
- 状态不仅靠颜色；
- Tooltip 不阻碍键盘用户；
- Checkbox 与 label 正确关联。

可选快捷键：

```text
Ctrl+F 聚焦搜索
Ctrl+R 刷新配置（避免与 WebView 刷新冲突时可不实现）
Ctrl+Enter 开始诊断
Esc 停止/关闭弹层（需避免误停止）
```

---

# 19. UI Fixture 与截图验收

建立仅开发/测试使用的 synthetic UI fixture，不得从真实 DB 生成截图。

Fixture 至少包含：

1. 当前配置成功；
2. URL 修正成功；
3. 协议回退成功；
4. 模型不可用；
5. 401/403；
6. 429；
7. 官方 Provider 跳过；
8. 未知 schema；
9. 长 Provider 名；
10. 长 Host、长模型名、中文和英文混排。

必须输出截图：

```text
docs/screenshots/main-empty.png
docs/screenshots/main-selected.png
docs/screenshots/diagnosing.png
docs/screenshots/results-mixed.png
docs/screenshots/schema-unknown.png
```

截图标准：

- 1440×900 主基线；
- 1366×768 额外布局测试；
- 不出现真实 Key/URL/用户名；
- 不出现水平页面溢出；
- 不出现 URL 字符级乱换行；
- 不出现按钮文字被截断；
- 不出现右侧结果每几个汉字换一行；
- 日志默认折叠；
- 主按钮明显；
- 截图第一眼具有产品感而非调试页感。

---

# 20. UI 自动验收建议

Playwright 检查：

- viewport 1440×900 和 1366×768；
- 页面无全局横向 overflow；
- AppHeader 和 SessionControlBar 可见；
- 开始诊断按钮主操作状态正确；
- ProviderRow 长文本有 ellipsis；
- Managed Provider checkbox disabled；
- EmptyState 文案存在；
- ResultCard 结论和建议存在；
- AttemptTimeline 默认折叠；
- AdvancedLogPanel 默认折叠；
- Copy 后出现反馈；
- 诊断中进度状态存在；
- 停止后未完成项为未测试；
- Schema unknown 页面没有强制测试按钮。

可加入 screenshot assertions，但应控制动态内容和字体导致的脆弱性。

---

# 21. 文案规范

优先自然中文，技术枚举作为辅助。

错误：

```text
UNSUPPORTED_PROTOCOL，请查看错误摘要。
```

正确：

```text
当前协议不兼容。
Doctor 已在同一 Host 内尝试受控协议组合，但暂未发现可用方案。
```

ResultCard 文案顺序：

1. 用户可理解的结论；
2. 建议动作；
3. 已验证证据；
4. 技术状态码；
5. 尝试链。

不要把工程枚举作为标题主体。

---

# 22. 不允许的实现

- 不允许继续使用当前截图中“整块日志文本框置顶”的布局；
- 不允许首页大面积安全说明占据首屏；
- 不允许用 HTML table 默认样式直接交付；
- 不允许 URL 在列表中逐字符换行；
- 不允许所有按钮同等视觉权重；
- 不允许 ResultCard 只显示枚举码和长段工程文字；
- 不允许原始日志默认展开；
- 不允许多个区域共享页面总滚动条；
- 不允许为模仿 CC Switch 复制其私有资产或未经许可代码；
- 不允许 UI fixture 含真实供应商信息和真实 Key。

---

# 23. UI Definition of Done

- [ ] 顶部区域紧凑、清晰；
- [ ] 安全说明进入 Drawer/Modal；
- [ ] 测试模式、预算和主按钮位于固定控制栏；
- [ ] Provider 列表完成产品化重构；
- [ ] URL、模型、Key 摘要排版清晰；
- [ ] Provider 与 Result 独立滚动；
- [ ] 右侧空状态友好；
- [ ] ResultCard 层级完整；
- [ ] 建议动作显著；
- [ ] EvidenceBadge 区分已验证和推断；
- [ ] AttemptTimeline 默认折叠；
- [ ] AdvancedLogPanel 默认折叠；
- [ ] 状态颜色和文案统一；
- [ ] Toast 和复制反馈统一；
- [ ] 1440×900 截图完整；
- [ ] 1366×768 无关键溢出；
- [ ] Playwright UI 验收通过；
- [ ] README 展示新界面截图；
- [ ] 审查者第一眼不再认为它是调试原型。
