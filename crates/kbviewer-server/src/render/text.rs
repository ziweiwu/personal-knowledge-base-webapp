//! Plain-text, source-code and CSV rendering.

use super::highlight::ClassedHighlighter;
use super::html::escape_text;
use comrak::adapters::SyntaxHighlighterAdapter;
use std::path::Path;
use std::sync::OnceLock;

/// Render a text or source file as a highlighted code block.
pub fn render_text(source: &str, path: &str) -> String {
    let language = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .or_else(|| {
            Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
        });

    // Loading syntect's syntax set costs tens of milliseconds and several megabytes;
    // the markdown path already shares one instance, and this path must too or browsing a
    // folder of source files re-pays it per file.
    static ADAPTER: OnceLock<ClassedHighlighter> = OnceLock::new();
    let adapter = ADAPTER.get_or_init(ClassedHighlighter::new);

    let mut highlighted = String::new();
    if adapter
        .write_highlighted(&mut highlighted, language.as_deref(), source)
        .is_err()
    {
        highlighted = escape_text(source);
    }
    format!("<pre class=\"code-file\"><code>{highlighted}</code></pre>")
}

/// Render delimited data as a table.
///
/// Falls back to `None` when the file does not parse as a consistent table, so the caller
/// can show it as plain text rather than a misleading half-parsed grid.
pub fn render_csv(source: &str, delimiter: char) -> Option<String> {
    let rows = parse_delimited(source, delimiter)?;
    let mut rows = rows.into_iter();
    let header = rows.next()?;
    if header.is_empty() {
        return None;
    }

    let mut html =
        String::from("<div class=\"table-scroll\"><table class=\"data-table\"><thead><tr>");
    for cell in &header {
        html.push_str(&format!("<th>{}</th>", escape_text(cell)));
    }
    html.push_str("</tr></thead><tbody>");

    for row in rows {
        html.push_str("<tr>");
        for index in 0..header.len() {
            let cell = row.get(index).map(String::as_str).unwrap_or("");
            html.push_str(&format!("<td>{}</td>", escape_text(cell)));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table></div>");
    Some(html)
}

/// RFC 4180 parsing: quoted fields may contain the delimiter, newlines, and doubled
/// quotes. A line-splitting parser gets all three wrong on real spreadsheet exports.
fn parse_delimited(source: &str, delimiter: char) -> Option<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            in_quotes = continue_quoted_field(ch, &mut chars, &mut field);
            continue;
        }

        match ch {
            '"' if field.is_empty() => in_quotes = true,
            c if c == delimiter => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }

    if in_quotes {
        return None; // Unterminated quote: the file is not valid delimited data.
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    (!rows.is_empty()).then_some(rows)
}

/// Consume one character from inside a quoted field, reporting whether the quoting is still
/// open: a doubled quote is a literal quote, a lone one closes the field.
fn continue_quoted_field(
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    field: &mut String,
) -> bool {
    if character != '"' {
        field.push(character);
        return true;
    }
    if chars.peek() == Some(&'"') {
        chars.next();
        field.push('"');
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_source_as_a_highlighted_block() {
        let html = render_text("fn main() {}\n", "src/main.rs");
        assert!(html.contains("class=\"code-file\""));
        assert!(html.contains("hl-"), "should be highlighted: {html}");
    }

    #[test]
    fn escapes_text_files_containing_markup() {
        let html = render_text("<img onerror=alert(1)>\n", "notes.txt");
        assert!(!html.contains("<img"), "raw tag leaked: {html}");
    }

    #[test]
    fn renders_a_simple_table() {
        let html = render_csv("a,b\n1,2\n", ',').unwrap();
        assert!(html.contains("<th>a</th>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn handles_a_quoted_field_containing_the_delimiter() {
        let html = render_csv("name,note\n\"Smith, John\",ok\n", ',').unwrap();
        assert!(html.contains("Smith, John"), "got {html}");
    }

    #[test]
    fn handles_a_quoted_field_containing_a_newline() {
        let rows = parse_delimited("a,b\n\"line one\nline two\",x\n", ',').unwrap();
        assert_eq!(
            rows.len(),
            2,
            "an embedded newline must not split the record"
        );
        assert_eq!(rows[1][0], "line one\nline two");
    }

    #[test]
    fn handles_doubled_quotes_inside_a_quoted_field() {
        let rows = parse_delimited("a\n\"she said \"\"hi\"\"\"\n", ',').unwrap();
        assert_eq!(rows[1][0], "she said \"hi\"");
    }

    #[test]
    fn pads_short_rows_rather_than_shifting_cells() {
        const COLUMNS: usize = 3;
        let html = render_csv("a,b,c\n1\n", ',').unwrap();
        assert_eq!(
            html.matches("<td>").count(),
            COLUMNS,
            "a ragged row must not misalign the table"
        );
    }

    #[test]
    fn refuses_data_with_an_unterminated_quote() {
        assert!(render_csv("a,b\n\"never closed\n", ',').is_none());
    }

    #[test]
    fn escapes_cell_contents() {
        let html = render_csv("a\n<script>alert(1)</script>\n", ',').unwrap();
        assert!(!html.contains("<script>"), "raw tag leaked: {html}");
    }

    #[test]
    fn supports_tab_separated_data() {
        let html = render_csv("a\tb\n1\t2\n", '\t').unwrap();
        assert!(html.contains("<th>a</th>") && html.contains("<td>2</td>"));
    }
}
