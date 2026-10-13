//! Full-text search by linear scan.
//!
//! At this corpus size (~1 MB of text) scanning every document costs well under a
//! millisecond, so there is no inverted index to build, invalidate, or keep consistent
//! with the filesystem. If the corpus grew by orders of magnitude this is the one module
//! that would be replaced, and nothing else would need to change.
//!
//! A query is a set of terms, not a phrase: a document matches when every term appears
//! somewhere in it — title, file name, path, tags or body — in any order. People type
//! the words they remember of a note, rarely in the order its title has them. A word
//! typed with `-`, `_` or `/` inside it (`m-team`, `2026-10-14`) is one term whose parts
//! must stay adjacent, while the same name typed with spaces is several terms. The whole
//! query still earns the old phrase scores on top, in the names and in the prose, so an
//! exact title or a verbatim sentence keeps ranking first.
//!
//! A term of one or two letters or digits matches only as a whole word: as a substring,
//! `m` or `to` is inside nearly every note.
//!
//! A term found nowhere may still match a word of the note's name — title, file name,
//! folders, tags — one typo away (`moviepilt`, `cheatshet`). Only names are read that
//! loosely: prose is long enough that some word is always one edit from a short term.
//! Terms with digits are never corrected, since one digit off is a different date or id.
//!
//! Scoring, snippets and their highlights all fold text through [`fold_chars`], so a
//! hit and the words marked in its snippet cannot disagree.

use std::collections::HashSet;
use std::path::Path;

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

/// Terms this short, made of ASCII letters or digits, match only as whole words.
const MAX_SHORT_TERM_CHARS: usize = 2;
/// Terms read from one query; the rest are ignored. Every term costs a scan of every
/// document, and nobody types more than a handful.
const MAX_QUERY_TERMS: usize = 32;

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
    // Snippets are built only for the hits returned: mapping a match back into the
    // original text is the slow part of a search.
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

/// Which characters fold to the space between two words.
#[derive(Clone, Copy)]
enum Gaps {
    /// Whitespace and the word separators: how every query made of words is read.
    SpacesAndSeparators,
    /// Whitespace alone: a query made only of separators (`-`, `/`) looks for them.
    SpacesOnly,
}

impl Gaps {
    /// Newlines are not gaps: they keep a heading on its own line, where the heading
    /// bonus looks for it.
    fn contains(self, character: char) -> bool {
        if character == '\n' {
            return false;
        }
        character.is_whitespace()
            || (matches!(self, Self::SpacesAndSeparators) && WORD_SEPARATORS.contains(&character))
    }
}

/// Every character of `text` as matching sees it, with the byte offset in `text` it
/// came from: lowercased one character at a time, final sigma read as sigma, and each
/// run of gap characters read as one space.
///
/// Lowercasing per character, never with `str::to_lowercase`, is what keeps scoring and
/// snippets in agreement: the string method lowercases a final `Σ` to `ς` by context,
/// which no per-character match can reproduce.
fn fold_chars(text: &str, gaps: Gaps, mut emit: impl FnMut(char, usize)) {
    let mut in_gap = false;
    for (offset, character) in text.char_indices() {
        if gaps.contains(character) {
            if !in_gap {
                emit(' ', offset);
            }
            in_gap = true;
            continue;
        }
        in_gap = false;
        // Most of a vault is ASCII, and its lowercase is one table-free character.
        if character.is_ascii() {
            emit(character.to_ascii_lowercase(), offset);
            continue;
        }
        for lower in character.to_lowercase() {
            emit(if lower == 'ς' { 'σ' } else { lower }, offset);
        }
    }
}

fn fold(text: &str, gaps: Gaps) -> String {
    let mut folded = String::with_capacity(text.len());
    fold_chars(text, gaps, |character, _| folded.push(character));
    folded
}

/// A name as it is compared: folded, without a leading or trailing gap.
fn normalize(text: &str, gaps: Gaps) -> String {
    fold(text, gaps).trim().to_string()
}

/// Folded text that remembers, for each of its bytes, the byte of the original it came
/// from, so a match found in it can be cut out of the original.
struct FoldedText {
    text: String,
    sources: Vec<usize>,
}

impl FoldedText {
    fn of(original: &str, gaps: Gaps) -> Self {
        let mut text = String::with_capacity(original.len());
        let mut sources = Vec::with_capacity(original.len());
        fold_chars(original, gaps, |character, offset| {
            text.push(character);
            sources.resize(text.len(), offset);
        });
        Self { text, sources }
    }

    /// The bytes of `original` that the folded bytes `start..end` came from, widened to
    /// whole characters: one original character can fold to several.
    fn source_range(&self, original: &str, start: usize, end: usize) -> (usize, usize) {
        let last = self.sources[end - 1];
        let last_len = original[last..].chars().next().map_or(0, char::len_utf8);
        (self.sources[start], last + last_len)
    }
}

/// One thing the query looks for, already folded.
struct Term {
    text: String,
    /// Whether the term must start, and end, at a word boundary: true where its first
    /// or last word is short enough to be inside nearly any word.
    bounded_start: bool,
    bounded_end: bool,
}

impl Term {
    fn new(text: String) -> Self {
        let first = text.split(' ').next().unwrap_or_default();
        let last = text.rsplit(' ').next().unwrap_or_default();
        Self {
            bounded_start: is_short_word(first),
            bounded_end: is_short_word(last),
            text,
        }
    }

    /// Byte offsets in `folded` where this term occurs, as a whole word where it must be.
    fn occurrences<'a>(&'a self, folded: &'a str) -> impl Iterator<Item = usize> + 'a {
        folded
            .match_indices(self.text.as_str())
            .map(|(offset, _)| offset)
            .filter(move |&offset| self.is_bounded_at(folded, offset))
    }

    fn occurs_in(&self, folded: &str) -> bool {
        self.occurrences(folded).next().is_some()
    }

    fn is_bounded_at(&self, folded: &str, offset: usize) -> bool {
        let end = offset + self.text.len();
        let opens = !self.bounded_start
            || !folded[..offset]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
        let closes = !self.bounded_end
            || !folded[end..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric);
        opens && closes
    }

    /// A term a typo can be forgiven in: long enough, one word, and no digits.
    fn is_correctable(&self) -> bool {
        self.text.chars().count() >= MIN_TYPO_TERM_CHARS
            && !self
                .text
                .chars()
                .any(|character| character == ' ' || character.is_numeric())
    }
}

/// A word of one or two ASCII letters or digits. CJK words are not short in this
/// sense: two characters are a whole word there.
fn is_short_word(word: &str) -> bool {
    word.chars().count() <= MAX_SHORT_TERM_CHARS && word.chars().all(|c| c.is_ascii_alphanumeric())
}

struct Query {
    /// The whole query, folded, its words joined by one space.
    phrase: Term,
    /// The whole query with its separators kept, as a sentence in the prose would hold
    /// it: folded the other way, every link to `jazz-kissa/tokyo-…` reads as the
    /// phrase `jazz kissa tokyo`.
    verbatim: Term,
    /// The distinct terms, longest first so the most specific one wins a tie.
    terms: Vec<Term>,
    gaps: Gaps,
}

impl Query {
    fn parse(raw: &str) -> Option<Self> {
        let gaps = if normalize(raw, Gaps::SpacesAndSeparators).is_empty() {
            Gaps::SpacesOnly
        } else {
            Gaps::SpacesAndSeparators
        };
        let mut seen = HashSet::new();
        let mut terms: Vec<Term> = raw
            .split_whitespace()
            .map(|token| normalize(token, gaps))
            .filter(|token| !token.is_empty() && seen.insert(token.clone()))
            .take(MAX_QUERY_TERMS)
            .map(Term::new)
            .collect();
        if terms.is_empty() {
            return None;
        }
        terms.sort_by_key(|term| std::cmp::Reverse(term.text.chars().count()));
        Some(Self {
            phrase: Term::new(normalize(raw, gaps)),
            verbatim: Term::new(normalize(raw, Gaps::SpacesOnly)),
            terms,
            gaps,
        })
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
    let names = Names::of(document, query.gaps);
    let prose = searchable_body(document);
    let body = prose.map(|prose| fold(prose, query.gaps));
    let heading_share = SCORE_HEADING_MATCH / query.terms.len() as i32;

    let mut score = names.phrase_score(&query.phrase);
    for term in &query.terms {
        let exact = names.term_score(|name| term.occurs_in(name))
            + body
                .as_deref()
                .map_or(0, |body| body_term_score(body, term, heading_share));
        score += if exact > 0 {
            exact
        } else {
            names.near_miss_score(term)?
        };
    }
    if let Some(prose) = prose {
        score += body_phrase_score(prose, query);
    }
    Some(score)
}

/// What one term earns in the prose: its hits, capped, and a share of the heading bonus
/// when the first of them opens a heading.
fn body_term_score(body: &str, term: &Term, heading_share: i32) -> i32 {
    let mut occurrences = term.occurrences(body);
    let Some(first) = occurrences.next() else {
        return 0;
    };
    let hits = 1 + occurrences.take(MAX_BODY_HITS_COUNTED - 1).count();
    let mut score = SCORE_PER_BODY_HIT * hits as i32;
    if is_heading_line(body, first) {
        score += heading_share;
    }
    score
}

/// What the prose earns for holding the query verbatim. It outweighs every term found
/// apart at its cap, heading shares included, so the sentence a person remembers word
/// for word ranks above a note that merely uses the same words often. Only a document
/// that already matched every term gets here, so folding its prose a second way is
/// paid for by few.
fn body_phrase_score(prose: &str, query: &Query) -> i32 {
    if query.terms.len() < 2 {
        return 0;
    }
    let body = fold(prose, Gaps::SpacesOnly);
    let Some(first) = query.verbatim.occurrences(&body).next() else {
        return 0;
    };
    let scattered_cap =
        query.terms.len() as i32 * SCORE_PER_BODY_HIT * MAX_BODY_HITS_COUNTED as i32;
    let mut score = scattered_cap + SCORE_HEADING_MATCH;
    if is_heading_line(&body, first) {
        score += SCORE_HEADING_MATCH;
    }
    score
}

/// What a document is called, folded once per query: its title, file name, path and
/// tags.
struct Names {
    title: String,
    stem: String,
    path: String,
    tags: Vec<String>,
}

impl Names {
    /// The file name without its extension is a second title, and the only readable
    /// one when the heading is in another language than the query.
    fn of(document: &Document, gaps: Gaps) -> Self {
        let stem = Path::new(&document.path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        Self {
            title: normalize(&document.title, gaps),
            stem: normalize(stem, gaps),
            path: normalize(&document.path, gaps),
            tags: document
                .tags
                .iter()
                .map(|tag| normalize(tag, gaps))
                .collect(),
        }
    }

    /// The whole query read as one phrase, against what the document is called.
    fn phrase_score(&self, phrase: &Term) -> i32 {
        let as_name = |name: &str| {
            if name == phrase.text {
                SCORE_TITLE_EXACT
            } else if phrase.occurrences(name).next() == Some(0) {
                SCORE_TITLE_PREFIX
            } else if phrase.occurs_in(name) {
                SCORE_TITLE_CONTAINS
            } else {
                0
            }
        };
        let mut score = as_name(&self.title).max(as_name(&self.stem));
        if phrase.occurs_in(&self.path) {
            score += SCORE_PATH_CONTAINS;
        }
        if self.tags.iter().any(|tag| phrase.occurs_in(tag)) {
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
    fn near_miss_score(&self, term: &Term) -> Option<i32> {
        if !term.is_correctable() {
            return None;
        }
        let typed: Vec<char> = term.text.chars().collect();
        let score = self.term_score(|name| has_word_near(name, &typed));
        (score > 0).then_some(score / NEAR_MISS_DIVISOR)
    }
}

/// Whether some word of `name` is within a typo of `typed`.
fn has_word_near(name: &str, typed: &[char]) -> bool {
    name.split(|c: char| !c.is_alphanumeric()).any(|word| {
        word.chars().count().abs_diff(typed.len()) <= MAX_TYPO_EDITS
            && edit_distance(typed, &word.chars().collect::<Vec<_>>()) <= MAX_TYPO_EDITS
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

/// Context around the place the query's terms gather, with every term inside it
/// marked; the title when no term is in the body.
fn snippet_for(document: &Document, query: &Query) -> String {
    let Some(content) = searchable_body(document) else {
        return document.title.clone();
    };
    let folded = FoldedText::of(content, query.gaps);
    let Some((start, end)) = snippet_focus(&folded.text, query) else {
        return document.title.clone();
    };
    let (start, end) = folded.source_range(content, start, end);
    build_snippet(content, start, end, query)
}

/// The folded range a snippet centres on: the verbatim query where the prose has it,
/// otherwise the stretch of one context's length holding the most distinct terms.
fn snippet_focus(folded: &str, query: &Query) -> Option<(usize, usize)> {
    if let Some(start) = query.phrase.occurrences(folded).next() {
        return Some((start, start + query.phrase.text.len()));
    }
    let mut found: Vec<(usize, usize, usize)> = Vec::new();
    for (index, term) in query.terms.iter().enumerate() {
        found.extend(
            term.occurrences(folded)
                .take(MAX_BODY_HITS_COUNTED)
                .map(|start| (start, start + term.text.len(), index)),
        );
    }
    found.sort_unstable();
    let (first, last) = densest_stretch(&found, query.terms.len())?;
    let end = found[first..=last].iter().map(|&(_, end, _)| end).max()?;
    Some((found[first].0, end))
}

/// Indices of the first and last of the `found` matches, sorted by start, that open a
/// stretch of `SNIPPET_CONTEXT` bytes holding the most distinct terms; the earliest
/// such stretch wins a tie.
fn densest_stretch(found: &[(usize, usize, usize)], term_count: usize) -> Option<(usize, usize)> {
    let mut counts = vec![0usize; term_count];
    let mut distinct = 0;
    let mut best: Option<(usize, usize, usize)> = None;
    let mut next = 0;
    for (first, &(start, _, term)) in found.iter().enumerate() {
        while next < found.len() && found[next].0 < start + SNIPPET_CONTEXT {
            counts[found[next].2] += 1;
            distinct += usize::from(counts[found[next].2] == 1);
            next += 1;
        }
        if best.is_none_or(|(most, _, _)| distinct > most) {
            best = Some((distinct, first, next - 1));
        }
        counts[term] -= 1;
        distinct -= usize::from(counts[term] == 0);
    }
    best.map(|(_, first, last)| (first, last))
}

/// Context around the bytes `focus_start..focus_end` of `content`, with each term
/// inside it wrapped in `**` for the UI to style.
fn build_snippet(content: &str, focus_start: usize, focus_end: usize, query: &Query) -> String {
    let start = floor_char_boundary(content, focus_start.saturating_sub(SNIPPET_CONTEXT));
    let end = ceil_char_boundary(content, focus_end.saturating_add(SNIPPET_CONTEXT));
    // The client pairs `**` markers in order, so one left over from the note's own
    // emphasis — cut in half at the window's edge, say — would invert every highlight
    // after it. Only the markers added below may remain.
    let window = content[start..end].replace('*', "");

    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.push_str(&mark_terms(window.trim(), query));
    if end < content.len() {
        snippet.push('…');
    }
    snippet.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `window` with every occurrence of a term wrapped in `**`, found exactly as scoring
/// finds it; where two occurrences overlap or touch they are marked as one.
fn mark_terms(window: &str, query: &Query) -> String {
    let folded = FoldedText::of(window, query.gaps);
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for term in &query.terms {
        ranges.extend(
            term.occurrences(&folded.text)
                .map(|start| folded.source_range(window, start, start + term.text.len())),
        );
    }
    ranges.sort_unstable();
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
        assert_eq!(search(&index, "pilot", 10).len(), 1);
        assert!(
            search(&index, "pliot", 10).is_empty(),
            "a five-letter typo is corrected in names only, never in prose"
        );
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

    fn paths(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|hit| hit.path.as_str()).collect()
    }

    /// The `**`-delimited spans of a snippet, paired the way the client pairs them.
    fn highlighted(snippet: &str) -> Vec<&str> {
        snippet.split("**").skip(1).step_by(2).collect()
    }

    #[test]
    fn a_verbatim_phrase_in_the_prose_outranks_the_same_words_scattered() {
        let scattered =
            "quick quick quick quick quick. brown brown brown brown brown. fox fox fox fox fox.\n";
        let index = fixture_index(
            "bodyphrase",
            &[("a.md", "the quick brown fox\n"), ("b.md", scattered)],
        );
        assert_eq!(
            paths(&search(&index, "quick brown fox", 10)),
            ["a.md", "b.md"]
        );
    }

    #[test]
    fn a_link_to_a_note_is_not_its_words_written_verbatim() {
        let index = fixture_index(
            "linkphrase",
            &[
                ("jazz-kissa/tokyo.md", "# Tokyo\nCoffee and records.\n"),
                (
                    "jazz-kissa/overview.md",
                    "# Overview\nSee [[jazz-kissa/tokyo]] and [[jazz-kissa/osaka]].\n",
                ),
            ],
        );
        assert_eq!(
            paths(&search(&index, "jazz kissa tokyo", 10))[0],
            "jazz-kissa/tokyo.md"
        );
    }

    #[test]
    fn an_exact_title_outranks_scattered_matches() {
        let index = fixture_index(
            "titlephrase",
            &[
                ("brown-fox.md", "# Brown Fox\nNotes.\n"),
                (
                    "other.md",
                    "# Other\nA fox, brown and quick, brown as a fox.\n",
                ),
            ],
        );
        assert_eq!(
            paths(&search(&index, "brown fox", 10)),
            ["brown-fox.md", "other.md"]
        );
    }

    #[test]
    fn a_word_typed_with_a_hyphen_keeps_its_parts_together() {
        let index = fixture_index(
            "mteam",
            &[
                ("m-team-account.md", "# M-Team Account\nRatio rules.\n"),
                ("notes.md", "# Notes\nThe team meets monthly.\n"),
                ("other.md", "# Other\nmom and a team\n"),
            ],
        );
        assert_eq!(paths(&search(&index, "m-team", 10)), ["m-team-account.md"]);
        assert_eq!(
            paths(&search(&index, "m team account", 10)),
            ["m-team-account.md"]
        );
    }

    #[test]
    fn a_term_of_one_or_two_letters_matches_only_a_whole_word() {
        let index = fixture_index(
            "short",
            &[
                ("alphabet.md", "# Alphabet\nabc\n"),
                ("plans.md", "# Plans\nPlan a, or plan b.\n"),
            ],
        );
        assert_eq!(paths(&search(&index, "a b", 10)), ["plans.md"]);
    }

    #[test]
    fn a_date_matches_only_as_written() {
        let index = fixture_index(
            "date",
            &[
                ("log.md", "# Log\nShipped on 2026-10-14.\n"),
                ("other.md", "# Other\nIn 2026, 10 people came on day 14.\n"),
            ],
        );
        assert_eq!(paths(&search(&index, "2026-10-14", 10)), ["log.md"]);
    }

    #[test]
    fn a_term_with_digits_is_never_corrected() {
        let index = fixture_index("digits", &[("20261013.md", "# 2026-10-13\nA day.\n")]);
        assert!(search(&index, "20261014", 10).is_empty());
    }

    #[test]
    fn a_query_of_separators_alone_is_searched_literally() {
        let index = fixture_index(
            "symbols",
            &[
                ("a.md", "# A\nleft - right\n"),
                ("b.md", "# B\nno dash here\n"),
            ],
        );
        assert_eq!(paths(&search(&index, "-", 10)), ["a.md"]);
        assert!(search(&index, "/", 10).is_empty());
    }

    #[test]
    fn a_snippet_cut_inside_the_notes_own_emphasis_still_pairs_its_markers() {
        let lead = "x".repeat(SNIPPET_CONTEXT);
        let body = format!("# A\n**{lead} bold text** the quick brown fox **fox** jumps\n");
        let index = fixture_index("bold", &[("a.md", body.as_str())]);
        let snippet = &search(&index, "quick fox", 10)[0].snippet;
        assert_eq!(
            highlighted(snippet),
            ["quick", "fox", "fox"],
            "got {snippet:?}"
        );
    }

    #[test]
    fn a_snippet_shows_where_the_terms_meet() {
        let filler = "Unrelated prose. ".repeat(SNIPPET_CONTEXT);
        let body = format!("# A\nThe insurance file.\n{filler}\nA quote for insurance arrived.\n");
        let index = fixture_index("cooccur", &[("a.md", body.as_str())]);
        let snippet = &search(&index, "quote insurance", 10)[0].snippet;
        assert_eq!(
            highlighted(snippet),
            ["quote", "insurance"],
            "got {snippet:?}"
        );
    }

    #[test]
    fn a_snippet_puts_no_space_beside_its_ellipsis() {
        let padding = "ab ".repeat(SNIPPET_CONTEXT);
        let body = format!("# A\n{padding}needle {padding}\n");
        let index = fixture_index("trim", &[("a.md", body.as_str())]);
        let snippet = &search(&index, "needle", 10)[0].snippet;
        assert!(
            !snippet.starts_with("… ") && !snippet.ends_with(" …"),
            "got {snippet:?}"
        );
    }

    #[test]
    fn a_hit_and_its_snippet_fold_case_the_same_way() {
        let index = fixture_index("sigma", &[("a.md", "# Epic\nΟΔΥΣΣΕΥΣ returns.\n")]);
        let hits = search(&index, "οδυσσευς", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            highlighted(&hits[0].snippet),
            ["ΟΔΥΣΣΕΥΣ"],
            "got {:?}",
            hits[0].snippet
        );
    }

    #[test]
    fn a_query_of_thousands_of_words_is_cut_short() {
        let index = fixture_index("long", &[("a.md", "# A\nword0 word1\n")]);
        const WORDS_TYPED: usize = 5000;
        let numbers = 0..WORDS_TYPED;
        let query: Vec<String> = numbers.map(|number| format!("word{number}")).collect();
        assert!(search(&index, &query.join(" "), 10).is_empty());
    }
}
