use once_cell::sync::Lazy;
use regex::Regex;

static BEARER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(bearer\s+)[a-z0-9._\-]{8,}").expect("re"));
static SK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(sk-[a-z0-9_\-]{8,})").expect("re"));
static JWT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\beyJ[a-zA-Z0-9_\-]+=*\.[a-zA-Z0-9_\-]+=*\.[a-zA-Z0-9_\-+/=]*\b").expect("re")
});
static QUERY_SECRET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)([?&](key|api[_-]?key|x-api-key|x-goog-api-key|token|access_token|auth|password|secret|signature)=)([^&]*)",
    )
    .expect("re")
});

#[derive(Debug, Default, Clone)]
pub struct SecretRedactor {
    known: Vec<String>,
}

impl SecretRedactor {
    pub fn new() -> Self {
        Self { known: Vec::new() }
    }

    pub fn register_key(&mut self, key: &str) {
        let k = key.trim();
        if k.len() >= 8 && !self.known.iter().any(|x| x == k) {
            self.known.push(k.to_string());
        }
    }

    pub fn redact(&self, input: &str) -> String {
        let mut out = input.to_string();
        for k in &self.known {
            if out.contains(k) {
                out = out.replace(k, &mask_api_key(k));
            }
        }
        out = BEARER_RE.replace_all(&out, "${1}***").to_string();
        out = SK_RE
            .replace_all(&out, |caps: &regex::Captures| mask_api_key(&caps[1]))
            .to_string();
        out = JWT_RE.replace_all(&out, "eyJ***").to_string();
        out = QUERY_SECRET_RE
            .replace_all(&out, |caps: &regex::Captures| format!("{}***", &caps[1]))
            .to_string();
        out
    }
}

pub fn mask_api_key(key: &str) -> String {
    let k = key.trim();
    if k.is_empty() {
        return String::new();
    }
    if k.len() <= 8 {
        return "***".into();
    }
    let prefix: String = k.chars().take(6).collect();
    let suffix: String = k
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{prefix}…{suffix}")
}

/// UTF-8 safe truncation on a char boundary, with ellipsis when shortened.
pub fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    if max_bytes == 0 {
        return String::new();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

/// Conservative URL sanitization for any frontend/log display:
/// - strip userinfo + fragment
/// - mask ALL query values (keep keys)
/// - replace known full keys in path via redactor when provided later
pub fn sanitize_url_for_display(raw: &str) -> String {
    let Ok(mut u) = url::Url::parse(raw) else {
        return SecretRedactor::new().redact(raw);
    };
    let _ = u.set_username("");
    let _ = u.set_password(None);
    let pairs: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, _v)| (k.to_string(), "***".into()))
        .collect();
    if pairs.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut().clear();
        for (k, v) in pairs {
            u.query_pairs_mut().append_pair(&k, &v);
        }
    }
    u.set_fragment(None);
    u.to_string()
}

pub fn sanitize_url_with_redactor(raw: &str, redactor: &SecretRedactor) -> String {
    redactor.redact(&sanitize_url_for_display(raw))
}

pub fn sanitize_error_body(body: &str, redactor: &SecretRedactor) -> String {
    let s = redactor.redact(body);
    truncate_utf8(&s, 64 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_key() {
        let m = mask_api_key("sk-abcdefghijklmnop");
        assert!(m.starts_with("sk-abc"));
        assert!(m.contains('…'));
        assert!(!m.contains("defghijklmnop"));
    }

    #[test]
    fn redacts_bearer_and_query() {
        let mut r = SecretRedactor::new();
        r.register_key("super-secret-key-value");
        let out = r.redact(
            "Authorization: Bearer super-secret-key-value https://x.test?api_key=super-secret-key-value",
        );
        assert!(!out.contains("super-secret-key-value"));
    }

    #[test]
    fn sanitize_url_strips_userinfo_and_all_query_values() {
        let s = sanitize_url_for_display(
            "https://user:pass@api.example.com/v1?key=abc12345&region=us-east",
        );
        assert!(!s.contains("user"));
        assert!(!s.contains("pass"));
        assert!(!s.contains("abc12345"));
        assert!(!s.contains("us-east"));
        assert!(s.contains("key=***"));
        assert!(s.contains("region=***"));
    }

    #[test]
    fn sanitize_unknown_query_and_path_key() {
        let mut r = SecretRedactor::new();
        r.register_key("sk-path-secret-ABCDEFGH");
        let s = sanitize_url_with_redactor(
            "https://api.example.com/v1/sk-path-secret-ABCDEFGH?x-api-key=other",
            &r,
        );
        assert!(!s.contains("sk-path-secret-ABCDEFGH"));
        assert!(s.contains("x-api-key=***") || s.contains("***"));
    }

    #[test]
    fn truncate_utf8_cjk_and_emoji() {
        let s = "你好世界😀测试";
        let t = truncate_utf8(s, 7);
        assert!(t.ends_with('…'));
        // must not panic and remain valid utf8
        assert!(t.is_char_boundary(t.len() - "…".len()) || t.ends_with('…'));
        assert_eq!(truncate_utf8("abc", 10), "abc");
        assert_eq!(truncate_utf8("", 5), "");
        // mid multi-byte
        let mid = truncate_utf8("áéíóú", 3);
        assert!(std::str::from_utf8(mid.as_bytes()).is_ok());
    }
}
