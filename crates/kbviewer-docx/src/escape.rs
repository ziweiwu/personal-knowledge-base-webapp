//! HTML escaping and URL vetting.
//!
//! Everything that reaches the output HTML from the document passes through
//! here. Word files carry arbitrary author text, so nothing is ever
//! interpolated raw.

/// Append `raw` to `out`, escaping every character that has meaning in HTML
/// text or in a double-quoted attribute value.
pub(crate) fn push_escaped(out: &mut String, raw: &str) {
    for character in raw.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(character),
        }
    }
}

/// Escape `raw` into a new string.
#[cfg(test)]
fn escaped(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    push_escaped(&mut out, raw);
    out
}

/// URL schemes an `<a href>` may use. Anything else -- `javascript:`,
/// `data:`, `vbscript:` -- is dropped, because a Word hyperlink is author
/// content and would otherwise be a script injection vector.
const ALLOWED_SCHEMES: [&str; 6] = ["http", "https", "mailto", "tel", "ftp", "ftps"];

/// Return the URL unchanged if it is safe to emit, otherwise `None`.
///
/// Scheme-relative and relative URLs, and bare fragments, are allowed; a URL
/// with an explicit scheme is allowed only if that scheme is in
/// [`ALLOWED_SCHEMES`].
pub(crate) fn sanitize_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return None;
    }
    match scheme_of(trimmed) {
        Some(scheme) => ALLOWED_SCHEMES
            .contains(&scheme.as_str())
            .then(|| trimmed.to_string()),
        None => Some(trimmed.to_string()),
    }
}

/// Extract the URL scheme, if the string starts with one.
fn scheme_of(url: &str) -> Option<String> {
    let mut scheme = String::new();
    for character in url.chars() {
        match character {
            ':' if !scheme.is_empty() => return Some(scheme.to_ascii_lowercase()),
            'a'..='z' | 'A'..='Z' => scheme.push(character),
            '0'..='9' | '+' | '-' | '.' if !scheme.is_empty() => scheme.push(character),
            _ => return None,
        }
    }
    None
}

/// Longest relationship id worth accepting. Real ones are `rId` and a few
/// digits, so this is generous by an order of magnitude.
const MAX_REL_ID_CHARS: usize = 64;

/// Whether a relationship id is safe to splice into a URL path segment.
///
/// Real ids look like `rId7`; rejecting anything else keeps a hostile file
/// from escaping the caller's `media_base` prefix with `../`.
pub(crate) fn is_safe_rel_id(rel_id: &str) -> bool {
    !rel_id.is_empty()
        && rel_id.len() <= MAX_REL_ID_CHARS
        && rel_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html_metacharacters() {
        assert_eq!(
            escaped(r#"<script>a & b "c" 'd'</script>"#),
            "&lt;script&gt;a &amp; b &quot;c&quot; &#39;d&#39;&lt;/script&gt;"
        );
    }

    #[test]
    fn allows_ordinary_urls() {
        assert_eq!(
            sanitize_url(" https://example.com/a?b=1 ").as_deref(),
            Some("https://example.com/a?b=1")
        );
        assert_eq!(sanitize_url("#anchor").as_deref(), Some("#anchor"));
        assert_eq!(
            sanitize_url("mailto:a@b.com").as_deref(),
            Some("mailto:a@b.com")
        );
    }

    #[test]
    fn rejects_script_urls() {
        assert_eq!(sanitize_url("javascript:alert(1)"), None);
        assert_eq!(sanitize_url("JavaScript:alert(1)"), None);
        assert_eq!(sanitize_url("data:text/html,<b>"), None);
        assert_eq!(sanitize_url("   "), None);
    }

    #[test]
    fn rejects_traversal_in_rel_ids() {
        assert!(is_safe_rel_id("rId12"));
        assert!(!is_safe_rel_id("../../etc/passwd"));
        assert!(!is_safe_rel_id(""));
    }
}
