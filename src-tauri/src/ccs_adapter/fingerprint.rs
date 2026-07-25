use crate::error::{PublicError, PublicResult};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Legacy compatibility status kept for existing UI/API fields during the
/// transition to version-verification + capability detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Verified,
    Compatible,
    Unknown,
    Unsupported,
}

impl CompatibilityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Compatible => "compatible",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }

    /// Historical gate: only verified/compatible could test.
    /// Prefer `CompatibilityReport::can_test()` which is capability-based.
    pub fn can_test(self) -> bool {
        matches!(self, Self::Verified | Self::Compatible)
    }
}

/// Whether this exact CC Switch version has been fully verified by Doctor.
/// Independent from whether runtime capabilities are usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionVerification {
    /// Exact allowlist hit with full verification.
    Verified,
    /// Exact allowlist hit marked compatible (not full verification).
    KnownCompatible,
    /// Version not in allowlist, but required core structures are present.
    UnverifiedStructureCompatible,
    /// Structure insufficient / unknown / unsupported.
    Unknown,
}

impl VersionVerification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::KnownCompatible => "known_compatible",
            Self::UnverifiedStructureCompatible => "unverified_structure_compatible",
            Self::Unknown => "unknown",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Verified => "已验证",
            Self::KnownCompatible => "已知兼容",
            Self::UnverifiedStructureCompatible => "结构兼容（尚未完整验证）",
            Self::Unknown => "未知",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Degraded,
    Disabled,
}

impl CapabilityState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Degraded => "degraded",
            Self::Disabled => "disabled",
        }
    }

    pub fn is_usable(self) -> bool {
        matches!(self, Self::Supported | Self::Degraded)
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Supported => "可用",
            Self::Degraded => "降级可用",
            Self::Disabled => "不可用",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityStatus {
    pub state: CapabilityState,
    pub reason: String,
    pub missing_tables: Vec<String>,
    pub missing_columns: Vec<String>,
    pub unverified_columns: Vec<String>,
}

impl CapabilityStatus {
    pub fn supported(reason: impl Into<String>) -> Self {
        Self {
            state: CapabilityState::Supported,
            reason: reason.into(),
            missing_tables: vec![],
            missing_columns: vec![],
            unverified_columns: vec![],
        }
    }

    pub fn degraded(reason: impl Into<String>) -> Self {
        Self {
            state: CapabilityState::Degraded,
            reason: reason.into(),
            missing_tables: vec![],
            missing_columns: vec![],
            unverified_columns: vec![],
        }
    }

    pub fn disabled(reason: impl Into<String>) -> Self {
        Self {
            state: CapabilityState::Disabled,
            reason: reason.into(),
            missing_tables: vec![],
            missing_columns: vec![],
            unverified_columns: vec![],
        }
    }

    pub fn with_missing_tables(mut self, tables: Vec<String>) -> Self {
        self.missing_tables = tables;
        self
    }

    pub fn with_missing_columns(mut self, cols: Vec<String>) -> Self {
        self.missing_columns = cols;
        self
    }

    pub fn with_unverified_columns(mut self, cols: Vec<String>) -> Self {
        self.unverified_columns = cols;
        self
    }

    pub fn is_usable(&self) -> bool {
        self.state.is_usable()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCapabilities {
    pub provider_scan: CapabilityStatus,
    pub endpoint_scan: CapabilityStatus,
    pub direct_diagnosis: CapabilityStatus,
    pub routing_discovery: CapabilityStatus,
    pub routing_diagnosis: CapabilityStatus,
}

impl SchemaCapabilities {
    pub fn all_disabled(reason: impl Into<String>) -> Self {
        let r = reason.into();
        Self {
            provider_scan: CapabilityStatus::disabled(r.clone()),
            endpoint_scan: CapabilityStatus::disabled(r.clone()),
            direct_diagnosis: CapabilityStatus::disabled(r.clone()),
            routing_discovery: CapabilityStatus::disabled(r.clone()),
            routing_diagnosis: CapabilityStatus::disabled(r),
        }
    }
}

/// Capability-first compatibility report.
/// Version verification is independent from runtime capability gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityReport {
    pub user_version: i32,
    pub version_verification: VersionVerification,
    pub observed_fingerprint: String,
    pub capabilities: SchemaCapabilities,
    pub warnings: Vec<String>,
    /// Human-readable summary (Chinese UI).
    pub message: String,
    /// Tables observed in the DB.
    pub tables: Vec<String>,
    pub providers_columns: Vec<String>,
    pub provider_endpoints_columns: Vec<String>,
    pub settings_columns: Vec<String>,
}

impl CompatibilityReport {
    /// Diagnose/start gate is based on direct_diagnosis capability, NOT version verification.
    pub fn can_test(&self) -> bool {
        self.capabilities.direct_diagnosis.is_usable()
            && self.capabilities.provider_scan.is_usable()
    }

    pub fn can_scan_providers(&self) -> bool {
        self.capabilities.provider_scan.is_usable()
    }

    /// Map to legacy CompatibilityStatus for existing UI fields.
    pub fn legacy_status(&self) -> CompatibilityStatus {
        match self.version_verification {
            VersionVerification::Verified => CompatibilityStatus::Verified,
            VersionVerification::KnownCompatible => CompatibilityStatus::Compatible,
            VersionVerification::UnverifiedStructureCompatible => {
                // Structure is usable — do not surface as "unknown" which historically blocked UI.
                if self.can_scan_providers() {
                    CompatibilityStatus::Compatible
                } else {
                    CompatibilityStatus::Unknown
                }
            }
            VersionVerification::Unknown => {
                if !self.capabilities.provider_scan.is_usable() {
                    CompatibilityStatus::Unsupported
                } else {
                    CompatibilityStatus::Unknown
                }
            }
        }
    }
}

/// Observed structural fingerprint (kept for API compatibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaFingerprint {
    pub id: String,
    pub user_version: i32,
    pub tables: Vec<String>,
    pub providers_columns: Vec<String>,
    pub provider_endpoints_columns: Vec<String>,
    pub settings_columns: Vec<String>,
    pub status: CompatibilityStatus,
    pub message: String,
}

impl From<&CompatibilityReport> for SchemaFingerprint {
    fn from(r: &CompatibilityReport) -> Self {
        Self {
            id: r.observed_fingerprint.clone(),
            user_version: r.user_version,
            tables: r.tables.clone(),
            providers_columns: r.providers_columns.clone(),
            provider_endpoints_columns: r.provider_endpoints_columns.clone(),
            settings_columns: r.settings_columns.clone(),
            status: r.legacy_status(),
            message: r.message.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct SchemaAllowEntry {
    id: &'static str,
    user_version: i32,
    verification: VersionVerification,
    required_tables: &'static [&'static str],
    /// Exact full column set used only for Verified / KnownCompatible matching.
    providers_columns: &'static [&'static str],
    provider_endpoints_columns: &'static [&'static str],
    message: &'static str,
    #[allow(dead_code)]
    cc_switch_version: &'static str,
}

/// Exact verified/compatible version entries — used ONLY for Verified labels.
/// Never used as the sole runtime gate for Provider scanning.
const SCHEMA_ALLOWLIST: &[SchemaAllowEntry] = &[
    SchemaAllowEntry {
        id: "ccs-schema-v16-providers-v318",
        user_version: 16,
        verification: VersionVerification::Verified,
        required_tables: &["providers", "provider_endpoints", "settings"],
        providers_columns: &[
            "id",
            "app_type",
            "name",
            "settings_config",
            "website_url",
            "category",
            "created_at",
            "sort_index",
            "notes",
            "icon",
            "icon_color",
            "meta",
            "is_current",
            "in_failover_queue",
        ],
        provider_endpoints_columns: &["id", "provider_id", "app_type", "url", "added_at"],
        message: "Schema 与 CC Switch v3.18.0（user_version=16）已验证指纹匹配。v15→v16 迁移仅重建 Codex 会话用量，Provider 核心结构未变。",
        cc_switch_version: "3.18.0",
    },
    SchemaAllowEntry {
        id: "ccs-schema-v15-providers-v317",
        user_version: 15,
        verification: VersionVerification::Verified,
        required_tables: &["providers", "provider_endpoints", "settings"],
        providers_columns: &[
            "id",
            "app_type",
            "name",
            "settings_config",
            "website_url",
            "category",
            "created_at",
            "sort_index",
            "notes",
            "icon",
            "icon_color",
            "meta",
            "is_current",
            "in_failover_queue",
        ],
        provider_endpoints_columns: &["id", "provider_id", "app_type", "url", "added_at"],
        message: "Schema 与 CC Switch v3.17.0（user_version=15）已验证指纹匹配。",
        cc_switch_version: "3.17.0",
    },
    // Real-world DBs observed as user_version=13 with the same core provider shape
    // as the v3.17 lineage (providers + endpoints). Marked known-compatible, not verified.
    SchemaAllowEntry {
        id: "ccs-schema-v13-providers-core",
        user_version: 13,
        verification: VersionVerification::KnownCompatible,
        required_tables: &["providers", "provider_endpoints"],
        providers_columns: &[
            "id",
            "app_type",
            "name",
            "settings_config",
            "website_url",
            "category",
            "created_at",
            "sort_index",
            "notes",
            "icon",
            "icon_color",
            "meta",
            "is_current",
            "in_failover_queue",
        ],
        provider_endpoints_columns: &["id", "provider_id", "app_type", "url", "added_at"],
        message: "Schema user_version=13 已按精确指纹标记为兼容；可只读扫描并测试第三方配置。",
        cc_switch_version: "3.x-observed-v13",
    },
];

/// Required columns for Provider Scan capability.
pub const REQUIRED_PROVIDER_COLS: &[&str] = &[
    "id",
    "app_type",
    "name",
    "settings_config",
    "meta",
    "is_current",
];

/// Optional/recommended provider columns (missing → Degraded, not Disabled).
pub const OPTIONAL_PROVIDER_COLS: &[&str] = &[
    "website_url",
    "category",
    "created_at",
    "sort_index",
    "notes",
    "icon",
    "icon_color",
    "in_failover_queue",
];

/// Required columns for Endpoint Scan capability.
pub const REQUIRED_ENDPOINT_COLS: &[&str] = &["provider_id", "app_type", "url"];

/// Optional endpoint columns.
pub const OPTIONAL_ENDPOINT_COLS: &[&str] = &["id", "added_at"];

/// Required proxy_config columns for Routing Discovery.
pub const REQUIRED_ROUTING_COLS: &[&str] = &[
    "app_type",
    "proxy_enabled",
    "listen_address",
    "listen_port",
    "enabled",
    "auto_failover_enabled",
];

/// Compute the full capability-first compatibility report.
pub fn compute_compatibility_report(conn: &Connection) -> PublicResult<CompatibilityReport> {
    let user_version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| PublicError::Database(format!("读取 user_version 失败：{e}")))?;

    let mut tables = list_tables(conn)?;
    tables.sort();

    let providers_columns = list_columns(conn, "providers").unwrap_or_default();
    let provider_endpoints_columns = list_columns(conn, "provider_endpoints").unwrap_or_default();
    let settings_columns = list_columns(conn, "settings").unwrap_or_default();
    let proxy_config_columns = list_columns(conn, "proxy_config").unwrap_or_default();

    let has_providers = tables.iter().any(|t| t == "providers");
    let has_endpoints = tables.iter().any(|t| t == "provider_endpoints");
    let has_proxy_config = tables.iter().any(|t| t == "proxy_config");

    let mut warnings = Vec::new();

    // --- Provider Scan capability ---
    let provider_scan = detect_provider_scan(has_providers, &providers_columns, &mut warnings);

    // --- Endpoint Scan capability ---
    let endpoint_scan =
        detect_endpoint_scan(has_endpoints, &provider_endpoints_columns, &mut warnings);

    // --- Direct Diagnosis: depends on provider_scan + ability to get base URL ---
    let direct_diagnosis = detect_direct_diagnosis(&provider_scan, &endpoint_scan);

    // --- Routing Discovery / Diagnosis ---
    let (routing_discovery, routing_diagnosis) =
        detect_routing_capabilities(has_proxy_config, &proxy_config_columns, &mut warnings);

    let capabilities = SchemaCapabilities {
        provider_scan,
        endpoint_scan,
        direct_diagnosis,
        routing_discovery,
        routing_diagnosis,
    };

    // --- Version verification (independent of capabilities) ---
    let (version_verification, id_override, verification_message) = classify_version(
        user_version,
        &tables,
        &providers_columns,
        &provider_endpoints_columns,
        &capabilities,
    );

    let observed_fingerprint = id_override.unwrap_or_else(|| {
        fingerprint_id(
            user_version,
            &tables,
            &providers_columns,
            &provider_endpoints_columns,
        )
    });

    // Unknown extra columns → record, never block
    let known_provider: Vec<&str> = REQUIRED_PROVIDER_COLS
        .iter()
        .chain(OPTIONAL_PROVIDER_COLS.iter())
        .copied()
        .collect();
    let unknown_provider_cols: Vec<String> = providers_columns
        .iter()
        .filter(|c| !known_provider.iter().any(|k| k == c))
        .cloned()
        .collect();
    if !unknown_provider_cols.is_empty() {
        warnings.push(format!(
            "providers 存在未知列（已忽略）：{}",
            unknown_provider_cols.join(", ")
        ));
    }

    let message = build_message(
        user_version,
        version_verification,
        &capabilities,
        verification_message.as_deref(),
        &warnings,
    );

    Ok(CompatibilityReport {
        user_version,
        version_verification,
        observed_fingerprint,
        capabilities,
        warnings,
        message,
        tables,
        providers_columns,
        provider_endpoints_columns,
        settings_columns,
    })
}

/// Backward-compatible wrapper: fingerprint + legacy status.
pub fn compute_fingerprint(conn: &Connection) -> PublicResult<SchemaFingerprint> {
    let report = compute_compatibility_report(conn)?;
    Ok(SchemaFingerprint::from(&report))
}

fn detect_provider_scan(
    has_providers: bool,
    providers_columns: &[String],
    warnings: &mut Vec<String>,
) -> CapabilityStatus {
    if !has_providers {
        return CapabilityStatus::disabled("缺少 providers 表，无法安全解析。")
            .with_missing_tables(vec!["providers".into()]);
    }

    let missing_required: Vec<String> = REQUIRED_PROVIDER_COLS
        .iter()
        .filter(|c| !providers_columns.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect();

    if !missing_required.is_empty() {
        return CapabilityStatus::disabled(format!(
            "providers 缺少必需字段：{}",
            missing_required.join(", ")
        ))
        .with_missing_columns(missing_required);
    }

    let missing_optional: Vec<String> = OPTIONAL_PROVIDER_COLS
        .iter()
        .filter(|c| !providers_columns.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect();

    if !missing_optional.is_empty() {
        warnings.push(format!(
            "providers 缺少可选字段：{}",
            missing_optional.join(", ")
        ));
        return CapabilityStatus::degraded(format!(
            "providers 核心字段完整，缺少可选字段：{}",
            missing_optional.join(", ")
        ))
        .with_missing_columns(missing_optional);
    }

    CapabilityStatus::supported("providers 核心与推荐字段完整。")
}

fn detect_endpoint_scan(
    has_endpoints: bool,
    endpoint_columns: &[String],
    warnings: &mut Vec<String>,
) -> CapabilityStatus {
    if !has_endpoints {
        warnings.push("provider_endpoints 表缺失；将尝试从 settings_config 提取 Base URL。".into());
        return CapabilityStatus::degraded(
            "provider_endpoints 表缺失；可从 settings_config 降级提取 Base URL。",
        )
        .with_missing_tables(vec!["provider_endpoints".into()]);
    }

    let missing_required: Vec<String> = REQUIRED_ENDPOINT_COLS
        .iter()
        .filter(|c| !endpoint_columns.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect();

    if !missing_required.is_empty() {
        warnings.push(format!(
            "provider_endpoints 缺少关键列：{}；将尝试从 settings_config 提取 Base URL。",
            missing_required.join(", ")
        ));
        return CapabilityStatus::degraded(format!(
            "provider_endpoints 关键列不完整：{}",
            missing_required.join(", ")
        ))
        .with_missing_columns(missing_required);
    }

    let missing_optional: Vec<String> = OPTIONAL_ENDPOINT_COLS
        .iter()
        .filter(|c| !endpoint_columns.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect();

    if !missing_optional.is_empty() {
        return CapabilityStatus::degraded(format!(
            "provider_endpoints 缺少可选字段：{}",
            missing_optional.join(", ")
        ))
        .with_missing_columns(missing_optional);
    }

    CapabilityStatus::supported("provider_endpoints 结构完整。")
}

fn detect_direct_diagnosis(
    provider_scan: &CapabilityStatus,
    endpoint_scan: &CapabilityStatus,
) -> CapabilityStatus {
    if !provider_scan.is_usable() {
        return CapabilityStatus::disabled(format!(
            "Provider Scan 不可用，无法执行上游直连：{}",
            provider_scan.reason
        ));
    }

    // Endpoint missing is OK if we can fall back to settings_config.
    if endpoint_scan.state == CapabilityState::Disabled {
        return CapabilityStatus::degraded(
            "Endpoint Scan 不可用；仅当 settings_config 含 Base URL 时可诊断。",
        );
    }

    if endpoint_scan.state == CapabilityState::Degraded {
        return CapabilityStatus::degraded(
            "Endpoint 结构降级；将从 settings_config 提取 Base URL 执行上游直连。",
        );
    }

    if provider_scan.state == CapabilityState::Degraded {
        return CapabilityStatus::degraded("Provider 结构降级但仍可执行上游直连。");
    }

    CapabilityStatus::supported("可执行上游直连诊断。")
}

fn detect_routing_capabilities(
    has_proxy_config: bool,
    proxy_columns: &[String],
    warnings: &mut Vec<String>,
) -> (CapabilityStatus, CapabilityStatus) {
    if !has_proxy_config {
        warnings.push("proxy_config 表不存在；路由发现与路由诊断已禁用。".into());
        let discovery = CapabilityStatus::disabled("proxy_config 表不存在。")
            .with_missing_tables(vec!["proxy_config".into()]);
        let diagnosis = CapabilityStatus::disabled("Routing Discovery 不可用，无法执行路由诊断。");
        return (discovery, diagnosis);
    }

    let missing_required: Vec<String> = REQUIRED_ROUTING_COLS
        .iter()
        .filter(|c| !proxy_columns.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect();

    if !missing_required.is_empty() {
        warnings.push(format!(
            "proxy_config 缺少关键列：{}；路由能力已禁用。",
            missing_required.join(", ")
        ));
        let discovery = CapabilityStatus::disabled(format!(
            "proxy_config 关键列缺失：{}",
            missing_required.join(", ")
        ))
        .with_missing_columns(missing_required);
        let diagnosis = CapabilityStatus::disabled("Routing Discovery 不可用，无法执行路由诊断。");
        return (discovery, diagnosis);
    }

    // Optional non-critical fields
    let optional = [
        "enable_logging",
        "max_retries",
        "streaming_first_byte_timeout",
        "streaming_idle_timeout",
        "non_streaming_timeout",
        "live_takeover_active",
    ];
    let missing_optional: Vec<String> = optional
        .iter()
        .filter(|c| !proxy_columns.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect();

    let discovery = if missing_optional.is_empty() {
        CapabilityStatus::supported("proxy_config 结构完整，可读取路由状态。")
    } else {
        CapabilityStatus::degraded(format!(
            "proxy_config 核心字段完整，缺少非关键字段：{}",
            missing_optional.join(", ")
        ))
        .with_missing_columns(missing_optional)
    };

    // Routing diagnosis further requires verified profile + loopback reachability at runtime.
    // At schema level we only say discovery is enough to *attempt* diagnosis when profile allows.
    let diagnosis = if discovery.is_usable() {
        CapabilityStatus::supported(
            "路由结构可读取；实际路由请求还依赖已验证 Routing Profile 与 loopback 可达。",
        )
    } else {
        CapabilityStatus::disabled("Routing Discovery 不可用。")
    };

    (discovery, diagnosis)
}

fn classify_version(
    user_version: i32,
    tables: &[String],
    providers_columns: &[String],
    provider_endpoints_columns: &[String],
    capabilities: &SchemaCapabilities,
) -> (VersionVerification, Option<String>, Option<String>) {
    // Exact allowlist match → Verified / KnownCompatible
    for entry in SCHEMA_ALLOWLIST {
        if entry.user_version != user_version {
            continue;
        }
        let tables_ok = entry
            .required_tables
            .iter()
            .all(|t| tables.iter().any(|x| x == *t));
        let prov_ok = columns_contain_all(providers_columns, entry.providers_columns);
        let ep_ok = if entry.required_tables.contains(&"provider_endpoints") {
            columns_contain_all(provider_endpoints_columns, entry.provider_endpoints_columns)
        } else {
            true
        };
        if tables_ok && prov_ok && ep_ok {
            return (
                entry.verification,
                Some(entry.id.to_string()),
                Some(entry.message.to_string()),
            );
        }
    }

    // Structure-compatible but version not verified
    if capabilities.provider_scan.is_usable() {
        return (
            VersionVerification::UnverifiedStructureCompatible,
            None,
            Some(format!(
                "CC Switch user_version={user_version} 尚未完成完整验证，但 Provider 核心结构兼容。"
            )),
        );
    }

    (
        VersionVerification::Unknown,
        None,
        Some(format!(
            "检测到不兼容或未知结构（user_version={user_version}）。"
        )),
    )
}

fn build_message(
    user_version: i32,
    verification: VersionVerification,
    capabilities: &SchemaCapabilities,
    verification_message: Option<&str>,
    warnings: &[String],
) -> String {
    let mut parts = Vec::new();
    if let Some(m) = verification_message {
        parts.push(m.to_string());
    } else {
        parts.push(format!(
            "user_version={user_version}，版本验证：{}",
            verification.label_zh()
        ));
    }
    parts.push(format!(
        "Provider：{}；上游直连：{}；CCS 路由：{}",
        capabilities.provider_scan.state.label_zh(),
        capabilities.direct_diagnosis.state.label_zh(),
        capabilities.routing_discovery.state.label_zh()
    ));
    if !warnings.is_empty() && warnings.len() <= 3 {
        parts.push(format!("提示：{}", warnings.join("；")));
    }
    parts.join(" ")
}

fn columns_contain_all(have: &[String], need: &[&str]) -> bool {
    need.iter().all(|c| have.iter().any(|x| x == *c))
}

fn fingerprint_id(
    user_version: i32,
    tables: &[String],
    providers_columns: &[String],
    endpoints_columns: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_version.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(tables.join(",").as_bytes());
    hasher.update(b"|");
    hasher.update(providers_columns.join(",").as_bytes());
    hasher.update(b"|");
    hasher.update(endpoints_columns.join(",").as_bytes());
    let dig = hasher.finalize();
    format!("ccs-schema-{}", hex::encode(&dig[..8]))
}

fn list_tables(conn: &Connection) -> PublicResult<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| PublicError::Database(e.to_string()))?);
    }
    Ok(out)
}

fn list_columns(conn: &Connection, table: &str) -> PublicResult<Vec<String>> {
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(PublicError::Database("非法表名".into()));
    }
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| PublicError::Database(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| PublicError::Database(e.to_string()))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn seed_core_shape(conn: &Connection, user_version: i32) {
        conn.execute_batch(&format!(
            r#"
            PRAGMA user_version={user_version};
            CREATE TABLE providers (
                id TEXT, app_type TEXT, name TEXT, settings_config TEXT,
                website_url TEXT, category TEXT, created_at INTEGER, sort_index INTEGER,
                notes TEXT, icon TEXT, icon_color TEXT, meta TEXT, is_current INTEGER,
                in_failover_queue INTEGER
            );
            CREATE TABLE provider_endpoints (
                id INTEGER PRIMARY KEY, provider_id TEXT, app_type TEXT, url TEXT, added_at INTEGER
            );
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
            "#
        ))
        .unwrap();
    }

    #[test]
    fn verified_v15_fingerprint() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 15);
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Verified);
        assert!(fp.status.can_test());

        let report = compute_compatibility_report(&conn).unwrap();
        assert_eq!(report.version_verification, VersionVerification::Verified);
        assert_eq!(
            report.capabilities.provider_scan.state,
            CapabilityState::Supported
        );
        assert_eq!(
            report.capabilities.direct_diagnosis.state,
            CapabilityState::Supported
        );
        assert!(report.can_test());
    }

    #[test]
    fn compatible_v13_exact_fingerprint() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 13);
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Compatible);
        assert!(fp.status.can_test());
        assert_eq!(fp.id, "ccs-schema-v13-providers-core");

        let report = compute_compatibility_report(&conn).unwrap();
        assert_eq!(
            report.version_verification,
            VersionVerification::KnownCompatible
        );
        assert!(report.can_test());
    }

    #[test]
    fn verified_v16_fingerprint() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 16);
        let report = compute_compatibility_report(&conn).unwrap();
        assert_eq!(report.version_verification, VersionVerification::Verified);
        assert_eq!(report.observed_fingerprint, "ccs-schema-v16-providers-v318");
        assert_eq!(
            report.capabilities.provider_scan.state,
            CapabilityState::Supported
        );
        assert_eq!(
            report.capabilities.direct_diagnosis.state,
            CapabilityState::Supported
        );
        assert!(report.can_test());
        assert!(report.can_scan_providers());
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Verified);
        assert!(fp.status.can_test());
    }

    #[test]
    fn future_v19_same_core_is_structure_compatible() {
        // Beyond the highest verified user_version; structure still wins.
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 19);
        let report = compute_compatibility_report(&conn).unwrap();
        assert_eq!(
            report.version_verification,
            VersionVerification::UnverifiedStructureCompatible
        );
        assert_eq!(
            report.capabilities.provider_scan.state,
            CapabilityState::Supported
        );
        assert_eq!(
            report.capabilities.direct_diagnosis.state,
            CapabilityState::Supported
        );
        assert!(report.can_test());
        assert!(report.can_scan_providers());
        let fp = SchemaFingerprint::from(&report);
        assert!(fp.status.can_test() || matches!(fp.status, CompatibilityStatus::Compatible));
    }

    #[test]
    fn future_v17_same_core_structure_compatible() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 17);
        let report = compute_compatibility_report(&conn).unwrap();
        assert_eq!(
            report.version_verification,
            VersionVerification::UnverifiedStructureCompatible
        );
        assert!(report.can_test());
    }

    #[test]
    fn extra_unrelated_table_and_column_compatible() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 18);
        conn.execute_batch(
            "CREATE TABLE unrelated_table(x TEXT); ALTER TABLE providers ADD COLUMN future_field TEXT;",
        )
        .unwrap();
        let report = compute_compatibility_report(&conn).unwrap();
        assert!(report.can_scan_providers());
        assert!(report.can_test());
        assert!(report
            .warnings
            .iter()
            .any(|w| w.contains("future_field") || w.contains("未知列")));
    }

    #[test]
    fn missing_optional_provider_cols_degraded_not_disabled() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA user_version=15;
            CREATE TABLE providers (
                id TEXT, app_type TEXT, name TEXT, settings_config TEXT,
                meta TEXT, is_current INTEGER
            );
            CREATE TABLE provider_endpoints (
                id INTEGER PRIMARY KEY, provider_id TEXT, app_type TEXT, url TEXT, added_at INTEGER
            );
            "#,
        )
        .unwrap();
        let report = compute_compatibility_report(&conn).unwrap();
        assert_eq!(
            report.capabilities.provider_scan.state,
            CapabilityState::Degraded
        );
        assert!(report.can_scan_providers());
        assert!(report.can_test());
    }

    #[test]
    fn missing_required_settings_config_disables_provider_scan() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA user_version=16;
            CREATE TABLE providers (
                id TEXT, app_type TEXT, name TEXT,
                meta TEXT, is_current INTEGER
            );
            CREATE TABLE provider_endpoints (
                id INTEGER PRIMARY KEY, provider_id TEXT, app_type TEXT, url TEXT, added_at INTEGER
            );
            "#,
        )
        .unwrap();
        let report = compute_compatibility_report(&conn).unwrap();
        assert_eq!(
            report.capabilities.provider_scan.state,
            CapabilityState::Disabled
        );
        assert!(!report.can_scan_providers());
        assert!(!report.can_test());
        assert!(report
            .capabilities
            .provider_scan
            .missing_columns
            .iter()
            .any(|c| c == "settings_config"));
    }

    #[test]
    fn endpoints_missing_degrades_not_blocks_provider() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA user_version=16;
            CREATE TABLE providers (
                id TEXT, app_type TEXT, name TEXT, settings_config TEXT,
                website_url TEXT, category TEXT, created_at INTEGER, sort_index INTEGER,
                notes TEXT, icon TEXT, icon_color TEXT, meta TEXT, is_current INTEGER,
                in_failover_queue INTEGER
            );
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
            "#,
        )
        .unwrap();
        let report = compute_compatibility_report(&conn).unwrap();
        assert!(report.can_scan_providers());
        assert_eq!(
            report.capabilities.endpoint_scan.state,
            CapabilityState::Degraded
        );
        assert!(report.capabilities.direct_diagnosis.is_usable());
    }

    #[test]
    fn routing_unknown_does_not_block_provider_or_direct() {
        let conn = Connection::open_in_memory().unwrap();
        seed_core_shape(&conn, 16);
        // no proxy_config table
        let report = compute_compatibility_report(&conn).unwrap();
        assert!(report.can_scan_providers());
        assert!(report.can_test());
        assert_eq!(
            report.capabilities.routing_discovery.state,
            CapabilityState::Disabled
        );
        assert_eq!(
            report.capabilities.routing_diagnosis.state,
            CapabilityState::Disabled
        );
        assert_eq!(
            report.capabilities.direct_diagnosis.state,
            CapabilityState::Supported
        );
    }

    #[test]
    fn missing_providers_unsupported() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA user_version=1; CREATE TABLE foo(x);")
            .unwrap();
        let fp = compute_fingerprint(&conn).unwrap();
        assert_eq!(fp.status, CompatibilityStatus::Unsupported);
        assert!(!fp.status.can_test());

        let report = compute_compatibility_report(&conn).unwrap();
        assert!(!report.can_test());
        assert_eq!(
            report.capabilities.provider_scan.state,
            CapabilityState::Disabled
        );
    }

    #[test]
    fn manifest_matches_runtime_allowlist() {
        let raw = include_str!("../../../compatibility/manifest.json");
        let v: serde_json::Value = serde_json::from_str(raw).expect("manifest json");
        let fingerprints = v["ccSwitch"]["schemaFingerprints"]
            .as_array()
            .expect("schemaFingerprints");

        for entry in SCHEMA_ALLOWLIST {
            let found = fingerprints
                .iter()
                .find(|f| f["id"] == entry.id)
                .unwrap_or_else(|| panic!("manifest missing allowlist id {}", entry.id));
            assert_eq!(
                found["userVersion"].as_i64().unwrap() as i32,
                entry.user_version,
                "userVersion mismatch for {}",
                entry.id
            );
            let status = found["status"].as_str().unwrap();
            let expected = match entry.verification {
                VersionVerification::Verified => "verified",
                VersionVerification::KnownCompatible => "compatible",
                _ => "unknown",
            };
            assert_eq!(status, expected, "status mismatch for {}", entry.id);

            let tables: Vec<&str> = found["requiredTables"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap())
                .collect();
            assert_eq!(
                tables, entry.required_tables,
                "tables mismatch for {}",
                entry.id
            );

            let cols: Vec<&str> = found["providersColumns"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap())
                .collect();
            assert_eq!(
                cols, entry.providers_columns,
                "providersColumns mismatch for {}",
                entry.id
            );

            let ecols: Vec<&str> = found["providerEndpointsColumns"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap())
                .collect();
            assert_eq!(
                ecols, entry.provider_endpoints_columns,
                "providerEndpointsColumns mismatch for {}",
                entry.id
            );
        }

        for f in fingerprints {
            let id = f["id"].as_str().unwrap();
            assert!(
                SCHEMA_ALLOWLIST.iter().any(|e| e.id == id),
                "manifest id {id} missing from SCHEMA_ALLOWLIST"
            );
        }
    }
}
