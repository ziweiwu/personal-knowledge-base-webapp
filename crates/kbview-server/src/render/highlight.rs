//! Syntax highlighting that respects the user's theme.
//!
//! comrak's bundled syntect adapter writes inline `style="color:#..."`, which locks code
//! blocks to one palette and makes them unreadable when the page switches to dark mode.
//! This adapter emits CSS classes instead, so the frontend colours them from the same
//! custom properties as everything else and both themes work.

use comrak::adapters::SyntaxHighlighterAdapter;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

const CLASS_PREFIX: &str = "hl-";

pub struct ClassedHighlighter {
    syntaxes: SyntaxSet,
}

impl Default for ClassedHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassedHighlighter {
    pub fn new() -> Self {
        Self {
            syntaxes: SyntaxSet::load_defaults_newlines(),
        }
    }
}

impl SyntaxHighlighterAdapter for ClassedHighlighter {
    fn write_highlighted(
        &self,
        output: &mut dyn fmt::Write,
        lang: Option<&str>,
        code: &str,
    ) -> fmt::Result {
        let syntax = lang
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .and_then(|l| {
                self.syntaxes
                    .find_syntax_by_token(l)
                    .or_else(|| self.syntaxes.find_syntax_by_extension(l))
            })
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());

        let mut generator = ClassedHTMLGenerator::new_with_class_style(
            syntax,
            &self.syntaxes,
            ClassStyle::SpacedPrefixed {
                prefix: CLASS_PREFIX,
            },
        );

        for line in LinesWithEndings::from(code) {
            // An unparseable line is not worth losing the document over: fall back to
            // emitting the code unhighlighted rather than failing the whole render.
            if generator
                .parse_html_for_line_which_includes_newline(line)
                .is_err()
            {
                return output.write_str(&super::html::escape_text(code));
            }
        }
        output.write_str(&generator.finalize())
    }

    fn write_pre_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        write_tag(output, "pre", attributes)
    }

    fn write_code_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        write_tag(output, "code", attributes)
    }
}

fn write_tag(
    output: &mut dyn fmt::Write,
    tag: &str,
    attributes: HashMap<&'static str, Cow<'_, str>>,
) -> fmt::Result {
    output.write_str("<")?;
    output.write_str(tag)?;
    // Sorted so identical input always produces identical output, which keeps the render
    // cache and any golden tests stable.
    let mut pairs: Vec<_> = attributes.into_iter().collect();
    pairs.sort_by_key(|(key, _)| *key);
    for (key, value) in pairs {
        write!(output, " {}=\"{}\"", key, super::html::escape_attr(&value))?;
    }
    output.write_str(">")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn highlight(lang: Option<&str>, code: &str) -> String {
        let mut out = String::new();
        ClassedHighlighter::new()
            .write_highlighted(&mut out, lang, code)
            .unwrap();
        out
    }

    #[test]
    fn emits_classes_rather_than_inline_colours() {
        let html = highlight(Some("rust"), "fn main() {}\n");
        assert!(
            html.contains("class=\"hl-"),
            "expected prefixed classes, got: {html}"
        );
        assert!(
            !html.contains("style=\"color"),
            "inline colours would break dark mode"
        );
    }

    #[test]
    fn an_unknown_language_still_renders_the_code() {
        let html = highlight(Some("not-a-real-language"), "some code\n");
        assert!(html.contains("some code"));
    }

    #[test]
    fn a_missing_language_still_renders_the_code() {
        let html = highlight(None, "plain text\n");
        assert!(html.contains("plain text"));
    }

    #[test]
    fn code_is_escaped() {
        let html = highlight(Some("html"), "<script>alert(1)</script>\n");
        assert!(!html.contains("<script"), "raw script tag leaked: {html}");
        assert_eq!(
            text_content(&html),
            "<script>alert(1)</script>\n",
            "highlighting must preserve the code exactly, only escaped"
        );
    }

    /// Strip highlight markup and decode entities, leaving the code as the reader sees it.
    /// syntect splits tokens across spans, so the original text is never contiguous in
    /// the raw HTML and has to be reassembled before it can be compared.
    fn text_content(html: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for ch in html.chars() {
            match ch {
                '<' => in_tag = true,
                '>' if in_tag => in_tag = false,
                c if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    }

    #[test]
    fn attributes_are_escaped_and_ordered() {
        let mut out = String::new();
        let mut attrs: HashMap<&'static str, Cow<'_, str>> = HashMap::new();
        attrs.insert("class", Cow::Borrowed("language-rust"));
        attrs.insert("data-x", Cow::Borrowed("\"><script>"));
        write_tag(&mut out, "pre", attrs).unwrap();
        assert!(out.starts_with("<pre class=\"language-rust\""), "got {out}");
        assert!(!out.contains("<script>"));
    }
}
