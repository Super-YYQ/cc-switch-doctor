pub mod origin;
pub mod redact;
pub mod url_variants;

pub use origin::{is_same_origin, SameOriginPolicy};
pub use redact::{mask_api_key, sanitize_error_body, sanitize_url_for_display, SecretRedactor};
pub use url_variants::{normalize_base_candidates, strip_known_endpoints};
