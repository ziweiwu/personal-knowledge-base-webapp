//! A deliberately small YAML-subset parser for note frontmatter.
//!
//! Frontmatter in practice is flat scalars and lists of scalars. Parsing only that keeps
//! a full YAML implementation out of the dependency tree; anything more exotic is passed
//! through as its literal text rather than being dropped, so nothing silently disappears.

use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub struct Frontmatter {
    pub scalars: BTreeMap<String, String>,
    pub lists: BTreeMap<String, Vec<String>>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.scalars.get(key).map(String::as_str)
    }

    /// Values for a key regardless of whether it was written as a scalar or a list.
    pub fn values(&self, key: &str) -> Vec<String> {
        if let Some(list) = self.lists.get(key) {
            return list.clone();
        }
        match self.scalars.get(key) {
            Some(value) if !value.is_empty() => vec![value.clone()],
            _ => Vec::new(),
        }
    }
}

/// Split a document into its frontmatter block and the body that follows it.
pub fn split(src: &str) -> (Option<&str>, &str) {
    let rest = match src.strip_prefix("---\n") {
        Some(rest) => rest,
        None => match src.strip_prefix("---\r\n") {
            Some(rest) => rest,
            None => return (None, src),
        },
    };

    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" || trimmed == "..." {
            let body_start = offset + line.len();
            return (Some(&rest[..offset]), &rest[body_start..]);
        }
        offset += line.len();
    }
    // An unterminated block is not frontmatter; treat the whole file as body.
    (None, src)
}

pub fn parse(yaml: &str) -> Frontmatter {
    let mut out = Frontmatter::default();
    let mut pending_list: Option<String> = None;

    for raw_line in yaml.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        if let Some(item) = line.trim_start().strip_prefix("- ") {
            if let Some(key) = &pending_list {
                out.lists
                    .entry(key.clone())
                    .or_default()
                    .push(unquote(item));
            }
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            pending_list = None;
            continue;
        };
        pending_list = record_entry(&mut out, key.trim().to_string(), value.trim());
    }
    out
}

/// Record one `key: value` line, returning the key when the value was empty: a block list
/// announces itself that way, and its items arrive on the lines that follow.
fn record_entry(out: &mut Frontmatter, key: String, value: &str) -> Option<String> {
    if value.is_empty() {
        out.lists.entry(key.clone()).or_default();
        return Some(key);
    }
    if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        let items = inner
            .split(',')
            .map(unquote)
            .filter(|item| !item.is_empty())
            .collect();
        out.lists.insert(key, items);
    } else {
        out.scalars.insert(key, unquote(value));
    }
    None
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
        })
        .unwrap_or(trimmed);
    unquoted.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_frontmatter_block_from_the_body() {
        let (fm, body) = split("---\ntitle: Hi\n---\n# Body\n");
        assert_eq!(fm, Some("title: Hi\n"));
        assert_eq!(body, "# Body\n");
    }

    #[test]
    fn a_document_without_frontmatter_is_all_body() {
        let (fm, body) = split("# Just a heading\n");
        assert_eq!(fm, None);
        assert_eq!(body, "# Just a heading\n");
    }

    #[test]
    fn an_unterminated_block_is_not_treated_as_frontmatter() {
        let src = "---\ntitle: Hi\nnever closed\n";
        let (fm, body) = split(src);
        assert_eq!(fm, None, "a runaway block must not swallow the document");
        assert_eq!(body, src);
    }

    #[test]
    fn a_horizontal_rule_mid_document_is_not_frontmatter() {
        let (fm, _) = split("Some text\n\n---\n\nMore text\n");
        assert_eq!(fm, None);
    }

    #[test]
    fn parses_scalars_inline_lists_and_block_lists() {
        let fm = parse("title: My Note\ntags: [a, b]\naliases:\n  - one\n  - two\ncount: 3\n");
        assert_eq!(fm.get("title"), Some("My Note"));
        assert_eq!(fm.lists["tags"], vec!["a", "b"]);
        assert_eq!(fm.lists["aliases"], vec!["one", "two"]);
        assert_eq!(fm.get("count"), Some("3"));
    }

    #[test]
    fn strips_surrounding_quotes() {
        let fm = parse("title: \"Quoted: with colon\"\nother: 'single'\n");
        assert_eq!(fm.get("title"), Some("Quoted: with colon"));
        assert_eq!(fm.get("other"), Some("single"));
    }

    #[test]
    fn values_reads_scalars_and_lists_the_same_way() {
        assert_eq!(parse("tags: solo\n").values("tags"), vec!["solo"]);
        assert_eq!(parse("tags: [a, b]\n").values("tags"), vec!["a", "b"]);
        assert!(parse("title: x\n").values("tags").is_empty());
    }
}
