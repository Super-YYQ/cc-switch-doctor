use url::Url;

#[derive(Debug, Clone)]
pub struct SameOriginPolicy {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
}

impl SameOriginPolicy {
    pub fn from_url(u: &Url) -> Option<Self> {
        Some(Self {
            scheme: u.scheme().to_string(),
            host: u.host_str()?.to_ascii_lowercase(),
            port: u.port(),
        })
    }

    pub fn parse_url(raw: &str) -> Option<Self> {
        Url::parse(raw).ok().as_ref().and_then(Self::from_url)
    }

    pub fn allows(&self, other: &Url) -> bool {
        if other.scheme() != self.scheme {
            return false;
        }
        match other.host_str() {
            Some(h) if h.eq_ignore_ascii_case(&self.host) => {}
            _ => return false,
        }
        // Treat default ports as equal to omitted ports
        let other_port = effective_port(other);
        let self_port = effective_port_parts(&self.scheme, self.port);
        other_port == self_port
    }
}

fn effective_port(u: &Url) -> u16 {
    u.port_or_known_default().unwrap_or(0)
}

fn effective_port_parts(scheme: &str, port: Option<u16>) -> u16 {
    port.unwrap_or(match scheme {
        "https" => 443,
        "http" => 80,
        _ => 0,
    })
}

pub fn is_same_origin(original: &str, candidate: &str) -> bool {
    let Some(policy) = SameOriginPolicy::parse_url(original) else {
        return false;
    };
    let Ok(u) = Url::parse(candidate) else {
        return false;
    };
    policy.allows(&u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_host_ok() {
        assert!(is_same_origin(
            "https://api.example.com/v1",
            "https://api.example.com/v1/chat/completions"
        ));
    }

    #[test]
    fn cross_host_blocked() {
        assert!(!is_same_origin(
            "https://api.example.com",
            "https://api.openai.com/v1"
        ));
    }

    #[test]
    fn scheme_change_blocked() {
        assert!(!is_same_origin(
            "https://api.example.com",
            "http://api.example.com"
        ));
    }
}
