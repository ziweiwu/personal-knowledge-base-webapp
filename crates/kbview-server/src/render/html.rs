//! HTML escaping helpers.
//!
//! Document content is user-authored and may contain anything. Every value the renderer
//! interpolates into markup goes through one of these, so escaping is never a decision
//! made at the call site.

pub fn escape_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn escape_attr(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Percent-encode a path for use in a URL, leaving `/` as a separator.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup_characters_in_text() {
        assert_eq!(
            escape_text("<script>alert(1)</script>"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
        assert_eq!(escape_text("a & b"), "a &amp; b");
    }

    #[test]
    fn escapes_quotes_in_attributes() {
        assert_eq!(
            escape_attr("\" onload=\"evil()"),
            "&quot; onload=&quot;evil()"
        );
        assert_eq!(escape_attr("it's"), "it&#39;s");
    }

    #[test]
    fn encodes_spaces_and_unicode_but_keeps_separators() {
        assert_eq!(encode_path("notes/My Note.md"), "notes/My%20Note.md");
        assert_eq!(
            encode_path("references/知识.md"),
            "references/%E7%9F%A5%E8%AF%86.md"
        );
    }

    #[test]
    fn encodes_characters_that_would_break_out_of_an_attribute() {
        assert_eq!(encode_path("a\"b.md"), "a%22b.md");
        assert_eq!(encode_path("a?b#c.md"), "a%3Fb%23c.md");
    }
}
