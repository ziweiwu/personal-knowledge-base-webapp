//! Wikilink and relative-link parsing, resolution, and rewriting.
//!
//! Rewriting runs against real files during a rename, so the scanner works in byte
//! ranges taken from the source rather than reconstructing text. Anything it is not
//! certain about, it leaves alone: a missed link is a broken reference the user can
//! see and fix, whereas a wrong rewrite silently corrupts prose.

use std::collections::HashMap;
use std::path::Path;

/// `[[` and `]]`, the brackets around every wikilink target.
const BRACKET_PAIR_LEN: usize = 2;
/// The `!` that turns a wikilink into an embed.
const EMBED_MARKER_LEN: usize = 1;
/// An indent this wide is already a code block, so a fence cannot open there.
const MAX_FENCE_INDENT: usize = 4;
/// The shortest run of backticks or tildes that opens a fence.
const MIN_FENCE_RUN: usize = 3;
/// `######` is the deepest ATX heading; more `#` than that is not a heading at all.
const MAX_HEADING_LEVEL: usize = 6;

/// One `[[wikilink]]` or `![[embed]]` found in a source document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    /// Byte range of the whole link, including brackets and any leading `!`.
    pub start: usize,
    pub end: usize,
    /// Byte range of just the target portion, which is what a rename rewrites.
    pub target_start: usize,
    pub target_end: usize,
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
    pub embed: bool,
}

impl WikiLink {
    /// The text a reader sees when the link renders.
    pub fn display(&self) -> String {
        if let Some(alias) = &self.alias {
            return alias.clone();
        }
        match &self.heading {
            Some(h) if self.target.is_empty() => h.clone(),
            Some(h) => format!("{} > {}", self.target, h),
            None => self.target.clone(),
        }
    }
}

/// Scan `src` for wikilinks, skipping fenced code blocks and inline code spans.
///
/// Links inside code are shown verbatim by Obsidian, so treating them as links would
/// both render wrongly and, worse, let a rename rewrite the inside of a code sample.
pub fn scan_wikilinks(src: &str) -> Vec<WikiLink> {
    let bytes = src.as_bytes();
    let mut links = Vec::new();
    let mut cursor = MarkdownCursor::new();

    while cursor.index < bytes.len() {
        let start = cursor.index;
        if cursor.next_prose_byte(src).is_none() {
            continue;
        }
        match wikilink_at(src, start) {
            Some(link) => {
                cursor.resume_at(link.end);
                links.push(link);
            }
            None => cursor.resume_at(start + 1),
        }
    }
    links
}

/// The wikilink opening at `start`, if one does and it is well formed.
fn wikilink_at(src: &str, start: usize) -> Option<WikiLink> {
    let bytes = src.as_bytes();
    let opens_wikilink =
        |from: usize| bytes.get(from) == Some(&b'[') && bytes.get(from + 1) == Some(&b'[');

    let embed = bytes[start] == b'!' && opens_wikilink(start + EMBED_MARKER_LEN);
    if !embed && !opens_wikilink(start) {
        return None;
    }

    let inner_start = if embed {
        start + EMBED_MARKER_LEN + BRACKET_PAIR_LEN
    } else {
        start + BRACKET_PAIR_LEN
    };
    let inner_end = find_close(bytes, inner_start)?;
    parse_wikilink(
        src,
        &LinkSpan {
            start,
            inner_start,
            inner_end,
            embed,
        },
    )
}

/// The walk every scanner in this module shares: it tracks where lines start and which
/// fenced block is open, and steps over code so that callers only ever see live prose.
struct MarkdownCursor {
    index: usize,
    /// The fence marker byte and its run length, while a fenced block is open.
    open_fence: Option<(u8, usize)>,
    at_line_start: bool,
}

impl MarkdownCursor {
    fn new() -> Self {
        Self {
            index: 0,
            open_fence: None,
            at_line_start: true,
        }
    }

    /// The prose byte at the cursor, or `None` when the cursor instead stepped over a line
    /// break, a fence line, or a run of code. The skipped bytes are those between the
    /// caller's previous index and the new one.
    fn next_prose_byte(&mut self, src: &str) -> Option<u8> {
        let bytes = src.as_bytes();
        if self.at_line_start && self.consume_fence_line(bytes) {
            return None;
        }

        let byte = bytes[self.index];
        if byte == b'\n' {
            self.index += 1;
            self.at_line_start = true;
            return None;
        }
        self.at_line_start = self.at_line_start && (byte == b' ' || byte == b'\t');

        if self.open_fence.is_some() {
            // Step a whole character, not a byte. Callers copy the skipped span out of the
            // `&str`, so advancing into the middle of a multi-byte character makes that
            // slice panic — which a fenced code block containing any non-ASCII text would
            // otherwise do on every render.
            self.index += char_len_at(src, self.index);
            return None;
        }
        if byte == b'`' {
            self.index = skip_inline_code(bytes, self.index);
            return None;
        }
        Some(byte)
    }

    /// Carry on from `index`, which the caller has read up to itself.
    fn resume_at(&mut self, index: usize) {
        self.index = index;
    }

    /// Open or close a fenced block if a fence line begins at the cursor.
    fn consume_fence_line(&mut self, bytes: &[u8]) -> bool {
        let Some((marker, run, line_end)) = fence_at(bytes, self.index) else {
            return false;
        };
        self.open_fence = match self.open_fence {
            Some((open_marker, open_run)) if open_marker == marker && run >= open_run => None,
            Some(existing) => Some(existing),
            None => Some((marker, run)),
        };
        self.index = line_end;
        true
    }

    /// Step over the `#` run that opens an ATX heading, so those markers are not read as a
    /// tag. Reports whether it moved.
    fn skip_heading_markers(&mut self, bytes: &[u8]) -> bool {
        if !self.at_line_start {
            return false;
        }
        let Some(end) = heading_markers_end(bytes, self.index) else {
            return false;
        };
        self.index = end;
        self.at_line_start = false;
        true
    }
}

/// Returns `(marker, run_length, index_after_line)` if a code fence opens or closes here.
fn fence_at(bytes: &[u8], start: usize) -> Option<(u8, usize, usize)> {
    let mut i = start;
    let mut indent = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') && indent < MAX_FENCE_INDENT {
        indent += 1;
        i += 1;
    }
    let marker = *bytes.get(i)?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let run_start = i;
    while i < bytes.len() && bytes[i] == marker {
        i += 1;
    }
    let run = i - run_start;
    if run < MIN_FENCE_RUN {
        return None;
    }
    let line_end = bytes[i..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| i + p + 1)
        .unwrap_or(bytes.len());
    Some((marker, run, line_end))
}

fn skip_inline_code(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && bytes[i] == b'`' {
        i += 1;
    }
    let run = i - start;
    let mut j = i;
    while j < bytes.len() {
        if bytes[j] == b'\n' && bytes.get(j + 1) == Some(&b'\n') {
            return i;
        }
        if bytes[j] == b'`' {
            let close_start = j;
            while j < bytes.len() && bytes[j] == b'`' {
                j += 1;
            }
            if j - close_start == run {
                return j;
            }
            continue;
        }
        j += 1;
    }
    i
}

fn find_close(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\n' {
            return None;
        }
        if bytes[i] == b']' && bytes[i + 1] == b']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Where one `[[...]]` occurrence sits in the source, and whether it is an embed.
struct LinkSpan {
    /// Index of the `!` for an embed, or of the first `[` otherwise.
    start: usize,
    inner_start: usize,
    inner_end: usize,
    embed: bool,
}

fn parse_wikilink(src: &str, span: &LinkSpan) -> Option<WikiLink> {
    let inner = src.get(span.inner_start..span.inner_end)?;
    if inner.contains('[') {
        return None;
    }

    let (before_alias, alias) = match inner.split_once('|') {
        Some((rest, alias)) => (rest, Some(alias.trim().to_string())),
        None => (inner, None),
    };
    let (target, heading) = match before_alias.split_once('#') {
        Some((rest, heading)) => (rest, Some(heading.trim().to_string())),
        None => (before_alias, None),
    };

    let trimmed = target.trim();
    // A link with neither target nor heading is just empty brackets, not a link.
    if trimmed.is_empty() && heading.is_none() {
        return None;
    }

    let leading_ws = target.len() - target.trim_start().len();
    Some(WikiLink {
        start: span.start,
        end: span.inner_end + BRACKET_PAIR_LEN,
        target_start: span.inner_start + leading_ws,
        target_end: span.inner_start + leading_ws + trimmed.len(),
        target: trimmed.to_string(),
        heading,
        alias,
        embed: span.embed,
    })
}

/// Resolves link text to a document path, using Obsidian's matching rules.
pub struct Resolver {
    /// Lowercased path -> canonical path, for exact matches.
    by_path: HashMap<String, String>,
    /// Lowercased basename without extension -> every path that has it.
    by_stem: HashMap<String, Vec<String>>,
}

impl Resolver {
    pub fn new<I, S>(paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut by_path = HashMap::new();
        let mut by_stem: HashMap<String, Vec<String>> = HashMap::new();

        for path in paths {
            let path = path.as_ref().to_string();
            by_path.insert(path.to_lowercase(), path.clone());
            if let Some(stripped) = path.strip_suffix(".md") {
                by_path.insert(stripped.to_lowercase(), path.clone());
            }
            if let Some(stem) = Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
            {
                by_stem.entry(stem).or_default().push(path.clone());
            }
            if let Some(name) = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
            {
                by_stem.entry(name).or_default().push(path);
            }
        }

        for candidates in by_stem.values_mut() {
            candidates.sort();
            candidates.dedup();
        }
        Self { by_path, by_stem }
    }

    /// Resolve wikilink text as Obsidian does: an exact path wins; otherwise match the
    /// basename, preferring a document in the linking note's own folder and then the
    /// shallowest path. Ambiguity resolves deterministically rather than arbitrarily.
    pub fn resolve(&self, from: &str, target: &str) -> Option<String> {
        if target.is_empty() {
            return Some(from.to_string());
        }
        let needle = target.trim_start_matches("./").to_lowercase();

        if let Some(found) = self.by_path.get(&needle) {
            return Some(found.clone());
        }

        // A target containing a separator is a path, so do not fall back to basename
        // matching — that would silently resolve `archive/Note` to an unrelated `Note`.
        if needle.contains('/') {
            return None;
        }

        nearest_candidate(self.by_stem.get(&needle)?, from)
    }

    /// Resolve a relative markdown link such as `./other.md` or `../img.png`.
    pub fn resolve_relative(&self, from: &str, target: &str) -> Option<String> {
        if target.starts_with('/') || target.contains("://") || target.starts_with('#') {
            return None;
        }
        let decoded = target.split(['#', '?']).next().unwrap_or(target);
        if decoded.is_empty() {
            return None;
        }

        let base = Path::new(from).parent().unwrap_or(Path::new(""));
        let mut stack: Vec<String> = base
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();

        for part in decoded.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    stack.pop();
                }
                other => stack.push(other.to_string()),
            }
        }
        let joined = stack.join("/");
        self.by_path.get(&joined.to_lowercase()).cloned()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.by_path.contains_key(&path.to_lowercase())
    }
}

/// Of several documents sharing a basename, the one a reader would mean: a sibling of the
/// linking note first, then the shallowest path. Ties break on the path itself, so the
/// same vault always resolves the same way.
fn nearest_candidate(candidates: &[String], from: &str) -> Option<String> {
    if candidates.len() == 1 {
        return candidates.first().cloned();
    }

    let from_dir = parent_of(from);
    let sibling = candidates.iter().find(|path| parent_of(path) == from_dir);
    if let Some(found) = sibling {
        return Some(found.clone());
    }

    candidates
        .iter()
        .min_by_key(|path| (path.matches('/').count(), path.len(), path.to_string()))
        .cloned()
}

fn parent_of(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Rewrite every wikilink in `src` that resolves to `old_path` so it points at `new_path`.
///
/// The author's link style is preserved: a link written as a bare basename stays a bare
/// basename if that is still unambiguous, and a link written as a full path stays a full
/// path. Aliases and heading fragments are untouched.
pub fn rewrite_wikilinks(src: &str, rename: RenameContext<'_>) -> Option<String> {
    let links = scan_wikilinks(src);
    if links.is_empty() {
        return None;
    }

    let edits = retarget_edits(&links, rename);
    if edits.is_empty() {
        return None;
    }
    Some(apply_edits(src, &edits))
}

/// What a rewrite needs to know about the rename it is applying.
/// Who is being renamed, where to, and the two link graphs to resolve against.
///
/// Loose `&str` arguments in a row invite a silent transposition — `old_path` and
/// `new_path` are the same type, and swapping them rewrites every link the wrong way.
/// Naming them at the call site removes that class of mistake.
///
/// Two resolvers, not one, because the question is asked twice: *before* the rename, to
/// decide whether a link pointed at the renamed document at all, and *after* it, to check
/// that the replacement text still means that same document. Without the second, a bare
/// link can be rewritten into a name that now resolves somewhere else entirely.
pub struct RenameContext<'a> {
    /// The linking document, as it is named before the rename.
    pub source_before: &'a str,
    /// The linking document, as it will be named after it (unchanged unless it moved too).
    pub source_after: &'a str,
    /// The path being renamed away from.
    pub old_path: &'a str,
    /// The path being renamed to.
    pub new_path: &'a str,
    /// The corpus as it stands now.
    pub before: &'a Resolver,
    /// The corpus as it will stand once the rename is applied.
    pub after: &'a Resolver,
}

/// The target replacements for every link that pointed at the renamed document, as
/// `(start, end, replacement)` byte ranges into the source.
fn retarget_edits(links: &[WikiLink], rename: RenameContext) -> Vec<(usize, usize, String)> {
    let bare = bare_link_name(rename.new_path);
    let qualified = rename
        .new_path
        .strip_suffix(".md")
        .unwrap_or(rename.new_path)
        .to_string();

    // Keep the author's shorthand only when it still points at the renamed document.
    // `[[a]]` meaning `x/a.md` must not become `[[b]]` when a different `y/b.md` exists —
    // that is not a broken link the reader can spot, it is a plausible link to the wrong
    // document, which is the failure this module exists to avoid.
    let bare_is_unambiguous =
        rename.after.resolve(rename.source_after, &bare).as_deref() == Some(rename.new_path);

    let mut edits = Vec::new();
    for link in links {
        let resolved = rename.before.resolve(rename.source_before, &link.target);
        if resolved.as_deref() != Some(rename.old_path) {
            continue;
        }
        let replacement = if link.target.contains('/') || !bare_is_unambiguous {
            qualified.clone()
        } else {
            bare.clone()
        };
        if replacement != link.target {
            edits.push((link.target_start, link.target_end, replacement));
        }
    }
    edits
}

/// The shorthand a reader would write for this path.
///
/// Only markdown may drop its extension: a wikilink resolves `.md` implicitly, so
/// `[[note]]` finds `note.md`. Dropping it from anything else turns `![[img.png]]` into
/// `[[img]]`, which resolves to a *different* file — `img.md` — and silently converts an
/// image embed into a link to a note.
fn bare_link_name(path: &str) -> String {
    let name = Path::new(path);
    let keep_extension = !path.to_lowercase().ends_with(".md");
    let part = if keep_extension {
        name.file_name()
    } else {
        name.file_stem()
    };
    part.map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn apply_edits(src: &str, edits: &[(usize, usize, String)]) -> String {
    let mut out = String::with_capacity(src.len());
    let mut cursor = 0usize;
    for (start, end, replacement) in edits {
        out.push_str(&src[cursor..*start]);
        out.push_str(replacement);
        cursor = *end;
    }
    out.push_str(&src[cursor..]);
    out
}

/// One `#tag` found in a source document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRef {
    pub start: usize,
    pub end: usize,
    pub name: String,
}

/// Scan `src` for inline `#tags`, skipping code spans and fenced code blocks.
///
/// A tag must start at a word boundary, so `C#` in prose and a URL fragment such as
/// `example.com/page#section` are not tags. The leading `#` markers of an ATX heading are
/// skipped, but a `#tag` written later on a heading line still counts, matching Obsidian.
pub fn scan_tags(src: &str) -> Vec<TagRef> {
    let bytes = src.as_bytes();
    let mut tags = Vec::new();
    let mut cursor = MarkdownCursor::new();

    while cursor.index < bytes.len() {
        if cursor.skip_heading_markers(bytes) {
            continue;
        }
        let start = cursor.index;
        if cursor.next_prose_byte(src).is_none() {
            continue;
        }
        match tag_at(src, start) {
            Some(tag) => {
                cursor.resume_at(tag.end);
                tags.push(tag);
            }
            None => cursor.resume_at(start + 1),
        }
    }
    tags
}

/// The index just past the `#` run opening an ATX heading at `start`, so those markers are
/// not read as a tag.
fn heading_markers_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut end = start;
    while end < bytes.len() && bytes[end] == b'#' {
        end += 1;
    }
    let level = end - start;
    if level == 0 || level > MAX_HEADING_LEVEL {
        return None;
    }
    match bytes.get(end) {
        Some(b' ' | b'\t') => Some(end),
        _ => None,
    }
}

/// The tag written at `start`, if what is there is a tag at all.
fn tag_at(src: &str, start: usize) -> Option<TagRef> {
    let bytes = src.as_bytes();
    if bytes[start] != b'#' {
        return None;
    }
    // `(` and `[` are deliberately absent: `[Setup](#installation)` is a markdown anchor
    // link, not a tag. Treating it as one both destroys the link and pollutes the tag
    // index with every intra-page anchor in the vault.
    let preceded_by_word = start > 0
        && !matches!(
            bytes[start - 1],
            b' ' | b'\t' | b'\n' | b'>' | b',' | b';' | b'\'' | b'"'
        );
    if preceded_by_word {
        return None;
    }

    let mut end = start + 1;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'_' | b'-' | b'/'))
    {
        end += 1;
    }
    let name = &src[start + 1..end];
    // Obsidian does not treat a purely numeric `#123` as a tag.
    if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(TagRef {
        start,
        end,
        name: name.trim_end_matches('/').to_string(),
    })
}

/// Escape `$` characters that are not maths delimiters.
///
/// A knowledge base is full of currency — "it costs $5 today and $7 tomorrow", "the draft
/// was $31,668.01" — and a permissive `$...$` scanner pairs those up, swallowing the prose
/// between them into a MathML blob with the spaces stripped out. The damage is silent: the
/// document still renders, just wrongly.
///
/// The rules here are Pandoc's, which exist for exactly this reason: a closing `$` may not
/// be preceded by whitespace nor followed by a digit, and a span may not cross an inline
/// code span. Anything that fails them is escaped so the parser treats it as literal text.
pub fn escape_stray_dollars(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut cursor = MarkdownCursor::new();

    while cursor.index < bytes.len() {
        let start = cursor.index;
        // Whatever the cursor steps over is code or a line break: copy it through as it is.
        let Some(byte) = cursor.next_prose_byte(src) else {
            out.push_str(&src[start..cursor.index]);
            continue;
        };

        if byte == b'$' {
            cursor.resume_at(copy_maths_or_escape(src, start, &mut out));
            continue;
        }
        let char_len = char_len_at(src, start);
        out.push_str(&src[start..start + char_len]);
        cursor.resume_at(start + char_len);
    }
    out
}

/// Copy a maths span starting at `start` through untouched, or escape a `$` that opens
/// nothing. Returns the index to carry on from.
fn copy_maths_or_escape(src: &str, start: usize, out: &mut String) -> usize {
    let bytes = src.as_bytes();
    let span_end = if bytes.get(start + 1) == Some(&b'$') {
        find_display_close(bytes, start + 2).map(|close| close + 2)
    } else {
        find_inline_math_close(bytes, start).map(|close| close + 1)
    };

    match span_end {
        Some(end) => {
            out.push_str(&src[start..end]);
            end
        }
        None => {
            out.push_str("\\$");
            start + 1
        }
    }
}

fn char_len_at(src: &str, index: usize) -> usize {
    src[index..].chars().next().map(char::len_utf8).unwrap_or(1)
}

fn find_display_close(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 1 < bytes.len() {
        if bytes[i] == b'$' && bytes[i + 1] == b'$' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// The closing `$` of a valid inline span starting at `open`, if there is one.
fn find_inline_math_close(bytes: &[u8], open: usize) -> Option<usize> {
    // An opening delimiter is never followed by whitespace, and never by a digit.
    //
    // The digit rule is the one that makes currency safe. Pandoc's rules alone are not
    // enough: in "$5 元，公式 $x^2$" the `$5` skips the invalid closer and pairs with the
    // real formula's, swallowing the prose between them. Requiring a non-digit means a
    // dollar amount can never open a span at all.
    //
    // The cost is that maths written as `$5x$` renders literally. That is visible and
    // trivially fixed by the author, whereas silently eating a sentence is neither — and
    // in a knowledge base, dollar amounts vastly outnumber formulae starting with a digit.
    match bytes.get(open + 1) {
        None | Some(b' ') | Some(b'\t') | Some(b'\n') => return None,
        Some(byte) if byte.is_ascii_digit() => return None,
        _ => {}
    }

    let mut i = open + 1;
    while i < bytes.len() {
        match bytes[i] {
            // Maths does not span a blank line, and does not cross inline code.
            b'\n' if matches!(bytes.get(i + 1), Some(b'\n')) => return None,
            b'`' => return None,
            // A `$` that closes nothing is skipped over, so `$a$ and $b$` still works.
            b'$' if closes_inline_math(bytes, i) => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Pandoc's rule for a closing delimiter: not preceded by whitespace, not followed by a
/// digit — the second half being what keeps "$5 and $7" out of a maths span.
fn closes_inline_math(bytes: &[u8], index: usize) -> bool {
    let preceded_by_space = matches!(bytes[index - 1], b' ' | b'\t' | b'\n');
    let followed_by_digit = bytes
        .get(index + 1)
        .map(|byte| byte.is_ascii_digit())
        .unwrap_or(false);
    !preceded_by_space && !followed_by_digit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(src: &str) -> Vec<String> {
        scan_wikilinks(src).into_iter().map(|l| l.target).collect()
    }

    #[test]
    fn finds_a_plain_link() {
        let links = scan_wikilinks("see [[Some Note]] here");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Some Note");
        assert!(!links[0].embed);
        assert_eq!(links[0].alias, None);
    }

    #[test]
    fn parses_alias_heading_and_embed_forms() {
        let links = scan_wikilinks("[[A|shown]] [[B#Heading]] ![[img.png]] [[C#H|both]]");
        let [aliased, with_heading, embed, combined] = links.as_slice() else {
            panic!("expected four links, got {links:?}");
        };
        assert_eq!(aliased.alias.as_deref(), Some("shown"));
        assert_eq!(with_heading.heading.as_deref(), Some("Heading"));
        assert!(embed.embed);
        assert_eq!(combined.target, "C");
        assert_eq!(combined.heading.as_deref(), Some("H"));
        assert_eq!(combined.alias.as_deref(), Some("both"));
    }

    #[test]
    fn parses_a_heading_only_link() {
        let links = scan_wikilinks("jump to [[#Local Section]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "");
        assert_eq!(links[0].heading.as_deref(), Some("Local Section"));
    }

    #[test]
    fn ignores_links_inside_fenced_code() {
        let src = "before [[Real]]\n```\n[[NotALink]]\n```\nafter [[AlsoReal]]";
        assert_eq!(targets(src), vec!["Real", "AlsoReal"]);
    }

    #[test]
    fn ignores_links_inside_tilde_fences_and_nested_backticks() {
        let src = "~~~\n[[Hidden]]\n~~~\n[[Visible]]";
        assert_eq!(targets(src), vec!["Visible"]);
    }

    #[test]
    fn ignores_links_inside_inline_code() {
        let src = "use `[[Literal]]` but link [[Real]]";
        assert_eq!(targets(src), vec!["Real"]);
    }

    #[test]
    fn ignores_empty_and_malformed_brackets() {
        assert!(scan_wikilinks("[[]] [[").is_empty());
        assert!(scan_wikilinks("[[unclosed\nnext line]]").is_empty());
    }

    /// A corpus with a deliberate stem collision (`Alpha` in two folders) and a
    /// non-markdown file, so rewrite tests can express the cases that actually bite.
    const CORPUS: &[&str] = &[
        "index.md",
        "notes/Alpha.md",
        "notes/deep/Alpha.md",
        "notes/Beta.md",
        "assets/img.png",
        "assets/img.md",
        "Projects/My Project.md",
    ];

    fn resolver() -> Resolver {
        Resolver::new(CORPUS.iter().copied())
    }

    /// The corpus as it stands once `old` has become `new`.
    fn resolver_after(old: &str, new: &str) -> Resolver {
        Resolver::new(
            CORPUS
                .iter()
                .map(|path| if *path == old { new } else { *path })
                .collect::<Vec<_>>(),
        )
    }

    /// Rewrite `src` for a rename, wiring up both corpora.
    fn rewrite_for_rename(src: &str, source: &str, old: &str, new: &str) -> Option<String> {
        let before = resolver();
        let after = resolver_after(old, new);
        rewrite_wikilinks(
            src,
            RenameContext {
                source_before: source,
                source_after: source,
                old_path: old,
                new_path: new,
                before: &before,
                after: &after,
            },
        )
    }

    #[test]
    fn resolves_an_exact_path_with_or_without_extension() {
        let r = resolver();
        assert_eq!(
            r.resolve("index.md", "notes/Beta.md").as_deref(),
            Some("notes/Beta.md")
        );
        assert_eq!(
            r.resolve("index.md", "notes/Beta").as_deref(),
            Some("notes/Beta.md")
        );
    }

    #[test]
    fn resolves_a_unique_basename_from_anywhere() {
        let r = resolver();
        assert_eq!(
            r.resolve("index.md", "Beta").as_deref(),
            Some("notes/Beta.md")
        );
    }

    #[test]
    fn resolution_is_case_insensitive() {
        let r = resolver();
        assert_eq!(
            r.resolve("index.md", "bETa").as_deref(),
            Some("notes/Beta.md")
        );
    }

    #[test]
    fn prefers_a_sibling_when_the_basename_is_ambiguous() {
        let r = resolver();
        assert_eq!(
            r.resolve("notes/deep/Other.md", "Alpha").as_deref(),
            Some("notes/deep/Alpha.md"),
            "a note in the same folder should win"
        );
    }

    #[test]
    fn falls_back_to_the_shallowest_path_when_ambiguous_and_not_a_sibling() {
        let r = resolver();
        assert_eq!(
            r.resolve("index.md", "Alpha").as_deref(),
            Some("notes/Alpha.md")
        );
    }

    #[test]
    fn does_not_basename_match_a_target_that_looks_like_a_path() {
        let r = resolver();
        assert_eq!(
            r.resolve("index.md", "archive/Alpha"),
            None,
            "a path-shaped target must not silently resolve to an unrelated note"
        );
    }

    #[test]
    fn returns_none_for_an_unresolved_link() {
        assert_eq!(resolver().resolve("index.md", "Does Not Exist"), None);
    }

    #[test]
    fn resolves_relative_markdown_links() {
        let r = resolver();
        assert_eq!(
            r.resolve_relative("notes/Beta.md", "./Alpha.md").as_deref(),
            Some("notes/Alpha.md")
        );
        assert_eq!(
            r.resolve_relative("notes/Beta.md", "../index.md")
                .as_deref(),
            Some("index.md")
        );
        assert_eq!(
            r.resolve_relative("notes/Beta.md", "deep/Alpha.md")
                .as_deref(),
            Some("notes/deep/Alpha.md")
        );
        assert_eq!(
            r.resolve_relative("index.md", "https://example.com")
                .as_deref(),
            None
        );
    }

    #[test]
    fn rewrites_only_links_that_resolve_to_the_renamed_file() {
        let src = "[[Beta]] and [[notes/Beta]] and [[Alpha]] and prose about Beta.";
        let out = rewrite_for_rename(src, "index.md", "notes/Beta.md", "notes/Gamma.md").unwrap();
        assert_eq!(
            out,
            "[[Gamma]] and [[notes/Gamma]] and [[Alpha]] and prose about Beta."
        );
    }

    #[test]
    fn rewriting_preserves_alias_and_heading() {
        let src = "[[Beta|the beta note]] and [[Beta#Section]] and ![[Beta]]";
        let out = rewrite_for_rename(src, "index.md", "notes/Beta.md", "notes/Gamma.md").unwrap();
        assert_eq!(
            out,
            "[[Gamma|the beta note]] and [[Gamma#Section]] and ![[Gamma]]"
        );
    }

    #[test]
    fn rewriting_leaves_code_blocks_alone() {
        let src = "```\n[[Beta]]\n```\n[[Beta]]";
        let out = rewrite_for_rename(src, "index.md", "notes/Beta.md", "notes/Gamma.md").unwrap();
        assert_eq!(
            out, "```\n[[Beta]]\n```\n[[Gamma]]",
            "a link inside a code sample must survive a rename"
        );
    }

    /// `[[a]]` in `y/c.md` unambiguously meant `x/a.md`. Renaming it to `x/b.md` must not
    /// leave a bare `[[b]]`, because from `y/c.md` that resolves to its own sibling
    /// `y/b.md` — a different document. A plausible link to the wrong note is worse than a
    /// visible broken one, and the server reports the rename as a success either way.
    #[test]
    fn a_bare_link_is_qualified_when_the_new_name_would_resolve_elsewhere() {
        let before = Resolver::new(["x/a.md", "y/b.md", "y/c.md"]);
        let after = Resolver::new(["x/b.md", "y/b.md", "y/c.md"]);
        let out = rewrite_wikilinks(
            "Link to [[a]].",
            RenameContext {
                source_before: "y/c.md",
                source_after: "y/c.md",
                old_path: "x/a.md",
                new_path: "x/b.md",
                before: &before,
                after: &after,
            },
        )
        .unwrap();
        assert_eq!(
            out, "Link to [[x/b]].",
            "a bare rewrite would have pointed at y/b.md"
        );
    }

    #[test]
    fn a_bare_link_stays_bare_when_the_new_stem_is_still_unique() {
        let out = rewrite_for_rename(
            "See [[Beta]].",
            "index.md",
            "notes/Beta.md",
            "notes/Gamma.md",
        )
        .unwrap();
        assert_eq!(
            out, "See [[Gamma]].",
            "the author's shorthand should survive"
        );
    }

    /// `![[img.png]]` is an image embed. Dropping the extension makes it `[[img]]`, which
    /// resolves to `img.md` — a different file, and no longer an image.
    #[test]
    fn a_non_markdown_target_keeps_its_extension() {
        let out = rewrite_for_rename(
            "Embed ![[img.png]]",
            "index.md",
            "assets/img.png",
            "media/picture.png",
        )
        .unwrap();
        assert!(
            out.contains("picture.png"),
            "the extension must survive or the embed becomes a link to a note: {out}"
        );
        assert!(!out.contains("[[picture]]"), "got {out}");
    }

    #[test]
    fn rewriting_a_document_with_no_matching_links_changes_nothing() {
        assert_eq!(
            rewrite_for_rename(
                "[[Alpha]] only",
                "index.md",
                "notes/Beta.md",
                "notes/Gamma.md"
            ),
            None
        );
    }

    #[test]
    fn rewrites_a_name_containing_spaces() {
        let src = "see [[My Project]] now";
        let out = rewrite_for_rename(
            src,
            "index.md",
            "Projects/My Project.md",
            "Projects/Renamed Project.md",
        )
        .unwrap();
        assert_eq!(out, "see [[Renamed Project]] now");
    }

    #[test]
    fn byte_ranges_point_at_the_target_text() {
        let src = "x [[Beta|alias]] y";
        let link = &scan_wikilinks(src)[0];
        assert_eq!(&src[link.target_start..link.target_end], "Beta");
        assert_eq!(&src[link.start..link.end], "[[Beta|alias]]");
    }

    fn tag_names(src: &str) -> Vec<String> {
        scan_tags(src).into_iter().map(|t| t.name).collect()
    }

    #[test]
    fn finds_plain_inline_tags() {
        assert_eq!(
            tag_names("about #rust and #web-dev today"),
            vec!["rust", "web-dev"]
        );
    }

    #[test]
    fn supports_nested_tag_paths() {
        assert_eq!(tag_names("#area/work/admin"), vec!["area/work/admin"]);
    }

    #[test]
    fn ignores_tags_inside_code() {
        assert_eq!(tag_names("`#nope` and #yes"), vec!["yes"]);
        assert_eq!(tag_names("```\n#nope\n```\n#yes"), vec!["yes"]);
    }

    #[test]
    fn ignores_a_hash_that_is_not_at_a_word_boundary() {
        assert_eq!(tag_names("the language C# is fine"), Vec::<String>::new());
        assert_eq!(
            tag_names("see example.com/page#section"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn ignores_purely_numeric_hashes() {
        assert_eq!(tag_names("issue #123 and #v2"), vec!["v2"]);
    }

    #[test]
    fn a_markdown_anchor_link_is_not_a_tag() {
        assert_eq!(
            tag_names("See [Setup](#installation) for details."),
            Vec::<String>::new()
        );
        assert_eq!(tag_names("[a](#one) and [b](#two)"), Vec::<String>::new());
    }

    #[test]
    fn a_tag_after_an_opening_bracket_in_prose_is_still_not_a_tag() {
        // Conservative: losing a tag is recoverable, corrupting a link is not.
        assert_eq!(tag_names("(#notatag)"), Vec::<String>::new());
    }

    #[test]
    fn does_not_read_heading_markers_as_tags() {
        assert_eq!(tag_names("## Heading text"), Vec::<String>::new());
    }

    #[test]
    fn still_finds_a_tag_written_on_a_heading_line() {
        assert_eq!(tag_names("## Heading with #inline"), vec!["inline"]);
    }

    #[test]
    fn currency_in_prose_is_not_treated_as_maths() {
        let out = escape_stray_dollars("it costs $5 today and $7 tomorrow.");
        assert_eq!(out, "it costs \\$5 today and \\$7 tomorrow.");
    }

    #[test]
    fn a_large_formatted_amount_is_not_maths() {
        let out = escape_stray_dollars("the draft was $31,668.01 and the deposit $10,000.");
        assert!(out.starts_with("the draft was \\$31,668.01"), "got {out}");
        assert!(out.contains("\\$10,000"));
    }

    #[test]
    fn a_currency_amount_cannot_pair_with_a_later_real_formula() {
        // The exact shape that corrupted a fixture: the amount skipped the invalid closer
        // and swallowed everything up to the next genuine delimiter.
        let out = escape_stray_dollars("costs $5 and later $x^2$ formula");
        assert!(out.contains("\\$5"), "got {out}");
        assert!(
            out.contains("$x^2$"),
            "the real formula must survive: {out}"
        );
    }

    #[test]
    fn genuine_inline_maths_is_left_alone() {
        assert_eq!(escape_stray_dollars("here $x^2$ ok"), "here $x^2$ ok");
        assert_eq!(escape_stray_dollars("$a$ and $b$"), "$a$ and $b$");
    }

    #[test]
    fn display_maths_is_left_alone() {
        let src = "before\n$$\nx = y\n$$\nafter";
        assert_eq!(escape_stray_dollars(src), src);
    }

    #[test]
    fn maths_never_crosses_an_inline_code_span() {
        // The regression: `$7` paired with the `$` inside `echo $HOME`, swallowing the
        // sentence between them.
        let src = "costs $7 tomorrow. safe too: `echo $HOME` and `$PATH`.";
        let out = escape_stray_dollars(src);
        assert!(
            out.contains("`echo $HOME`"),
            "code span must be untouched: {out}"
        );
        assert!(out.contains("\\$7"), "the currency must be escaped: {out}");
    }

    /// A fenced block containing any non-ASCII text used to panic: the scanner stepped a
    /// byte at a time inside a fence, and the caller then sliced the source mid-character.
    /// A vault with CJK, accents or emoji in a code sample hit this on every render.
    #[test]
    fn a_fence_containing_non_ascii_text_does_not_panic() {
        for src in [
            "```\n汉字\n```\n",
            "```sh\necho 'café'\n```\n",
            "```\n// 🚀 ship it\n```\ncosts $5 after",
            "~~~\n日本語のコード\n~~~\n",
        ] {
            let out = escape_stray_dollars(src);
            assert!(
                out.contains('\n'),
                "should still produce output for {src:?}"
            );
        }
    }

    #[test]
    fn a_fence_with_non_ascii_is_copied_through_byte_for_byte() {
        let src = "```\n汉字 $HOME\n```\n";
        assert_eq!(escape_stray_dollars(src), src);
    }

    #[test]
    fn scanning_non_ascii_fences_for_links_and_tags_does_not_panic() {
        let src = "```\n汉字 [[NotALink]] #nottag\n```\n[[Real]] #real";
        assert_eq!(scan_wikilinks(src).len(), 1);
        assert_eq!(tag_names(src), vec!["real"]);
    }

    #[test]
    fn dollars_inside_fenced_code_are_untouched() {
        let src = "```sh\necho $HOME $PATH\n```\n";
        assert_eq!(escape_stray_dollars(src), src);
    }

    #[test]
    fn an_unpaired_dollar_is_escaped() {
        assert_eq!(escape_stray_dollars("a lone $ sign"), "a lone \\$ sign");
    }

    #[test]
    fn multibyte_text_survives_escaping() {
        let src = "价格是 $5 元，公式 $x^2$ 保留";
        let out = escape_stray_dollars(src);
        assert!(out.contains("\\$5"), "got {out}");
        assert!(out.contains("$x^2$"), "got {out}");
        assert!(out.contains("价格是"));
    }
}
