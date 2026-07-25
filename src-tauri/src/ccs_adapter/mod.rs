pub mod fingerprint;
pub mod managed_auth;
pub mod model_semantics;
pub mod models;
pub mod normalize;
pub mod path_discovery;
pub mod readonly_db;
pub mod routing;
pub mod routing_profile;
pub mod scan;

pub use fingerprint::{
    compute_compatibility_report, compute_fingerprint, CapabilityState, CapabilityStatus,
    CompatibilityReport, CompatibilityStatus, SchemaCapabilities, SchemaFingerprint,
    VersionVerification,
};
pub use model_semantics::{
    models_equivalent_after_local_normalize, strip_claude_one_m_marker, ModelCandidate,
    ModelCandidateSource, ONE_M_CONTEXT_MARKER,
};
pub use models::{
    AppType, AuthKind, DiscoveryInfo, NormalizedProvider, ProtocolKind, ProviderKind,
    ProviderListItem, ProviderScanView, SchemaInfoView,
};
pub use path_discovery::discover_database_paths;
pub use readonly_db::open_readonly;
pub use routing::{
    discover_routing_status_sync, loopback_connect_host, route_base_url, RoutingStatusView,
    CCS_PROXY_PLACEHOLDER_TOKEN,
};
pub use routing_profile::{
    active_routing_profile, client_route_model, default_direct_model_guess, route_profile_verified,
};
pub use scan::scan_database;
