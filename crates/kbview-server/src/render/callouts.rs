//! Obsidian callout blocks: `> [!note] Optional title`.
//!
//! Rewritten in the source before parsing rather than transformed in the AST, because
//! CommonMark's HTML-block rules already give exactly the behaviour wanted here: the
//! wrapper is raw HTML, and the body between blank lines is still parsed as markdown.

use super::html::{escape_attr, escape_text};

const MAX_NESTING: usize = 4;

/// Rewrite every callout in `source`, innermost included, into `<div class="callout">`.
pub fn transform(source: &str) -> String {
    transform_at_depth(source, 0)
}

fn transform_at_depth(source: &str, depth: usize) -> String {
    if depth >= MAX_NESTING {
        return source.to_string();
    }

    // `split` yields a trailing empty element for a newline-terminated string;
    // dropping it keeps a document that needs no transformation byte-identical.
    let ends_with_newline = source.ends_with('\n');
    let mut lines: Vec<&str> = source.split('\n').collect();
    if ends_with_newline {
        lines.pop();
    }

    let mut out = rewrite_callouts(&lines, depth);
    if !ends_with_newline {
        while out.ends_with('\n') {
            out.pop();
        }
    }
    out
}

fn rewrite_callouts(lines: &[&str], depth: usize) -> String {
    let mut out = String::with_capacity(lines.iter().map(|line| line.len() + 1).sum());
    let mut index = 0usize;
    let mut in_fence = false;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            push_line(&mut out, line);
            index += 1;
            continue;
        }

        if in_fence {
            push_line(&mut out, line);
            index += 1;
            continue;
        }

        let Some(header) = parse_callout_header(line) else {
            push_line(&mut out, line);
            index += 1;
            continue;
        };

        index += 1;
        let body_lines = take_blockquote_body(lines, &mut index);
        let body = transform_at_depth(&body_lines.join("\n"), depth + 1);
        out.push_str(&render_callout(&header, &body));
    }
    out
}

/// Take the rest of the blockquote that opened the callout, advancing past it.
fn take_blockquote_body(lines: &[&str], index: &mut usize) -> Vec<String> {
    let mut body_lines: Vec<String> = Vec::new();
    while *index < lines.len() {
        let candidate = lines[*index].trim_start();
        if !candidate.starts_with('>') {
            break;
        }
        body_lines.push(strip_quote_marker(lines[*index]));
        *index += 1;
    }
    body_lines
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

struct CalloutHeader {
    kind: String,
    title: String,
    foldable: bool,
    collapsed: bool,
}

/// Match `> [!type]`, optionally followed by `+`/`-` (fold state) and a title.
fn parse_callout_header(line: &str) -> Option<CalloutHeader> {
    let content = line.trim_start().strip_prefix('>')?.trim_start();
    let rest = content.strip_prefix("[!")?;
    let (kind, after) = rest.split_once(']')?;
    if kind.is_empty()
        || !kind
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    let (foldable, collapsed, title) = match after.strip_prefix('+') {
        Some(t) => (true, false, t),
        None => match after.strip_prefix('-') {
            Some(t) => (true, true, t),
            None => (false, false, after),
        },
    };

    Some(CalloutHeader {
        kind: kind.to_lowercase(),
        title: title.trim().to_string(),
        foldable,
        collapsed,
    })
}

fn strip_quote_marker(line: &str) -> String {
    let trimmed = line.trim_start();
    match trimmed.strip_prefix('>') {
        Some(rest) => rest.strip_prefix(' ').unwrap_or(rest).to_string(),
        None => line.to_string(),
    }
}

fn render_callout(header: &CalloutHeader, body: &str) -> String {
    // Title defaults to the type name, which is what Obsidian shows.
    let title = if header.title.is_empty() {
        let mut chars = header.kind.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    } else {
        header.title.clone()
    };

    let kind = escape_attr(&header.kind);
    let title = escape_text(&title);
    let body = body.trim_end();

    // A foldable callout becomes a real `<details>`: the whole point of writing `[!note]-`
    // is to collapse a long aside, and rendering it as a plain div both ignores the
    // author's intent and gives the reader no way to collapse it.
    if header.foldable {
        let open = if header.collapsed { "" } else { " open" };
        return format!(
            "\n<details class=\"callout callout--foldable\" data-callout=\"{kind}\"{open}>\n             <summary class=\"callout-title\">{title}</summary>\n             <div class=\"callout-body\">\n\n{body}\n\n</div>\n</details>\n\n"
        );
    }

    format!(
        "\n<div class=\"callout\" data-callout=\"{kind}\">\n         <div class=\"callout-title\">{title}</div>\n         <div class=\"callout-body\">\n\n{body}\n\n</div>\n</div>\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_a_basic_callout() {
        let out = transform("> [!note]\n> Body text\n");
        assert!(out.contains("data-callout=\"note\""));
        assert!(out.contains("Body text"));
        assert!(!out.contains("> [!note]"));
    }

    #[test]
    fn uses_the_type_name_as_the_default_title() {
        let out = transform("> [!warning]\n> careful\n");
        assert!(out.contains(">Warning</div>"), "got: {out}");
    }

    #[test]
    fn keeps_an_explicit_title() {
        let out = transform("> [!tip] Do this instead\n> body\n");
        assert!(out.contains("Do this instead"));
    }

    #[test]
    fn a_foldable_callout_becomes_a_real_disclosure() {
        let collapsed = transform("> [!info]- Hidden\n> body\n");
        assert!(collapsed.contains("<details"), "got {collapsed}");
        assert!(
            collapsed.contains("<summary"),
            "the title must be the disclosure control"
        );
        assert!(
            !collapsed.contains(
                "<details class=\"callout callout--foldable\" data-callout=\"info\" open>"
            ),
            "a `-` marker means it starts collapsed"
        );

        let expanded = transform("> [!info]+ Shown\n> body\n");
        assert!(
            expanded.contains(" open>"),
            "a `+` marker means it starts expanded: {expanded}"
        );
    }

    #[test]
    fn a_plain_callout_is_not_a_disclosure() {
        let out = transform("> [!note]\n> body\n");
        assert!(!out.contains("<details"), "only `+`/`-` callouts fold");
    }

    #[test]
    fn leaves_an_ordinary_blockquote_alone() {
        let src = "> just a quote\n> more\n";
        assert_eq!(transform(src), src);
    }

    #[test]
    fn ignores_callout_syntax_inside_a_code_fence() {
        let src = "```\n> [!note]\n> body\n```\n";
        assert_eq!(
            transform(src),
            src,
            "a code sample showing callout syntax must survive"
        );
    }

    #[test]
    fn handles_a_nested_callout() {
        let out = transform("> [!note] Outer\n> > [!tip] Inner\n> > inner body\n");
        assert!(out.contains("data-callout=\"note\""));
        assert!(
            out.contains("data-callout=\"tip\""),
            "inner callout must transform too: {out}"
        );
    }

    #[test]
    fn escapes_markup_in_the_title() {
        let out = transform("> [!note] <script>alert(1)</script>\n> body\n");
        assert!(!out.contains("<script>"), "title must be escaped: {out}");
        assert!(out.contains("&lt;script&gt;"));
    }

    #[test]
    fn a_malformed_marker_is_not_a_callout() {
        assert_eq!(transform("> [!] body\n"), "> [!] body\n");
        assert_eq!(
            transform("> [!no closing bracket\n"),
            "> [!no closing bracket\n"
        );
    }

    #[test]
    fn content_after_a_callout_is_preserved() {
        let out = transform("> [!note]\n> inside\n\nAfter the callout.\n");
        assert!(out.contains("After the callout."));
    }
}
