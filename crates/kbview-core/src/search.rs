//! Full-text search by linear scan.
//!
//! At this corpus size (~1 MB of text) scanning every document costs well under a
//! millisecond, so there is no inverted index to build, invalidate, or keep consistent
//! with the filesystem. If the corpus grew by orders of magnitude this is the one module
//! that would be replaced, and nothing else would need to change.

use crate::index::{Document, Index};
use crate::kinds::DocumentKind;
use crate::model::SearchHit;

const SNIPPET_CONTEXT: usize = 60;

/// Scores are relative, not absolute: they exist only to order results.
const SCORE_TITLE_EXACT: i32 = 1000;
const SCORE_TITLE_PREFIX: i32 = 500;
const SCORE_TITLE_CONTAINS: i32 = 250;
const SCORE_PATH_CONTAINS: i32 = 120;
const SCORE_TAG_MATCH: i32 = 200;
const SCORE_HEADING_MATCH: i32 = 60;
const SCORE_PER_BODY_HIT: i32 = 10;
const MAX_BODY_HITS_COUNTED: usize = 10;

pub fn search(index: &Index, query: &str, limit: usize) -> Vec<SearchHit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut hits: Vec<SearchHit> = index
        .documents
        .values()
        .filter_map(|document| score_document(document, &needle))
        .collect();

    // Sort by score, then path, so equal scores do not reorder between identical queries.
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    hits.truncate(limit);
    hits
}

/// One document's hit for `needle`, or `None` when nothing in it matched.
fn score_document(document: &Document, needle: &str) -> Option<SearchHit> {
    let (body_score, body_snippet) = score_body(document, needle);
    let score = score_metadata(document, needle) + body_score;
    if score == 0 {
        return None;
    }

    Some(SearchHit {
        path: document.path.clone(),
        title: document.title.clone(),
        kind: document.kind,
        snippet: if body_snippet.is_empty() {
            document.title.clone()
        } else {
            body_snippet
        },
        score,
    })
}

/// Title, path and tags: what a document is called counts for more than what it says.
fn score_metadata(document: &Document, needle: &str) -> i32 {
    let title = document.title.to_lowercase();
    let mut score = if title == needle {
        SCORE_TITLE_EXACT
    } else if title.starts_with(needle) {
        SCORE_TITLE_PREFIX
    } else if title.contains(needle) {
        SCORE_TITLE_CONTAINS
    } else {
        0
    };

    if document.path.to_lowercase().contains(needle) {
        score += SCORE_PATH_CONTAINS;
    }
    if document
        .tags
        .iter()
        .any(|tag| tag.to_lowercase().contains(needle))
    {
        score += SCORE_TAG_MATCH;
    }
    score
}

/// Body matches, and the snippet shown for the first of them.
fn score_body(document: &Document, needle: &str) -> (i32, String) {
    if !document.kind.is_searchable() {
        return (0, String::new());
    }
    let Some(stored) = &document.content else {
        return (0, String::new());
    };
    // Search the prose, not the YAML header. Frontmatter is already searched through the
    // title and tag scores, and including it here made every snippet open with
    // "--- type: guide tags: …" instead of the sentence the reader was looking for.
    let content = match document.kind {
        DocumentKind::Markdown => crate::frontmatter::split(stored).1,
        _ => stored.as_str(),
    };
    let body_hits = count_ignoring_case(content, needle);
    if body_hits == 0 {
        return (0, String::new());
    }

    let mut score = SCORE_PER_BODY_HIT * body_hits.min(MAX_BODY_HITS_COUNTED) as i32;
    let Some(match_start) = find_ignoring_case(content, needle) else {
        return (score, String::new());
    };
    if is_heading_line(content, match_start) {
        score += SCORE_HEADING_MATCH;
    }
    (score, build_snippet(content, match_start, needle.len()))
}

/// Find `needle` (already lowercased) in `haystack`, returning a byte offset **into
/// `haystack` itself**.
///
/// Searching a `to_lowercase()` copy and then slicing the original is wrong: lowercasing
/// is not length-preserving (`ẞ` becomes `ss`, `İ` becomes two chars), so the offsets
/// drift apart and the slice lands mid-character. That panics the request handler, and it
/// only shows up for documents containing such characters — which a multilingual vault
/// certainly has.
fn find_ignoring_case(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack
        .char_indices()
        .find(|(offset, _)| matches_at(haystack, *offset, needle))
        .map(|(offset, _)| offset)
}

fn count_ignoring_case(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack
        .char_indices()
        .filter(|(offset, _)| matches_at(haystack, *offset, needle))
        .count()
}

fn matches_at(haystack: &str, offset: usize, needle: &str) -> bool {
    let mut candidate = haystack[offset..].chars().flat_map(char::to_lowercase);
    let mut wanted = needle.chars();
    loop {
        match (wanted.next(), candidate.next()) {
            (None, _) => return true,
            (Some(_), None) => return false,
            (Some(want), Some(got)) if want != got => return false,
            _ => {}
        }
    }
}

/// Context around a match, with the match wrapped in `**` for the UI to style.
fn build_snippet(content: &str, match_start: usize, match_len: usize) -> String {
    let match_len = match_len.min(content.len().saturating_sub(match_start));
    let start = floor_char_boundary(content, match_start.saturating_sub(SNIPPET_CONTEXT));
    let end = ceil_char_boundary(
        content,
        (match_start + match_len + SNIPPET_CONTEXT).min(content.len()),
    );
    let match_end = ceil_char_boundary(content, match_start + match_len);

    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.push_str(content[start..match_start].trim_start());
    snippet.push_str("**");
    snippet.push_str(&content[match_start..match_end]);
    snippet.push_str("**");
    snippet.push_str(content[match_end..end].trim_end());
    if end < content.len() {
        snippet.push('…');
    }
    snippet.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_heading_line(content: &str, offset: usize) -> bool {
    let line_start = content[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    content[line_start..].trim_start().starts_with('#')
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RootConfig;

    fn fixture_index(label: &str, files: &[(&str, &str)]) -> Index {
        let root = std::env::temp_dir().join(format!("kbview-search-{label}"));
        let _ = std::fs::remove_dir_all(&root);
        for (path, body) in files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, body).unwrap();
        }
        Index::build(&RootConfig {
            id: "t".into(),
            name: "t".into(),
            path: root,
            index_names: vec!["index.md".into()],
            wikilinks: Some(true),
            folder_notes: false,
            read_only: false,
        })
    }

    #[test]
    fn ranks_a_title_match_above_a_body_mention() {
        let index = fixture_index(
            "ranking",
            &[
                ("Rust.md", "# Rust\nA language.\n"),
                ("Other.md", "# Other\nI once used rust here.\n"),
            ],
        );
        let hits = search(&index, "rust", 10);
        assert_eq!(hits[0].path, "Rust.md");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn finds_matches_in_the_body_and_marks_them_in_the_snippet() {
        let index = fixture_index("snippet", &[("a.md", "# A\nThe quick brown fox jumps.\n")]);
        let hits = search(&index, "brown", 10);
        assert_eq!(hits.len(), 1);
        assert!(
            hits[0].snippet.contains("**brown**"),
            "got {:?}",
            hits[0].snippet
        );
    }

    #[test]
    fn matching_is_case_insensitive() {
        let index = fixture_index("case", &[("a.md", "# Title\nMixedCase Word\n")]);
        assert_eq!(search(&index, "mixedcase", 10).len(), 1);
        assert_eq!(search(&index, "MIXEDCASE", 10).len(), 1);
    }

    #[test]
    fn an_empty_query_returns_nothing() {
        let index = fixture_index("empty", &[("a.md", "# A\ncontent\n")]);
        assert!(search(&index, "", 10).is_empty());
        assert!(search(&index, "   ", 10).is_empty());
    }

    #[test]
    fn respects_the_limit() {
        const MATCHING_DOCUMENTS: usize = 20;
        const HIT_LIMIT: usize = 5;

        let mut files: Vec<(String, String)> = Vec::new();
        for ordinal in 0..MATCHING_DOCUMENTS {
            files.push((format!("n{ordinal}.md"), "shared keyword\n".to_string()));
        }
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(name, body)| (name.as_str(), body.as_str()))
            .collect();
        let index = fixture_index("limit", &refs);
        assert_eq!(search(&index, "keyword", HIT_LIMIT).len(), HIT_LIMIT);
    }

    #[test]
    fn a_snippet_shows_prose_rather_than_the_yaml_header() {
        let index = fixture_index(
            "snippetfm",
            &[(
                "a.md",
                "---\ntype: guide\ntags: [vinyl]\n---\n# Vinyl\nStore records upright.\n",
            )],
        );
        let hits = search(&index, "upright", 10);
        assert_eq!(hits.len(), 1);
        assert!(
            !hits[0].snippet.starts_with("---"),
            "got {:?}",
            hits[0].snippet
        );
        assert!(hits[0].snippet.contains("**upright**"));
    }

    #[test]
    fn frontmatter_only_matches_still_surface_through_title_and_tags() {
        let index = fixture_index(
            "fmonly",
            &[("a.md", "---\ntags: [kankyo]\n---\n# Ambient\nBody text.\n")],
        );
        assert_eq!(
            search(&index, "kankyo", 10).len(),
            1,
            "tag scoring must still find it"
        );
    }

    #[test]
    fn tag_matches_are_found() {
        let index = fixture_index(
            "tags",
            &[("a.md", "---\ntags: [architecture]\n---\nbody\n")],
        );
        assert_eq!(search(&index, "architecture", 10).len(), 1);
    }

    #[test]
    fn snippets_never_split_a_multibyte_character() {
        const PADDING_REPEATS: usize = 80;

        let long_cjk = "汉字".repeat(PADDING_REPEATS);
        let body = format!("# T\n{long_cjk}needle{long_cjk}\n");
        let index = fixture_index("utf8", &[("a.md", body.as_str())]);
        let hits = search(&index, "needle", 10);
        assert_eq!(hits.len(), 1, "must find the match inside CJK text");
        assert!(hits[0].snippet.contains("**needle**"));
    }

    /// The regression this guards: lowercasing is not length-preserving, so offsets taken
    /// from a lowercased copy do not address the original string.
    #[test]
    fn matching_survives_characters_whose_lowercase_has_a_different_byte_length() {
        for tricky in [
            "\u{1E9E}needle here",
            "\u{130} needle",
            "STRASSE \u{1E9E} needle",
        ] {
            let index = fixture_index("lowercase", &[("a.md", tricky)]);
            let hits = search(&index, "needle", 10);
            assert_eq!(hits.len(), 1, "should match inside {tricky:?}");
            assert!(
                hits[0].snippet.contains("**needle**"),
                "got {:?}",
                hits[0].snippet
            );
        }
    }

    #[test]
    fn a_match_at_the_very_end_of_a_document_does_not_overrun() {
        let index = fixture_index("tail", &[("a.md", "trailing needle")]);
        assert_eq!(search(&index, "needle", 10).len(), 1);
    }

    #[test]
    fn results_are_stable_across_identical_queries() {
        let index = fixture_index(
            "stable",
            &[("a.md", "same\n"), ("b.md", "same\n"), ("c.md", "same\n")],
        );
        let first: Vec<String> = search(&index, "same", 10)
            .into_iter()
            .map(|h| h.path)
            .collect();
        let second: Vec<String> = search(&index, "same", 10)
            .into_iter()
            .map(|h| h.path)
            .collect();
        assert_eq!(first, second);
    }
}
