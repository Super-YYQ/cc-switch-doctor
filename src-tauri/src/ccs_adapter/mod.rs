pub mod fingerprint;
pub mod managed_auth;
pub mod models;
pub mod normalize;
pub mod path_discovery;
pub mod readonly_db;
pub mod routing;
pub mod scan;

pub use fingerprint::{compute_fingerprint, CompatibilityStatus, SchemaFingerprint};
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
pub use scan::scan_database;
