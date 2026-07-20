use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

static BEARER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(bearer\s+)[a-z0-9._\-]{8,}").expect("re"));
static SK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(sk-[a-z0-9_\-]{8,})").expect("re"));
static JWT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\beyJ[a-zA-Z0-9_\-]+=*\.[a-zA-Z0-9_\-]+=*\.[a-zA-Z0-9_\-+/=]*\b").expect("re")
});
static QUERY_SECRET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)([?&](key|api_key|apikey|token|access_token|auth|password|secret)=)([^&]+)")
        .expect("re")
});

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

pub fn sanitize_url_for_display(raw: &str) -> String {
    let Ok(mut u) = url::Url::parse(raw) else {
        return SecretRedactor::new().redact(raw);
    };
    let _ = u.set_username("");
    let _ = u.set_password(None);
    // Redact sensitive query params
    let pairs: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| {
            let kl = k.to_ascii_lowercase();
            if matches!(
                kl.as_str(),
                "key"
                    | "api_key"
                    | "apikey"
                    | "token"
                    | "access_token"
                    | "auth"
                    | "password"
                    | "secret"
            ) {
                (k.to_string(), "***".into())
            } else {
                (k.to_string(), v.to_string())
            }
        })
        .collect();
    if !pairs.is_empty() {
        u.query_pairs_mut().clear();
        for (k, v) in pairs {
            u.query_pairs_mut().append_pair(&k, &v);
        }
    }
    u.set_fragment(None);
    u.to_string()
}

pub fn sanitize_error_body(body: &str, redactor: &SecretRedactor) -> String {
    let mut s = redactor.redact(body);
    const MAX: usize = 64 * 1024;
    if s.len() > MAX {
        s.truncate(MAX);
        s.push_str("…[truncated]");
    }
    s
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
    fn sanitize_url_strips_userinfo() {
        let s = sanitize_url_for_display("https://user:pass@api.example.com/v1?key=abc12345");
        assert!(!s.contains("user"));
        assert!(!s.contains("pass"));
        assert!(s.contains("***"));
    }
}
