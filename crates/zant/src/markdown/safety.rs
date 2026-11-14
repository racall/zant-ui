//! URL hardening for links and images (Streamdown's rehype-harden).
//!
//! The parser keeps every URL; the renderer asks `check` before making a span
//! clickable or loading an image. Blocked links render as plain text.

pub struct Policy<'a> {
    /// Allowed link prefixes. Empty = any URL with a safe scheme.
    pub allowed_link_prefixes: &'a [&'a str],
    /// Allowed image prefixes. Empty = any URL with a safe scheme.
    pub allowed_image_prefixes: &'a [&'a str],
    /// Allow `data:image/...` sources for images.
    pub allow_data_images: bool,
    /// Allow local paths (relative or absolute) for images.
    pub allow_local_images: bool,
    /// Autolink schemes to ignore entirely (Streamdown's
    /// `disableAutolinkProtocols`, e.g. "mailto").
    pub disabled_autolink_protocols: &'a [&'a str],
}

impl Default for Policy<'_> {
    fn default() -> Self {
        Policy {
            allowed_link_prefixes: &[],
            allowed_image_prefixes: &[],
            allow_data_images: true,
            allow_local_images: true,
            disabled_autolink_protocols: &[],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Link,
    Image,
}

const DANGEROUS: [&str; 4] = ["javascript:", "vbscript:", "file:", "data:"];

fn has_scheme(url: &str) -> Option<&str> {
    for (i, c) in url.bytes().enumerate() {
        if c == b':' {
            return if i > 0 { Some(&url[..i + 1]) } else { None };
        }
        if !(c.is_ascii_alphanumeric() || c == b'+' || c == b'-' || c == b'.') {
            return None;
        }
    }
    None
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text
            .as_bytes()
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// Returns true when `url` may be used as a link target / image source.
pub fn check(policy: &Policy<'_>, kind: Kind, raw: &str) -> bool {
    let url = raw.trim_matches([' ', '\t', '\r', '\n']);
    if url.is_empty() {
        return false;
    }
    if url == "#incomplete" {
        return false;
    }
    let scheme = has_scheme(url);
    if let Some(s) = scheme {
        'dangerous: for bad in DANGEROUS {
            if s.eq_ignore_ascii_case(bad) {
                if kind == Kind::Image
                    && policy.allow_data_images
                    && starts_with_ignore_case(url, "data:image/")
                {
                    break 'dangerous;
                }
                return false;
            }
        }
    } else if kind == Kind::Image && !policy.allow_local_images {
        return false;
    }
    let prefixes = if kind == Kind::Link {
        policy.allowed_link_prefixes
    } else {
        policy.allowed_image_prefixes
    };
    if prefixes.is_empty() {
        return true;
    }
    // relative URLs and fragments are always allowed for links
    if scheme.is_none() && kind == Kind::Link {
        return true;
    }
    for prefix in prefixes {
        if starts_with_ignore_case(url, prefix) {
            return true;
        }
    }
    false
}

pub fn autolink_allowed(policy: &Policy<'_>, url: &str) -> bool {
    let Some(scheme) = has_scheme(url) else {
        return true;
    };
    for protocol in policy.disabled_autolink_protocols {
        let p = protocol.trim_end_matches(':');
        if scheme.len() == p.len() + 1 && scheme[..p.len()].eq_ignore_ascii_case(p) {
            return false;
        }
    }
    true
}

/// True for URLs that leave the app (shown in the link-safety dialog).
pub fn is_external(url: &str) -> bool {
    has_scheme(url).is_some() || url.starts_with("www.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_schemes_are_blocked() {
        let p = Policy::default();
        assert!(!check(&p, Kind::Link, "javascript:alert(1)"));
        assert!(!check(&p, Kind::Link, " JavaScript:alert(1)"));
        assert!(check(&p, Kind::Link, "https://example.com"));
        assert!(check(&p, Kind::Image, "data:image/png;base64,AAAA"));
        assert!(!check(&p, Kind::Link, "data:text/html,<b>"));
        assert!(check(&p, Kind::Image, "images/a.png"));
    }

    #[test]
    fn prefix_allow_list() {
        let p = Policy {
            allowed_link_prefixes: &["https://docs.example.com"],
            ..Policy::default()
        };
        assert!(check(&p, Kind::Link, "https://docs.example.com/a"));
        assert!(!check(&p, Kind::Link, "https://evil.example.com"));
        assert!(check(&p, Kind::Link, "#section"));
    }
}
