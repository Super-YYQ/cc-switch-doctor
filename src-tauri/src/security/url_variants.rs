use super::origin::is_same_origin;
use url::Url;

const KNOWN_ENDPOINT_SUFFIXES: &[&str] = &[
    "/chat/completions",
    "/v1/chat/completions",
    "/responses",
    "/v1/responses",
    "/messages",
    "/v1/messages",
    "/completions",
    "/v1/completions",
];

/// Generate ordered, de-duplicated base URL candidates from the original base.
/// All candidates stay same-origin with the original.
pub fn normalize_base_candidates(original: &str, extra_endpoints: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if s.is_empty() {
            return;
        }
        if !is_same_origin(original, &s) && s != original.trim_end_matches('/') {
            // allow exact original even if parse fails later
            if Url::parse(original).is_ok()
                && Url::parse(&s).is_ok()
                && !is_same_origin(original, &s)
            {
                return;
            }
        }
        if !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    };

    let original_trim = original.trim().to_string();
    push(original_trim.clone());

    let no_slash = original_trim.trim_end_matches('/').to_string();
    push(no_slash.clone());

    let stripped = strip_known_endpoints(&no_slash);
    push(stripped.clone());

    // collapse /v1/v1
    let collapsed = collapse_dup_v1(&stripped);
    push(collapsed.clone());

    // add /v1
    if !collapsed.ends_with("/v1") {
        push(format!("{collapsed}/v1"));
    }
    // remove /v1
    if let Some(base) = collapsed.strip_suffix("/v1") {
        push(base.trim_end_matches('/').to_string());
    }

    for ep in extra_endpoints {
        let e = ep.trim().trim_end_matches('/').to_string();
        if e.is_empty() {
            continue;
        }
        if is_same_origin(original, &e) || Url::parse(original).is_err() {
            push(e.clone());
            let s = strip_known_endpoints(&e);
            push(s.clone());
            if !s.ends_with("/v1") {
                push(format!("{s}/v1"));
            }
            if let Some(base) = s.strip_suffix("/v1") {
                push(base.trim_end_matches('/').to_string());
            }
        }
    }

    // Final filter: same-origin only when original is a valid URL
    if Url::parse(original).is_ok() {
        out.retain(|u| is_same_origin(original, u) || u == original || u == &no_slash);
    }
    out
}

pub fn strip_known_endpoints(base: &str) -> String {
    let mut s = base.trim_end_matches('/').to_string();
    loop {
        let mut changed = false;
        for suf in KNOWN_ENDPOINT_SUFFIXES {
            if let Some(stripped) = s.strip_suffix(suf) {
                s = stripped.trim_end_matches('/').to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    s
}

fn collapse_dup_v1(s: &str) -> String {
    let mut out = s.to_string();
    while out.contains("/v1/v1") {
        out = out.replace("/v1/v1", "/v1");
    }
    out.trim_end_matches('/').to_string()
}

/// Join base + path without double slashes; path should start with /
pub fn join_url(base: &str, path: &str) -> String {
    let b = base.trim_end_matches('/');
    if path.is_empty() {
        return b.to_string();
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    // If base already ends with the path prefix, avoid duplication of /v1
    if path.starts_with("/v1/") && b.ends_with("/v1") {
        return format!("{b}{}", path.trim_start_matches("/v1"));
    }
    if path == "/v1" && b.ends_with("/v1") {
        return b.to_string();
    }
    format!("{b}{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_removes_v1() {
        let c = normalize_base_candidates("https://api.example.com", &[]);
        assert!(c.iter().any(|u| u == "https://api.example.com"));
        assert!(c.iter().any(|u| u == "https://api.example.com/v1"));
    }

    #[test]
    fn strips_chat_completions() {
        let s = strip_known_endpoints("https://api.example.com/v1/chat/completions");
        assert_eq!(s, "https://api.example.com/v1");
    }

    #[test]
    fn rejects_cross_origin_extra() {
        let c = normalize_base_candidates(
            "https://api.example.com",
            &["https://api.openai.com/v1".into()],
        );
        assert!(!c.iter().any(|u| u.contains("openai.com")));
    }

    #[test]
    fn collapse_double_v1() {
        let c = normalize_base_candidates("https://api.example.com/v1/v1", &[]);
        assert!(c.iter().any(|u| u == "https://api.example.com/v1"));
    }
}
