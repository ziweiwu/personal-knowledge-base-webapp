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

/// Reverse `encode_path`.
///
/// Returns `None` on anything that is not valid percent-encoded UTF-8, so a malformed URL
/// cannot be coerced into naming a document.
pub fn decode_path(encoded: &str) -> Option<String> {
    /// `%` plus two hex digits.
    const ESCAPE_LEN: usize = 3;
    const HEX: u32 = 16;

    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let digits = encoded.get(index + 1..index + ESCAPE_LEN)?;
            out.push(u8::from_str_radix(digits, HEX).ok()?);
            index += ESCAPE_LEN;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_survives_a_round_trip_through_the_url() {
        for path in [
            "notes/a.png",
            "Spaces And Caps.png",
            "unicode-\u{6807}\u{9898}.png",
            "a+b & c/d.png",
        ] {
            assert_eq!(decode_path(&encode_path(path)).as_deref(), Some(path));
        }
    }

    #[test]
    fn a_malformed_encoding_decodes_to_nothing_rather_than_guessing() {
        assert_eq!(decode_path("%ZZ"), None);
        assert_eq!(decode_path("%2"), None);
        assert_eq!(decode_path("%FF"), None, "not valid UTF-8");
    }

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
