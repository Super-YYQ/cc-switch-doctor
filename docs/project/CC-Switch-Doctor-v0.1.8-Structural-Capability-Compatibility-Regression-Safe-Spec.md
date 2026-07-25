# CC Switch Doctor v0.1.8 结构能力兼容与升级韧性回归安全修复规范

> 目标版本：v0.1.8  
> 基线版本：当前 `main / v0.1.7`  
> 任务类型：CC Switch 3.18.0 兼容热修复 + 长期 Schema 能力检测架构修正  
> 核心目标：**彻底解决“CC Switch 每升级一次，Doctor 就因未知 user_version 停止工作”的设计问题。**

---

# 0. 本次任务背景

当前运行环境：

```text
CC Switch：3.18.0
SQLite user_version：16
CC Switch Doctor：v0.1.7
Doctor 已验证版本：3.17.0
```

当前表现：

```text
Schema 未知
停止读取 Provider
Provider 列表为空
无法开始诊断
```

经核对 CC Switch 3.18.0 上游源码：

- `user_version` 从 15 升级到 16；
- v15 → v16 迁移主要用于重建 Codex 会话用量数据；
- Doctor 所依赖的核心表没有发生破坏性变化：
  - `providers`
  - `provider_endpoints`
  - `settings`
- Provider 核心字段仍然存在；
- `settings_config`、`meta`、`is_current` 和 Endpoint 结构仍可按现有逻辑读取。

当前问题不是 Provider 结构损坏，而是 Doctor 把：

```text
未知 user_version
```

错误地等同于：

```text
Provider 结构不兼容
```

这次必须同时：

1. 恢复 CC Switch 3.18.0 / Schema 16 的使用；
2. 修正长期兼容架构；
3. 确保未来 CC Switch 仅增加无关表、统计字段或迁移版本时，Doctor 不会再次整体失效。

---

# 1. 强制长期架构原则

## 1.1 两套判断必须完全分离

### 精确版本白名单和上游 Commit Manifest

只负责：

```text
是否经过 Doctor 团队完整验证
```

用于显示：

```text
Verified / 已验证
```

以及：

- 自动化回归测试；
- 上游源码基线记录；
- Routing Profile 版本绑定；
- Upstream Watch；
- Release Notes；
- 已验证兼容范围。

它不能作为 Provider 是否允许读取、上游直连是否允许执行的唯一门槛。

### 运行时实际结构能力检测

负责：

```text
当前数据库里的实际结构，是否足以安全执行某项功能
```

用于判断：

- Provider 扫描；
- Endpoint 扫描；
- 上游直连诊断；
- CCS 路由配置读取；
- CCS 路由链诊断；
- 当前 Provider 识别；
- 故障转移状态读取。

必须落实以下架构：

```text
精确版本白名单
→ 只负责“已验证”标签

实际结构能力检测
→ 负责“能不能运行”
```

## 1.2 核心规则

```text
版本未知 ≠ 结构不兼容
新增字段 ≠ 结构不兼容
新增无关表 ≠ 结构不兼容
无关数据库迁移 ≠ Provider 不兼容
路由结构变化 ≠ Provider 结构不兼容
单个 Provider 配置异常 ≠ 整个数据库不可用
```

只有实际发生以下情况时，才允许禁用相关能力：

- Doctor 必需的核心表缺失；
- Doctor 必需的关键列缺失或语义明显变化；
- `settings_config` 已无法安全读取；
- Provider 配置变成未知加密 Blob；
- 主键、关联字段或数据类型发生破坏性变化；
- 继续读取可能导致凭据误读或泄露。

即使出现破坏性变化，也必须：

```text
只禁用受影响能力
```

不得一处变化导致整个应用全部停止。

---

# 2. 冻结功能清单

以下 v0.1.7 已通过功能不得改坏。

## 2.1 Provider 和 UI

- 默认应用筛选为 `Claude`。
- 核心筛选：
  - 全部；
  - Claude；
  - Codex；
  - Gemini；
  - OpenCode。
- Provider 行默认不勾选。
- 用户手动勾选后才可诊断。
- 三点菜单：
  - 点击外部关闭；
  - Esc 关闭；
  - 点击菜单项后关闭。
- Provider 与结果双向定位继续有效。
- 结果上一条 / 下一条继续有效。
- 默认窗口尺寸和最小尺寸不退化。
- 紧凑 UI 密度不退化。

## 2.2 诊断与路由

- Primary / Direct / Route 结果分层不退化。
- `CCS_ROUTE_NOT_APPLICABLE` 不得覆盖真实直连错误。
- 只有真实发送路由请求时，路由结果才参与主结论。
- 快速验证、智能诊断、深度兼容含义不变。
- 并发 1 / 2 / 3 继续可见可改。
- 自动 / 仅直连 / 直连+路由继续有效。
- Provider、Host、Route 请求预算不退化。
- 路由只允许 loopback。
- 路由请求不携带 Provider 真实 Key。

## 2.3 安全边界

- SQLite 只读。
- 不修改 `user_version`。
- 不写 CC Switch 数据库。
- 不修改 Provider。
- 不修改 `proxy_config`。
- 不启动、停止或切换 CCS 路由。
- 不启动任何 AI CLI。
- 不读取 AI CLI 登录目录。
- 完整 Key 不进入前端、日志、缓存键或剪贴板。
- 未知或不兼容结构必须安全降级，不能盲目猜测敏感字段。

---

# 3. 当前错误架构必须删除

当前 `fingerprint.rs` 使用类似以下逻辑：

```rust
SCHEMA_ALLOWLIST = [
    user_version 13,
    user_version 15,
]
```

只有精确命中条目才允许：

```text
can_test = true
```

其他版本即使：

- `providers` 完整；
- `provider_endpoints` 完整；
- 字段完全一致；

仍被判为：

```text
Unknown
停止读取 Provider
```

这种逻辑必须废除。

不得采用以下伪修复：

```rust
user_version 12..=20 全部兼容
```

也不得只增加：

```rust
user_version=16
```

然后保留同样的长期问题。

v16 精确条目仍然要增加，但它只用于：

```text
Verified 标签
```

不能再作为唯一运行门槛。

---

# 4. 新的兼容性数据模型

新增能力级兼容报告：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityReport {
    pub user_version: i32,
    pub version_verification: VersionVerification,
    pub observed_fingerprint: String,
    pub capabilities: SchemaCapabilities,
    pub warnings: Vec<String>,
}
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionVerification {
    Verified,
    KnownCompatible,
    UnverifiedStructureCompatible,
    Unknown,
}
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCapabilities {
    pub provider_scan: CapabilityStatus,
    pub endpoint_scan: CapabilityStatus,
    pub direct_diagnosis: CapabilityStatus,
    pub routing_discovery: CapabilityStatus,
    pub routing_diagnosis: CapabilityStatus,
}
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Degraded,
    Disabled,
}
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityStatus {
    pub state: CapabilityState,
    pub reason: String,
    pub missing_tables: Vec<String>,
    pub missing_columns: Vec<String>,
    pub unverified_columns: Vec<String>,
}
```

---

# 5. Version Verification 与 Capability 必须独立

例如 CC Switch 3.19.0 / user_version=17 发布后，若核心结构未变化：

```text
Version Verification：
UnverifiedStructureCompatible

Provider Scan：
Supported

Endpoint Scan：
Supported

Direct Diagnosis：
Supported

Routing Discovery：
Supported 或 Degraded

Routing Diagnosis：
Supported / Degraded / Disabled
```

页面允许继续工作，只提示：

```text
CC Switch 当前版本尚未完成完整验证，但 Provider 核心结构兼容。
```

不得清空 Provider。

---

# 6. Provider 核心能力检测

## 6.1 Provider Scan 必需结构

`providers` 必需表。

必需字段：

```text
id
app_type
name
settings_config
meta
is_current
```

只要这些字段存在，Provider Scan 可进入：

```text
Supported
```

推荐但非必需字段：

```text
website_url
category
created_at
sort_index
notes
icon
icon_color
in_failover_queue
```

推荐字段缺失时：

```text
Provider Scan：Degraded
```

但仍继续读取 Provider。

未知新增字段：

```text
忽略并继续
```

不得因为列集合与已验证版本不完全相等就停止。

## 6.2 类型和数据安全

不能只检查列名，还应检查实际读取能力：

- `id` 可读取为字符串；
- `app_type` 可读取为字符串；
- `name` 可读取为字符串；
- `settings_config` 可读取为字符串；
- `meta` 可读取为字符串或 NULL 安全回退；
- `is_current` 可读取为整数/布尔兼容值。

如果某一行类型异常：

```text
只跳过该 Provider
```

不得阻断整个 Provider 列表。

如果整列发生破坏性类型变化：

```text
Provider Scan：Disabled
```

并显示具体缺失或不兼容原因。

---

# 7. Endpoint 能力检测

`provider_endpoints` 必需字段：

```text
provider_id
app_type
url
```

推荐字段：

```text
id
added_at
```

## 7.1 Endpoint 表完整

```text
Endpoint Scan：Supported
```

## 7.2 Endpoint 表缺失或关键列不完整

不能直接阻断 Provider。

应该：

1. Provider 仍然展示；
2. 尝试从 `settings_config` 安全提取 Base URL；
3. 能提取：
   ```text
   Endpoint Scan：Degraded
   Direct Diagnosis：Supported
   ```
4. 无法提取：
   ```text
   仅该 Provider 标记“缺少 Base URL”
   ```

不得隐藏所有 Provider。

---

# 8. 单个 Provider 独立降级

每个 Provider 必须有独立解析结果：

```rust
enum ProviderReadiness {
    Testable,
    ManagedAuthSkipped,
    MissingKey,
    MissingBaseUrl,
    UnknownSettingsFormat,
    UnsupportedAppType,
    InvalidRow,
}
```

示例：

```text
Provider A：可诊断
Provider B：配置格式未识别
Provider C：托管认证，跳过
Provider D：缺少 Key
Provider E：可诊断
```

任何单个 Provider 失败，都不能阻断其他 Provider。

---

# 9. Direct Diagnosis 能力

只要：

- Provider Scan 可用；
- 能安全解析 Provider；
- 能取得 Base URL；
- 能取得可使用凭据；
- App 类型有已知协议 Adapter；

即可：

```text
Direct Diagnosis：Supported
```

未知 CC Switch `user_version` 不得单独禁用 Direct Diagnosis。

若 Provider 配置格式发生变化，只对该 Provider 降级。

---

# 10. Routing Discovery 与 Routing Diagnosis 分离

## 10.1 Routing Discovery

用于只读读取：

```text
proxy_config
proxy_enabled
listen_address
listen_port
enabled
auto_failover_enabled
live_takeover_active
```

只要已知核心字段存在：

```text
Routing Discovery：Supported
```

缺少非关键字段：

```text
Routing Discovery：Degraded
```

表或关键结构未知：

```text
Routing Discovery：Disabled
```

但不得影响：

- Provider Scan；
- Endpoint Scan；
- Direct Diagnosis。

## 10.2 Routing Diagnosis

除了 Routing Discovery，还需要：

- 已验证 CCS Routing Profile；
- 已知本地路由路径；
- 已知占位 Token；
- 已知客户端模型别名；
- loopback 地址；
- 本地路由实际可达。

未知 CCS 版本但路由协议 Profile 未验证时：

```text
Provider：正常
直连诊断：正常
路由状态读取：可以
路由真实请求：禁用
```

UI：

```text
CCS 路由结构可读取，但当前 CCS 版本的路由协议尚未验证；本次仅执行上游直连。
```

不能清空 Provider。

---

# 11. CC Switch 3.18.0 / Schema 16 精确验证

长期结构检测完成后，再增加 v16 的 Verified 信息。

新增精确记录：

```text
CC Switch：3.18.0
user_version：16
状态：Verified
上游基线 Commit：878c26f31e012ba32b9772bd080bd4fa9e7d495e
```

v16 Provider 核心字段与 v15 一致。

必须记录：

- `providers` 指纹；
- `provider_endpoints` 指纹；
- `settings` 指纹；
- `proxy_config` 能力；
- 上游迁移说明。

v15 → v16 迁移只涉及 Codex 会话用量重建，不应成为 Provider 不兼容依据。

---

# 12. Manifest 新结构

兼容 Manifest 不再只保存版本列表。

建议：

```json
{
  "doctorVersion": "0.1.8",
  "verifiedVersions": [
    {
      "ccSwitchVersion": "3.17.0",
      "userVersion": 15,
      "upstreamCommit": "...",
      "providerShapeId": "provider-core-v1",
      "endpointShapeId": "endpoint-core-v1",
      "routingProfileId": "ccs-routing-v317"
    },
    {
      "ccSwitchVersion": "3.18.0",
      "userVersion": 16,
      "upstreamCommit": "878c26f31e012ba32b9772bd080bd4fa9e7d495e",
      "providerShapeId": "provider-core-v1",
      "endpointShapeId": "endpoint-core-v1",
      "routingProfileId": "ccs-routing-v318"
    }
  ],
  "capabilityShapes": {
    "provider-core-v1": {
      "requiredColumns": ["id", "app_type", "name", "settings_config", "meta", "is_current"],
      "optionalColumns": [
        "website_url",
        "category",
        "created_at",
        "sort_index",
        "notes",
        "icon",
        "icon_color",
        "in_failover_queue"
      ]
    },
    "endpoint-core-v1": {
      "requiredColumns": ["provider_id", "app_type", "url"],
      "optionalColumns": ["id", "added_at"]
    }
  }
}
```

运行时：

- Capability Shape 负责判断功能；
- Verified Version 负责显示“已验证”；
- 两者不能混为一个门禁。

---

# 13. Fingerprint 规则

Observed Fingerprint 应包括：

```text
user_version
表名
关键列名
关键列 SQLite 类型
关键索引/主键信息
```

但兼容判断不能要求所有表和列完全相同。

规则：

```text
新增无关表 → 兼容
新增可选列 → 兼容
新增未知列 → 兼容并记录
缺少可选列 → 降级
缺少必需列 → 禁用对应能力
必需列类型破坏性变化 → 禁用对应能力
```

---

# 14. UI 状态设计

顶部不再只有单一：

```text
Schema：已知 / 未知
```

应显示：

```text
CC Switch：3.18.0
版本验证：已验证
Provider：可用
上游直连：可用
CCS 路由：可用
```

未知版本但兼容：

```text
CC Switch：3.19.0
版本验证：尚未完整验证
Provider：结构兼容
上游直连：可用
CCS 路由：暂未验证
```

严重破坏：

```text
Provider：不可用
原因：providers.settings_config 字段缺失
```

能力状态使用：

```text
已验证
结构兼容
降级可用
暂不可用
不兼容
```

不得因为 `version_verification=Unverified` 把整个开始诊断按钮禁用。

开始诊断是否可用，应由：

```text
direct_diagnosis.state
```

和选中的 Provider 状态决定。

---

# 15. Upstream Watch 长期设计

Upstream Watch 继续监控：

- CC Switch 最新 Release；
- `SCHEMA_VERSION`；
- `schema.rs`；
- Provider 表；
- Endpoint 表；
- Proxy Config；
- 路由 Handler；
- 路由协议映射。

但它的职责是：

```text
更新 Verified 信息和发现破坏性变化
```

不能成为本地 Doctor 能否继续运行的唯一条件。

Upstream Watch 应对变化分类：

```text
无关迁移
新增无关表
新增兼容字段
Provider 核心变化
Endpoint 核心变化
路由结构变化
路由协议变化
```

只有核心变化才创建高优先级 Issue。

无关版本升级：

```text
No breaking provider capability change
```

不得要求立即发布 Doctor 才能恢复 Provider。

---

# 16. 必须新增的测试

## 16.1 当前 v16

```text
user_version=16 + v16 完整结构
→ version_verification=Verified
→ provider_scan=Supported
→ direct_diagnosis=Supported
→ Provider 正常展示
```

## 16.2 未知版本但结构相同

```text
user_version=17 + 与 v16 相同核心结构
→ version_verification=UnverifiedStructureCompatible
→ provider_scan=Supported
→ direct_diagnosis=Supported
→ Provider 正常展示
```

这是本次最关键回归测试。

## 16.3 新增无关表和字段

```text
user_version=18
新增 unrelated_table
providers 新增 future_field
→ Provider 正常展示
→ Direct Diagnosis 可用
```

## 16.4 缺少可选字段

```text
缺少 icon / notes / category
→ Provider 正常展示
→ provider_scan=Degraded 或 Supported
```

## 16.5 缺少 Provider 必需字段

```text
缺少 settings_config
→ provider_scan=Disabled
→ 不读取敏感配置
```

## 16.6 Endpoint 变化

```text
provider_endpoints 缺失
settings_config 有 Base URL
→ Provider 展示
→ endpoint_scan=Degraded
→ direct_diagnosis=Supported
```

```text
provider_endpoints 缺失
settings_config 也无 Base URL
→ 只跳过该 Provider
```

## 16.7 路由单独降级

```text
Provider 核心正常
proxy_config 结构未知
→ Provider 正常
→ Direct Diagnosis 正常
→ Routing Discovery/Diagnosis 禁用
```

## 16.8 单 Provider 异常

```text
三个 Provider
一个 settings_config 无法解析
→ 另外两个仍正常展示和测试
```

## 16.9 新增未知列

```text
providers 新增 JSON 字段
→ 不阻断
```

## 16.10 旧版本回归

```text
user_version=13
user_version=15
user_version=16
```

全部继续正常。

---

# 17. Synthetic Fixtures

新增：

```text
compatibility/fixtures/
├─ synthetic-v16.sql
├─ synthetic-future-v17-same-core.sql
├─ synthetic-future-extra-columns.sql
├─ synthetic-provider-required-column-missing.sql
├─ synthetic-endpoints-missing-baseurl-in-settings.sql
├─ synthetic-routing-unknown-provider-compatible.sql
└─ synthetic-one-provider-invalid.sql
```

所有 Fixture 必须：

- 纯虚拟；
- 不含真实 Key；
- 不含真实私人 URL；
- 测试前后 SHA-256 不变。

---

# 18. 推荐实现顺序

## Commit 1

```text
refactor(schema): separate version verification from runtime capabilities
```

建立数据模型，不改变 Provider 解析。

## Commit 2

```text
feat(schema): detect provider and endpoint capabilities from observed structure
```

实现结构能力检测。

## Commit 3

```text
fix(scan): degrade individual providers instead of blocking the database
```

实现单 Provider 降级。

## Commit 4

```text
fix(routing): decouple routing compatibility from provider compatibility
```

路由结构变化只影响路由能力。

## Commit 5

```text
feat(compat): verify CC Switch 3.18.0 schema v16
```

加入 v16 Verified 信息。

## Commit 6

```text
fix(ui): show version verification and capability status separately
```

修改顶部状态和空状态。

## Commit 7

```text
test(v0.1.8): add future-version and capability degradation matrix
```

补全测试。

## Commit 8

```text
release: prepare v0.1.8
```

仅版本、CHANGELOG 和 Release Notes。

---

# 19. 禁止事项

- 禁止只增加 `user_version=16` 后结束任务。
- 禁止使用宽范围：
  ```rust
  12..=99
  ```
- 禁止未知版本直接全部放行。
- 禁止未知版本直接全部阻断。
- 禁止降低或修改数据库 `user_version`。
- 禁止写数据库。
- 禁止因路由结构未知隐藏 Provider。
- 禁止因单个 Provider 解析失败阻断其他 Provider。
- 禁止把所有字段都变成可选，导致凭据误读。
- 禁止移除安全停止机制。
- 禁止推倒 v0.1.7 的 Outcome、Route、Parser 或 UI 架构。
- 禁止 CI 未绿时发布。

---

# 20. 发布前验证

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

额外断言：

```text
v16 Provider 正常展示
v17 相同结构正常展示
新增字段不阻断
缺少必需字段仅禁用对应能力
路由未知不影响 Provider 和 Direct
单 Provider 异常不影响其他 Provider
DB SHA256 前后不变
无 process spawn
无登录目录读取
无完整 Key 进入前端
```

---

# 21. Release 资产

```text
CC-Switch-Doctor-v0.1.8-Windows-x64-setup.exe
CC-Switch-Doctor-v0.1.8-Windows-x64-portable.zip
SHA256SUMS.txt
```

Release Notes 必须说明：

```text
- Added CC Switch 3.18.0 / schema v16 verification.
- Replaced version-gated shutdown with capability-based schema compatibility.
- Future unknown CC Switch versions can continue Provider scanning and direct diagnosis when required core structures remain compatible.
- Routing compatibility now degrades independently from Provider compatibility.
- Unknown or destructive structures still fail closed for affected sensitive capabilities.
```

---

# 22. 最终汇报格式

完成后只输出：

```text
1. 修复提交列表
2. Version Verification 与 Capability Detection 的新架构
3. v16 验证结果
4. v17 相同结构自动兼容结果
5. 新增字段兼容测试
6. 必需字段缺失安全停止测试
7. Endpoint 降级测试
8. Routing 单独降级测试
9. 单 Provider 异常隔离测试
10. UI 状态截图
11. 本地测试结果
12. GitHub Actions 状态
13. v0.1.8 Tag SHA
14. Release 资产大小和 SHA-256
15. git status --short（必须为空）
```

---

# 23. 直接交给 AI 工具的执行指令

```text
严格阅读并执行仓库中的 CC-Switch-Doctor-v0.1.8-Structural-Capability-Compatibility-Regression-Safe-Spec.md。

这是基于 main/v0.1.7 的兼容架构修复任务，不是单纯添加 user_version=16 白名单。

必须落实长期架构：

精确版本白名单和上游 Commit Manifest
→ 只负责“已验证 Verified”标签、回归测试和源码基线

运行时实际结构能力检测
→ 负责判断 Provider Scan、Endpoint Scan、Direct Diagnosis、Routing Discovery 和 Routing Diagnosis 能否运行

未知 user_version 但核心结构兼容时，必须继续读取 Provider 和执行上游直连诊断。
新增无关表、未知字段或无关数据库迁移不得让 Provider 列表清空。
路由结构未知时，只禁用 CCS 路由能力，不得影响 Provider 和 Direct Diagnosis。
单个 Provider settings_config 异常时，只跳过该 Provider，不得阻断其他 Provider。

同时完成 CC Switch 3.18.0 / user_version=16 的 Verified 记录和完整 Fixture，但禁止只加 v16 后结束任务。

严格冻结 v0.1.7 已通过的默认 Claude 筛选、Provider 默认不勾选、Primary/Direct/Route 结果分层、路由辅助状态不覆盖直连错误、菜单关闭、双向结果联动、请求预算、Key 脱敏、SQLite 只读和 CLI 隔离。

按照文档的小提交顺序执行，每组修改后立即运行相关测试。全部旧测试、新测试、安全门禁、Windows 构建和远程 CI 成功后发布 v0.1.8；CI 或 Release 失败时继续修复，不能提前结束。
```
