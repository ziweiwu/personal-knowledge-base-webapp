//! Full-text search by linear scan.
//!
//! At this corpus size (~1 MB of text) scanning every document costs well under a
//! millisecond, so there is no inverted index to build, invalidate, or keep consistent
//! with the filesystem. If the corpus grew by orders of magnitude this is the one module
//! that would be replaced, and nothing else would need to change.
//!
//! A query is a set of terms, not a phrase: a document matches when every term appears
//! somewhere in it — title, file name, path, tags or body — in any order. People type
//! the words they remember of a note, rarely in the order its title has them, and a
//! hyphenated name (`long-running`, `m-team`) is typed with spaces. The whole query
//! still earns the old phrase scores on top, so an exact title keeps ranking first.
//!
//! A term found nowhere may still match a word of the note's name — title, file name,
//! folders, tags — one typo away (`moviepilt`, `cheatshet`). Only names are read that
//! loosely: prose is long enough that some word is always one edit from a short term.

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

/// What one term earns where it is found. A term in the title says more about what the
/// note is than the same term in its folder, its tags or its prose.
const SCORE_TERM_IN_TITLE: i32 = 100;
const SCORE_TERM_IN_PATH: i32 = 60;
const SCORE_TERM_IN_TAG: i32 = 80;

/// A term matched only through a typo earns this share of what the same word spelled
/// right would have earned where it was found.
const NEAR_MISS_DIVISOR: i32 = 2;
/// Shorter terms are not corrected: one edit from a four-letter word is too many words.
const MIN_TYPO_TERM_CHARS: usize = 5;
/// Edits forgiven between a term and a word of the name.
const MAX_TYPO_EDITS: usize = 1;

/// Characters a person types as a space between the words of a name.
const WORD_SEPARATORS: [char; 3] = ['-', '_', '/'];

pub fn search(index: &Index, query: &str, limit: usize) -> Vec<SearchHit> {
    let Some(query) = Query::parse(query) else {
        return Vec::new();
    };

    let mut scored: Vec<(i32, &Document)> = index
        .documents
        .values()
        .filter_map(|document| score_document(document, &query).map(|score| (score, document)))
        .collect();

    // Sort by score, then path, so equal scores do not reorder between identical queries.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    scored.truncate(limit);
    // Snippets are built only for the hits returned: finding a match in the original
    // text, rather than a lowercased copy, is the slow part of a search.
    scored
        .into_iter()
        .map(|(score, document)| SearchHit {
            path: document.path.clone(),
            title: document.title.clone(),
            kind: document.kind,
            snippet: snippet_for(document, &query),
            score,
        })
        .collect()
}

struct Query {
    /// The whole query, lowercased, with word separators and runs of space collapsed to
    /// one space.
    phrase: String,
    /// The distinct terms, longest first so the most specific one anchors a snippet.
    terms: Vec<String>,
}

impl Query {
    fn parse(raw: &str) -> Option<Self> {
        let phrase = normalize(raw);
        if phrase.is_empty() {
            return None;
        }
        let mut terms: Vec<String> = Vec::new();
        for term in phrase.split(' ') {
            if !terms.iter().any(|known| known == term) {
                terms.push(term.to_string());
            }
        }
        terms.sort_by_key(|term| std::cmp::Reverse(term.chars().count()));
        Some(Self { phrase, terms })
    }
}

/// Lowercased, with word separators read as spaces and whitespace collapsed, so that
/// `Long-Running Agents` and `long running agents` compare equal.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| c.is_whitespace() || WORD_SEPARATORS.contains(&c))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A document's file name without its extension: a second title, and the only
/// readable one when the heading is in another language than the query.
fn file_stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}

/// The text a body match is looked for in: the prose, not the YAML header.
fn searchable_body(document: &Document) -> Option<&str> {
    if !document.kind.is_searchable() {
        return None;
    }
    let stored = document.content.as_deref()?;
    // Frontmatter is already searched through the title and tag scores, and including
    // it here made every snippet open with "--- type: guide tags: …" instead of the
    // sentence the reader was looking for.
    Some(match document.kind {
        DocumentKind::Markdown => crate::frontmatter::split(stored).1,
        _ => stored,
    })
}

/// The document's score for `query`, or `None` when some term appears nowhere in it.
fn score_document(document: &Document, query: &Query) -> Option<i32> {
    let names = Names::of(document);
    let body = searchable_body(document).map(str::to_lowercase);
    let heading_share = SCORE_HEADING_MATCH / query.terms.len() as i32;

    let mut score = names.phrase_score(&query.phrase);
    for term in &query.terms {
        let exact = names.term_score(|name| name.contains(term.as_str()))
            + body
                .as_deref()
                .map_or(0, |body| body_term_score(body, term, heading_share));
        score += if exact > 0 {
            exact
        } else {
            names.near_miss_score(term)?
        };
    }
    Some(score)
}

/// What one term earns in the prose: its hits, capped, and a share of the heading bonus
/// when the first of them opens a heading.
fn body_term_score(body: &str, term: &str, heading_share: i32) -> i32 {
    let hits = body.matches(term).count();
    if hits == 0 {
        return 0;
    }
    let mut score = SCORE_PER_BODY_HIT * hits.min(MAX_BODY_HITS_COUNTED) as i32;
    if body
        .find(term)
        .is_some_and(|offset| is_heading_line(body, offset))
    {
        score += heading_share;
    }
    score
}

/// What a document is called, normalized once per query: its title, file name, path
/// and tags.
struct Names {
    title: String,
    stem: String,
    path: String,
    tags: Vec<String>,
}

impl Names {
    fn of(document: &Document) -> Self {
        Self {
            title: normalize(&document.title),
            stem: normalize(file_stem(&document.path)),
            path: normalize(&document.path),
            tags: document.tags.iter().map(|tag| normalize(tag)).collect(),
        }
    }

    /// The whole query read as one phrase, against what the document is called.
    fn phrase_score(&self, phrase: &str) -> i32 {
        let as_name = |name: &str| {
            if name == phrase {
                SCORE_TITLE_EXACT
            } else if name.starts_with(phrase) {
                SCORE_TITLE_PREFIX
            } else if name.contains(phrase) {
                SCORE_TITLE_CONTAINS
            } else {
                0
            }
        };
        let mut score = as_name(&self.title).max(as_name(&self.stem));
        if self.path.contains(phrase) {
            score += SCORE_PATH_CONTAINS;
        }
        if self.tags.iter().any(|tag| tag.contains(phrase)) {
            score += SCORE_TAG_MATCH;
        }
        score
    }

    /// What one term earns in the names that `holds` it, weighted by which name it is.
    fn term_score(&self, holds: impl Fn(&str) -> bool) -> i32 {
        let mut score = 0;
        if holds(&self.title) || holds(&self.stem) {
            score += SCORE_TERM_IN_TITLE;
        }
        if holds(&self.path) {
            score += SCORE_TERM_IN_PATH;
        }
        if self.tags.iter().any(|tag| holds(tag)) {
            score += SCORE_TERM_IN_TAG;
        }
        score
    }

    /// What `term` earns as a typo of a word in the names; `None` when no word is near
    /// enough.
    fn near_miss_score(&self, term: &str) -> Option<i32> {
        let typed: Vec<char> = term.chars().collect();
        if typed.len() < MIN_TYPO_TERM_CHARS {
            return None;
        }
        let score = self.term_score(|name| has_word_near(name, &typed));
        (score > 0).then_some(score / NEAR_MISS_DIVISOR)
    }
}

/// Whether some word of `name` is within a typo of `typed`.
fn has_word_near(name: &str, typed: &[char]) -> bool {
    name.split(|c: char| !c.is_alphanumeric()).any(|word| {
        let word: Vec<char> = word.chars().collect();
        word.len().abs_diff(typed.len()) <= MAX_TYPO_EDITS
            && edit_distance(typed, &word) <= MAX_TYPO_EDITS
    })
}

/// Optimal string alignment distance: insertions, deletions, substitutions, and a swap
/// of two neighbours, each one edit — the four ways a word is mistyped.
fn edit_distance(typed: &[char], word: &[char]) -> usize {
    let mut before_previous: Vec<usize> = vec![0; word.len() + 1];
    let mut previous: Vec<usize> = (0..=word.len()).collect();
    let mut current: Vec<usize> = vec![0; word.len() + 1];
    for i in 1..=typed.len() {
        current[0] = i;
        for j in 1..=word.len() {
            let substitution = previous[j - 1] + usize::from(typed[i - 1] != word[j - 1]);
            current[j] = substitution.min(previous[j] + 1).min(current[j - 1] + 1);
            if i > 1 && j > 1 && typed[i - 1] == word[j - 2] && typed[i - 2] == word[j - 1] {
                current[j] = current[j].min(before_previous[j - 2] + 1);
            }
        }
        std::mem::swap(&mut before_previous, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    previous[word.len()]
}

/// Context around the first match of the most specific term, with every term inside it
/// marked; the title when no term is in the body.
fn snippet_for(document: &Document, query: &Query) -> String {
    let Some(content) = searchable_body(document) else {
        return document.title.clone();
    };
    let anchor = std::iter::once(query.phrase.as_str())
        .chain(query.terms.iter().map(String::as_str))
        .find_map(|needle| find_ignoring_case(content, needle).map(|start| (start, needle)));
    match anchor {
        Some((start, needle)) => build_snippet(content, start, needle, &query.terms),
        None => document.title.clone(),
    }
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

/// The byte length in `haystack` of a case-insensitive match of `needle` at `offset`.
fn match_len_at(haystack: &str, offset: usize, needle: &str) -> Option<usize> {
    let mut wanted = needle.chars().peekable();
    let mut consumed = 0;
    for source in haystack[offset..].chars() {
        if wanted.peek().is_none() {
            break;
        }
        if !source.to_lowercase().all(|got| wanted.next() == Some(got)) {
            return None;
        }
        consumed += source.len_utf8();
    }
    wanted.peek().is_none().then_some(consumed)
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

/// Context around a match, with each term inside it wrapped in `**` for the UI to style.
fn build_snippet(content: &str, match_start: usize, needle: &str, terms: &[String]) -> String {
    let match_len = match_len_at(content, match_start, needle)
        .unwrap_or(needle.len())
        .min(content.len().saturating_sub(match_start));
    let start = floor_char_boundary(content, match_start.saturating_sub(SNIPPET_CONTEXT));
    let end = ceil_char_boundary(
        content,
        (match_start + match_len + SNIPPET_CONTEXT).min(content.len()),
    );

    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.push_str(&mark_terms(&content[start..end], terms));
    if end < content.len() {
        snippet.push('…');
    }
    snippet.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `window` with every case-insensitive occurrence of a term wrapped in `**`; where two
/// occurrences overlap they are marked as one.
fn mark_terms(window: &str, terms: &[String]) -> String {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (offset, _) in window.char_indices() {
        for term in terms {
            if let Some(len) = match_len_at(window, offset, term) {
                ranges.push((offset, offset + len));
            }
        }
    }
    ranges.sort();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }

    let mut marked = String::with_capacity(window.len());
    let mut cursor = 0;
    for (start, end) in merged {
        marked.push_str(&window[cursor..start]);
        marked.push_str("**");
        marked.push_str(&window[start..end]);
        marked.push_str("**");
        cursor = end;
    }
    marked.push_str(&window[cursor..]);
    marked
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

    fn fixture_index(label: &str, files: &[(&str, &str)]) -> Index {
        Index::build(&crate::test_support::fixture_root(label, files, Some(true)))
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

    #[test]
    fn every_term_must_match_but_in_any_order_and_any_field() {
        let index = fixture_index(
            "terms",
            &[
                (
                    "condo/insurance-quote.md",
                    "# Coverage quote\nWhat the building requires.\n",
                ),
                ("condo/other.md", "# Other\nNothing about cover here.\n"),
            ],
        );
        let hits = search(&index, "quote condo", 10);
        assert_eq!(hits.len(), 1, "got {hits:?}");
        assert_eq!(hits[0].path, "condo/insurance-quote.md");
        assert!(search(&index, "condo xylophone", 10).is_empty());
    }

    #[test]
    fn a_hyphenated_name_is_found_when_typed_with_spaces() {
        let index = fixture_index(
            "hyphen",
            &[
                (
                    "long-running-agents.md",
                    "# Long-Running Agents\nHarnesses.\n",
                ),
                (
                    "running.md",
                    "# Running\nA long note about agents and running.\n",
                ),
            ],
        );
        let hits = search(&index, "long running agents", 10);
        assert_eq!(hits[0].path, "long-running-agents.md", "got {hits:?}");
    }

    #[test]
    fn a_term_one_typo_from_a_word_of_the_name_still_finds_the_note() {
        let index = fixture_index(
            "typo",
            &[("tmux-cheatsheet.md", "# Tmux Cheatsheet\nPrefix keys.\n")],
        );
        assert_eq!(search(&index, "tmux cheatshet", 10).len(), 1);
        assert_eq!(
            search(&index, "cheatsheet tmxu", 10).len(),
            0,
            "four letters are never corrected"
        );
    }

    #[test]
    fn a_typo_in_prose_alone_matches_nothing() {
        let index = fixture_index("typobody", &[("a.md", "# A\nThe pilot flew.\n")]);
        assert!(search(&index, "pilt flew", 10).is_empty());
        assert!(search(&index, "flwe", 10).is_empty());
    }

    #[test]
    fn every_term_inside_the_snippet_is_marked() {
        let index = fixture_index("marks", &[("a.md", "# A\nThe quick brown fox jumps.\n")]);
        let hits = search(&index, "fox brown", 10);
        assert!(
            hits[0].snippet.contains("**brown**"),
            "got {:?}",
            hits[0].snippet
        );
        assert!(
            hits[0].snippet.contains("**fox**"),
            "got {:?}",
            hits[0].snippet
        );
    }
}
