# CC Switch Doctor v0.1.10 Provider 与诊断结果紧凑化回归安全修复规范

> 目标版本：v0.1.10  
> 基线：当前 `main / v0.1.9`  
> 审查提交：`5a1e81ecde3b56059a27f9c7dba8aed22bea5832`  
> 类型：纯前端定向修复  
> 原则：**只清理重复交互、压缩信息密度、补齐回归测试；不扩展协议、模型、路由、数据库或诊断能力。**

---

## 1. 最新仓库审查结论

当前代码中：

- `ProviderRow` 的整张卡片点击会调用 `onActivate`。
- 卡片底部“查看详情”按钮也只调用同一个 `onActivate`。
- 未诊断时右侧没有对应 Result，按钮没有实际目标。
- 普通 Provider 卡片仍有独立 Footer 行，且 CSS `min-height: 96px`。
- `ResultCard` 已在 Header 显示 Primary Badge，却又重复显示“诊断结论 + 同一文案”。
- Route 未执行时，Header 的“路由未验证”和独立 Route 大框重复表达同一信息。
- `DiagnosisWorkspace` 在 ResultCard 外层绑定整体点击；点击复制、Accordion 等内部控件时，可能同时触发左侧 Provider 定位。

本轮应删除重复内容并重排现有信息，不删除诊断证据，也不新增复杂功能。

---

## 2. 开发边界

### 允许修改

```text
src/components/ProviderRow.tsx
src/components/ProviderWorkspace.tsx
src/components/DiagnosisWorkspace.tsx
src/components/ResultCard.tsx
src/styles/index.css
src/styles/tokens.css（仅必要的现有密度 Token）
tests/ 对应前端测试
版本、CHANGELOG、Release Notes
```

必要时可在 `src/lib/utils.ts` 增加极少量 UI 辅助函数。

### 禁止修改

```text
src-tauri/src/diagnostics/*
src-tauri/src/protocols/*
src-tauri/src/ccs_adapter/*
src-tauri/src/security/*
路由、模型、错误分类、Schema、SQLite、请求预算
```

禁止新增：

- 新协议、模型规则、路由能力；
- 新设置持久化；
- 新 UI 框架或状态管理库；
- Provider 编辑能力；
- 详情抽屉、右键菜单或虚拟列表库；
- 无关重构。

---

## 3. 冻结功能

必须保留：

- 默认筛选 Claude。
- Provider 默认不勾选。
- Checkbox 只负责选择。
- Provider 与 Result 双向定位。
- 三点菜单外部点击、Esc、菜单项执行后关闭。
- Result 筛选、上一条、下一条、选择器。
- Primary / Direct / Route 数据语义。
- 路由辅助状态不覆盖主结果。
- 模型语义、成功组合、建议、Evidence、尝试链和调试日志。
- v0.1.9 `[1M]` 归一化及模型错误分类。
- v0.1.8 Schema Capability 架构。
- SQLite 只读、CLI 隔离、Key 脱敏、请求预算。

---

## 4. Bug：删除 Provider“查看详情”

### 当前问题

Provider 卡片和“查看详情”按钮执行同一操作。未诊断时没有结果可跳转，按钮无效且容易被理解成“查看 Provider 配置详情”。

### 修改要求

删除可见的“查看详情”按钮，不改名保留。

未诊断 Provider：

- 点击内容不触发跳转；
- 不设置 activeId；
- 不显示 Pointer Cursor；
- Checkbox 正常工作。

已诊断 Provider：

- 点击卡片内容区域跳转右侧结果；
- Checkbox 不跳转；
- Enter / Space 可跳转；
- 有清晰 Focus 样式；
- 提供 `aria-label="查看 <Provider> 的诊断结果"`。

推荐让 `.provider-card-body` 承担可访问的跳转语义，Checkbox 保持为独立兄弟元素，避免把含 Checkbox 的整张卡片改成 `<button>`。

`ProviderRow` 增加：

```ts
hasResult: boolean;
```

由：

```ts
statusById.has(provider.opaqueId);
```

提供，不能用“是否被勾选”代替。

---

## 5. Provider 三行紧凑布局

普通 Provider 保留全部必要信息，但合并为三层：

```text
第一行：Provider 名称 + 当前标记                  Primary Badge
第二行：App · Masked Key · Protocol
第三行：Host                                      Model
```

删除独立 Footer 行。

不得删除：

- Provider 名称；
- App；
- Masked Key；
- Protocol；
- Host；
- Model；
- Primary Status；
- 当前标记。

不可诊断 Provider 的 Skip Reason：

- 继续保留；
- 默认最多两行；
- 完整内容放 `title`；
- 允许卡片自然增高。

建议 CSS：

```css
.provider-card {
  min-height: 78px;
  padding: 7px 9px;
  margin-bottom: 4px;
}
```

可在 `76px～84px` 微调，不得靠全局缩小字体解决。

验收：

- 1100×740：至少显示 5 个普通 Provider；
- 960×640：至少显示 4 个普通 Provider。

---

## 6. Bug：ResultCard 主结论重复

当前 Header Badge 已显示：

```text
请求被限流
```

后面又显示：

```text
诊断结论
请求被限流
```

删除独立的“诊断结论 + 重复正文”。

Header 统一展示：

```text
Claude Code / Provider       [请求被限流] [可信度：低]
Host
```

Primary Status 只展示一次，Status Code 继续保留在 Tooltip。

---

## 7. Direct / Route 紧凑摘要

### 普通场景

Route 未真实尝试、没有复杂 Route Evidence 时，不再渲染两个大边框卡片。

改为一条摘要：

```text
直连：请求被限流 · 路由：未验证（CCS 未运行）
```

Header 不再额外重复显示“路由未验证”。

以下 Route Disposition 只显示摘要：

```text
not_requested
not_configured
not_running
not_current_target
unsupported_app
blocked_non_loopback
```

### 何时显示 Route 详细区

仅当存在以下任一项：

- 真实发送 CCS 路由请求；
- Actual Provider；
- Failover Before/After；
- Route Target Mismatch；
- Route Side Effect Notice；
- Generate 与 Streaming 结果不同。

普通 Direct 状态也只放在摘要行；Native/CrossProtocol、模型转换、URL/协议/Auth 变体等详细证据仍放现有对应区块。

---

## 8. Confidence 布局

保留 Confidence，不修改后端算法。

要求：

- 与 Primary Badge 在同一 Header Meta 区；
- 不独占一行；
- 文案可使用 `可信度：低`；
- 低、中保持明显，高可用中性样式；
- Tooltip 说明含义。

---

## 9. ResultCard 其他压缩

### Evidence Tag

避免和 Primary 重复。

失败时可显示：

```text
真实请求 1 · 未发现成功组合
```

而不是再次重复“请求被限流”。

### 建议区

保留完整建议，只压缩：

```css
padding: 6px 8px;
margin-bottom: 5px;
```

不新增“展开建议”状态。

### 模型语义

v0.1.9 内容必须保留，可在明确时合并：

```text
模型：GLM-5.2[1M] → GLM-5.2（[1M] 归一化）
```

### 成功组合

可压缩为：

```text
成功组合：Anthropic Messages · GLM-5.2 · /v1/messages
```

长 URL 继续 Ellipsis + Title。

### 折叠区域

以下继续默认折叠：

- 判定依据；
- 尝试链；
- 调试日志。

---

## 10. 隐藏交互 Bug：内部控件触发跨栏跳转

当前 ResultCard 外层整体绑定 `onClick={() => jumpTo(...)}`。内部有：

- 复制摘要；
- 复制建议；
- `<summary>` 判定依据；
- `<summary>` 尝试链；
- `<summary>` 调试日志。

点击这些控件可能同时触发左侧定位和滚动。

推荐统一过滤交互目标：

```ts
function isInteractiveTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    !!target.closest("button, a, input, select, textarea, summary, details, [role='button']")
  );
}
```

外层：

```tsx
onClick={(event) => {
  if (isInteractiveTarget(event.target)) return;
  jumpTo(summary.opaqueId);
}}
```

不要给每个按钮散落添加 `stopPropagation()`，避免以后遗漏。

---

## 11. 过滤与定位边界

需要覆盖：

1. Provider 有结果且当前右侧筛选可见：正常跳转。
2. Provider 有结果但被右侧筛选隐藏：显式点击 Provider 后，应让该结果可见并定位，不能无反应。
3. Provider 没有结果：不跳转。
4. 点击 ResultCard 非交互区域：左侧定位。
5. 点击复制、Select、Accordion：不定位。
6. 刷新数据库后 activeId 清空。
7. 重新诊断后联动继续基于 opaqueId。

保持局部实现，不引入全局状态库。

---

## 12. 视觉密度目标

普通 Provider：

```text
76～84px
```

简单 ResultCard（无 Route Attempt、无模型转换详情）：

```text
110～150px
```

默认 1100×740：

- 左侧至少 5 个普通 Provider；
- 右侧至少 3 个简单结果摘要。

960×640：

- 左侧至少 4 个普通 Provider；
- 右侧至少 2 个简单结果摘要。

不得修改整体左右列比例、品牌色、字体族、窗口尺寸和 Header 主结构。

---

## 13. 必须新增测试

### ProviderRow

未诊断：

```text
不显示“查看详情”
无 Button Role
点击内容不调用 onActivate
Checkbox 调用 onToggle
```

已诊断：

```text
不显示“查看详情”
内容区域可访问
点击调用 onActivate
Enter / Space 调用 onActivate
Checkbox 不调用 onActivate
```

### ResultCard 去重

`RATE_LIMITED`：

```text
Primary 文案只出现一次
不渲染“诊断结论”重复区
Direct / Route 使用摘要
not_running 不产生大型 Route 卡片
```

Route Attempted：

```text
基础推理仍显示
Streaming 仍显示
Actual Provider / Failover 仍显示
```

### 事件冒泡

```text
点击结果空白区域 → 激活 Provider
点击复制摘要 → 不激活
点击复制建议 → 不激活
展开判定依据 → 不激活
展开尝试链 → 不激活
展开调试日志 → 不激活
```

### 过滤联动

```text
目标结果被筛选隐藏
点击左侧 Provider
→ 结果恢复可见并定位
```

### 冻结回归

```text
默认 Claude
Provider 默认未勾选
三点菜单关闭
上一条 / 下一条
Primary 不被 Route 覆盖
模型语义显示
完整 Key 不进入 DOM
```

---

## 14. 推荐提交顺序

```text
1. fix(provider-ui): remove redundant details action and gate result navigation
2. style(provider-ui): compact provider rows without hiding metadata
3. fix(result-ui): remove duplicated conclusion and compact channel summary
4. fix(result-nav): prevent controls from triggering provider jump
5. test(v0.1.10): add density and interaction regressions
6. release: prepare v0.1.10
```

不要压成一个巨大 Commit。

---

## 15. 禁止事项

- 禁止只改“查看详情”文字而保留按钮。
- 禁止删除 Protocol、Host、Model、Masked Key、Status。
- 禁止把含 Checkbox 的整张卡片改成 Button。
- 禁止删除键盘跳转。
- 禁止未诊断时继续无效跳转。
- 禁止删除 Confidence。
- 禁止删除 Direct / Route 数据语义。
- 禁止点击复制或展开时触发滚动。
- 禁止全局缩小字体。
- 禁止新增 UI 框架或状态库。
- 禁止修改 Rust 后端。
- 禁止顺手重构无关组件。
- 禁止 CI 未通过就发布。

---

## 16. 发布前验证

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

手工尺寸：

```text
1100×740
960×640
1366×768
1440×900
```

手工数据：

```text
无结果、1 个结果、10 个结果
RATE_LIMITED、AUTH_INVALID、MODEL_NOT_FOUND
CURRENT_CONFIG_OK、Route Not Running、Route Attempted
Model Transform、Managed Auth Skipped
```

---

## 17. Release 资产

```text
CC-Switch-Doctor-v0.1.10-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.10-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release Notes：

```text
- Removed the redundant Provider “查看详情” action.
- Provider rows navigate only when a result exists.
- Compacted Provider metadata without hiding protocol, host, model, key mask, or status.
- Removed duplicated diagnosis conclusion text.
- Replaced simple Direct/Route blocks with a compact channel summary.
- Prevented copy and accordion controls from triggering cross-pane navigation.
- Added UI density and interaction regression tests.
```

---

## 18. 最终验收示例

### 未诊断 Provider

```text
Provider 名称                         可诊断
Claude Code · sk-abc…123 · Anthropic Messages
api.example.com                      glm-5.2
```

- 无“查看详情”；
- Checkbox 可选；
- 点击正文不跳转；
- Cursor 非 Pointer。

### 已诊断 Provider

- 点击正文跳转结果；
- Checkbox 只选择；
- Enter / Space 跳转。

### Rate Limited Result

```text
Claude Code / Provider       [请求被限流] [可信度：低]
api.example.com

直连：请求被限流 · 路由：未验证（CCS 未运行）

真实请求 1 · 未发现成功组合

建议
请稍后重试，并检查 Retry-After 或供应商限流策略。

判定依据（折叠）
尝试链（折叠）
调试日志（折叠）
```

不得重复显示：

```text
诊断结论
请求被限流
```

---

## 19. 最终汇报格式

```text
1. 修复提交列表
2. “查看详情”删除与导航条件
3. Provider 三行布局截图
4. ResultCard 去重说明
5. Direct / Route 摘要说明
6. 事件冒泡测试
7. 1100×740 截图
8. 960×640 截图
9. 前端测试
10. Rust 回归测试
11. Windows 构建
12. GitHub Actions
13. v0.1.10 Tag SHA
14. Release 资产大小和 SHA-256
15. git status --short（必须为空）
```

---

## 20. 直接交给 AI 工具的执行指令

```text
严格阅读并执行：

docs/project/CC-Switch-Doctor-v0.1.10-Compact-Provider-and-Result-UI-Regression-Safe-Spec.md

这是基于 main/v0.1.9 的纯前端定向修复。

只处理：
1. 删除重复且未诊断时无效的“查看详情”；
2. Provider 仅在存在结果时允许点击内容区域跳转；
3. 保留全部核心信息并压缩 Provider 卡片；
4. 删除 ResultCard 重复“诊断结论”；
5. 将简单 Direct / Route 状态合并为摘要行；
6. 只有真实路由尝试或复杂 Evidence 时显示 Route 详情；
7. 防止复制、Accordion、Select 触发跨栏跳转；
8. 补齐交互、键盘、筛选和密度测试。

禁止修改 Rust 诊断、协议、模型、路由、Schema、数据库和安全逻辑。
禁止增加新功能、新设置、新框架或无关重构。
禁止通过全局缩小字体解决密度问题。

冻结 v0.1.9 已通过的模型语义、错误分类、Schema Capability、Primary/Direct/Route、默认 Claude、Provider 默认不勾选、双向定位、三点菜单、请求预算、Key 脱敏、SQLite 只读和 CLI 隔离。

全部旧测试、新测试、安全门禁、Windows 构建和 GitHub Actions 成功后发布 v0.1.10。
```
