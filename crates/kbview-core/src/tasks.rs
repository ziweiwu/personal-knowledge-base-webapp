//! Toggling a task-list checkbox in markdown source.
//!
//! This is deliberately the narrowest possible write. A checkbox click cannot send
//! content: it names a line and the state it wants, and the only edit this module will
//! ever make is replacing the single character between `[` and `]`. If the addressed line
//! no longer looks like a task, nothing is written at all — a click that lands on a
//! document someone else has since edited must fail, not guess.

/// A code fence opens on three or more of its marker character.
const FENCE_MARKERS: usize = 3;

/// What a click wants the checkbox to become.
///
/// An enum rather than a bool because `set_task_state(source, line, true)` reads as
/// nothing at all at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Done,
    Todo,
}

impl TaskState {
    fn marker(self) -> char {
        match self {
            Self::Done => 'x',
            Self::Todo => ' ',
        }
    }
}

/// One task item found in a document: where it is, and whether it is ticked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskLine {
    /// 1-based line number in the source the scan was given.
    pub line: usize,
    pub state: TaskState,
}

/// Why a toggle was refused. Every variant means the file was left untouched.
#[derive(Debug, PartialEq, Eq)]
pub enum TaskError {
    /// The document has no such line.
    NoSuchLine,
    /// The line exists but is not a task item — the document changed underneath the click.
    NotATask,
    /// The line already holds the requested state; there is nothing to write.
    AlreadySet,
}

/// The fence state after `line` has been read, given the state before it.
fn fence_after(line: &str, fence: Option<(u8, usize)>) -> Option<(u8, usize)> {
    let trimmed = line.trim_start();
    let Some(marker @ (b'`' | b'~')) = trimmed.as_bytes().first().copied() else {
        return fence;
    };
    let run = trimmed.bytes().take_while(|byte| *byte == marker).count();
    if run < FENCE_MARKERS {
        return fence;
    }
    match fence {
        Some((open_marker, open_run)) if open_marker == marker && run >= open_run => None,
        Some(existing) => Some(existing),
        None => Some((marker, run)),
    }
}

/// Every line of `source` paired with its 1-based number and whether it sits inside a
/// fenced code block.
///
/// A code sample may contain `- [ ] ...` as *text*. It renders as code, no checkbox is
/// drawn for it, and editing it would silently corrupt the sample — so such a line is
/// refused even though it matches the task shape. The state reported for a line is the
/// one in force *before* that line, so a fence's own opening marker reads as outside.
fn lines_with_fence_state(source: &str) -> impl Iterator<Item = (usize, &str, bool)> {
    let mut fence: Option<(u8, usize)> = None;
    source
        .split_inclusive('\n')
        .enumerate()
        .map(move |(index, text)| {
            let was_inside = fence.is_some();
            fence = fence_after(text, fence);
            (index + 1, text, was_inside)
        })
}

fn inside_fence(source: &str, line: usize) -> bool {
    lines_with_fence_state(source)
        .find(|(number, _, _)| *number == line)
        .is_some_and(|(_, _, in_fence)| in_fence)
}

/// Every task item in `source`, in document order.
///
/// This is the reading half of the same scanner `set_task_state` writes through, which is
/// the point of it: a line number handed to a client can then only ever name a line the
/// writer will accept. Deriving those numbers from the parsed document instead was a real
/// bug — the parser sees a *prepared* source with frontmatter stripped and callouts
/// expanded, so its line numbers drift from the file on disk and a click silently edited
/// a different task.
pub fn task_lines(source: &str) -> Vec<TaskLine> {
    lines_with_fence_state(source)
        .filter(|(_, _, in_fence)| !in_fence)
        .filter_map(|(line, text, _)| task_state(text).map(|state| TaskLine { line, state }))
        .collect()
}

/// Past any indentation and any depth of blockquote marker, to where the content starts.
///
/// A task inside a quote is still a task and the renderer draws a live checkbox for it;
/// refusing it here would leave that box dead and, because the two sides would then
/// disagree on how many tasks the document has, would take every other checkbox in the
/// file down with it.
fn skip_indent_and_quotes(bytes: &[u8]) -> usize {
    let mut i = 0;
    loop {
        while matches!(bytes.get(i), Some(b' ' | b'\t')) {
            i += 1;
        }
        if bytes.get(i) != Some(&b'>') {
            return i;
        }
        i += 1;
    }
}

/// Past a list marker and the whitespace after it, or `None` if there is no marker.
///
/// Accepts the GFM shapes: `-`, `*`, `+` and ordered `1.` / `1)`.
fn skip_list_marker(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    match bytes.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            while matches!(bytes.get(i), Some(b'0'..=b'9')) {
                i += 1;
            }
            if !matches!(bytes.get(i), Some(b'.' | b')')) {
                return None;
            }
            i += 1;
        }
        _ => return None,
    }

    // A list marker must be followed by whitespace, or `-[ ]` would parse as a task.
    if !matches!(bytes.get(i), Some(b' ' | b'\t')) {
        return None;
    }
    while matches!(bytes.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    Some(i)
}

/// The byte offset of the state character inside a task marker on `line`, if it is one.
fn state_offset(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let i = skip_list_marker(bytes, skip_indent_and_quotes(bytes))?;

    if bytes.get(i) != Some(&b'[') {
        return None;
    }
    let state = i + 1;
    if bytes.get(state + 1) != Some(&b']') {
        return None;
    }
    // GFM requires whitespace after the closing bracket for this to be a task item.
    if !matches!(bytes.get(state + 2), None | Some(b' ' | b'\t')) {
        return None;
    }
    match bytes.get(state)? {
        b' ' | b'x' | b'X' => Some(state),
        _ => None,
    }
}

/// Whether `line` is a task item, and what state it is in.
pub fn task_state(line: &str) -> Option<TaskState> {
    let at = state_offset(line)?;
    Some(if line.as_bytes()[at].eq_ignore_ascii_case(&b' ') {
        TaskState::Todo
    } else {
        TaskState::Done
    })
}

/// Set the checkbox on the 1-based `line` of `source` to `wanted`.
///
/// Returns the whole document with that one character changed. Line endings, indentation
/// and every other byte are preserved exactly, because this runs against files an editor
/// and a sync client are also writing.
pub fn set_task_state(source: &str, line: usize, wanted: TaskState) -> Result<String, TaskError> {
    let (offset, text) = byte_offset_of_line(source, line).ok_or(TaskError::NoSuchLine)?;
    if inside_fence(source, line) {
        return Err(TaskError::NotATask);
    }

    let at = state_offset(text).ok_or(TaskError::NotATask)?;
    if task_state(text) == Some(wanted) {
        return Err(TaskError::AlreadySet);
    }

    let mut out = String::with_capacity(source.len());
    out.push_str(&source[..offset + at]);
    out.push(wanted.marker());
    out.push_str(&source[offset + at + 1..]);
    Ok(out)
}

/// Where the 1-based `line` starts in `source`, and its text.
fn byte_offset_of_line(source: &str, line: usize) -> Option<(usize, &str)> {
    if line == 0 {
        return None;
    }
    let mut offset = 0usize;
    for (index, text) in source.split_inclusive('\n').enumerate() {
        if index + 1 == line {
            return Some((offset, text));
        }
        offset += text.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use TaskState::{Done, Todo};

    /// The 1-based line holding `needle`. Tests then say which line they mean by quoting
    /// it, instead of counting newlines and going stale the moment a fixture gains a line.
    fn line_of(source: &str, needle: &str) -> usize {
        source
            .split_inclusive('\n')
            .position(|line| line.contains(needle))
            .map(|index| index + 1)
            .unwrap_or_else(|| panic!("no line contains {needle:?}"))
    }

    fn expected_task(line: usize, state: TaskState) -> TaskLine {
        TaskLine { line, state }
    }

    #[test]
    fn recognises_the_gfm_task_shapes() {
        assert_eq!(task_state("- [ ] todo"), Some(Todo));
        assert_eq!(task_state("- [x] done"), Some(Done));
        assert_eq!(task_state("- [X] done"), Some(Done));
        assert_eq!(task_state("* [ ] star"), Some(Todo));
        assert_eq!(task_state("+ [ ] plus"), Some(Todo));
        assert_eq!(task_state("1. [ ] ordered"), Some(Todo));
        assert_eq!(task_state("12) [x] ordered"), Some(Done));
        assert_eq!(task_state("    - [ ] nested"), Some(Todo));
        assert_eq!(task_state("\t- [ ] tabbed"), Some(Todo));
    }

    #[test]
    fn rejects_things_that_only_look_like_tasks() {
        for line in [
            "not a task",
            "[ ] no list marker",
            "-[ ] no space after the marker",
            "- [] empty brackets",
            "- [ ]no space after the bracket",
            "- [y] not a state character",
            "- [ x] too wide",
            "1 [ ] no delimiter",
            "> [!note] a callout, not a task",
            "",
        ] {
            assert_eq!(task_state(line), None, "should not be a task: {line:?}");
        }
    }

    #[test]
    fn a_task_inside_a_blockquote_is_a_task() {
        let source = "> - [ ] quoted\n> > - [x] nested\n";
        assert_eq!(
            task_lines(source),
            vec![
                expected_task(line_of(source, "quoted"), Todo),
                expected_task(line_of(source, "nested"), Done),
            ]
        );
        assert_eq!(
            set_task_state(source, line_of(source, "quoted"), Done).unwrap(),
            "> - [x] quoted\n> > - [x] nested\n"
        );
    }

    #[test]
    fn task_lines_are_numbered_against_the_whole_file_frontmatter_included() {
        let source = "---\ntitle: t\n---\n\n- [ ] first\n- [x] second\n";
        assert_eq!(
            task_lines(source),
            vec![
                expected_task(line_of(source, "first"), Todo),
                expected_task(line_of(source, "second"), Done),
            ]
        );
    }

    #[test]
    fn task_lines_skips_task_shapes_inside_a_code_fence() {
        let source = "- [ ] real\n\n```\n- [ ] a sample, not a task\n```\n\n- [x] also real\n";
        assert_eq!(
            task_lines(source),
            vec![
                expected_task(line_of(source, "] real"), Todo),
                expected_task(line_of(source, "also real"), Done),
            ]
        );
    }

    #[test]
    fn toggles_only_the_state_character() {
        let src = "# T\n\n- [ ] first\n- [x] second\n";
        assert_eq!(
            set_task_state(src, line_of(src, "first"), Done).unwrap(),
            "# T\n\n- [x] first\n- [x] second\n"
        );
        assert_eq!(
            set_task_state(src, line_of(src, "second"), Todo).unwrap(),
            "# T\n\n- [ ] first\n- [ ] second\n"
        );
    }

    #[test]
    fn preserves_indentation_and_trailing_text_exactly() {
        let src = "  - [ ] nested task with **bold** and [[a link]]\n";
        assert_eq!(
            set_task_state(src, line_of(src, "nested"), Done).unwrap(),
            "  - [x] nested task with **bold** and [[a link]]\n"
        );
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let src = "- [ ] one\r\n- [ ] two\r\n";
        assert_eq!(
            set_task_state(src, line_of(src, "two"), Done).unwrap(),
            "- [ ] one\r\n- [x] two\r\n"
        );
    }

    #[test]
    fn preserves_multibyte_content_on_the_line() {
        let src = "- [ ] 汉字と日本語 café 🚀\n";
        assert_eq!(
            set_task_state(src, line_of(src, "café"), Done).unwrap(),
            "- [x] 汉字と日本語 café 🚀\n"
        );
    }

    #[test]
    fn a_line_that_is_not_a_task_is_refused_rather_than_edited() {
        let src = "# Heading\n- [ ] task\n";
        assert_eq!(
            set_task_state(src, line_of(src, "Heading"), Done),
            Err(TaskError::NotATask)
        );
    }

    /// A code sample containing task syntax renders as code, draws no checkbox, and must
    /// not be editable through this route even when addressed directly.
    #[test]
    fn a_task_shape_inside_a_code_fence_is_refused() {
        let src = "- [ ] real\n\n```\n- [ ] sample\n```\n";
        assert_eq!(
            set_task_state(src, line_of(src, "sample"), Done),
            Err(TaskError::NotATask)
        );
        assert!(
            set_task_state(src, line_of(src, "real"), Done).is_ok(),
            "the real task still toggles"
        );
    }

    #[test]
    fn a_task_after_a_closed_fence_still_toggles() {
        let src = "```\n- [ ] sample\n```\n- [ ] real\n";
        assert_eq!(
            set_task_state(src, line_of(src, "sample"), Done),
            Err(TaskError::NotATask)
        );
        assert_eq!(
            set_task_state(src, line_of(src, "real"), Done).unwrap(),
            "```\n- [ ] sample\n```\n- [x] real\n"
        );
    }

    #[test]
    fn tilde_fences_are_honoured_too() {
        let src = "~~~\n- [ ] sample\n~~~\n";
        assert_eq!(
            set_task_state(src, line_of(src, "sample"), Done),
            Err(TaskError::NotATask)
        );
    }

    #[test]
    fn a_line_past_the_end_is_refused() {
        let src = "- [ ] only\n";
        let past_the_end = src.lines().count() + 1;
        assert_eq!(
            set_task_state(src, past_the_end, Done),
            Err(TaskError::NoSuchLine)
        );
        // Lines are 1-based, so there is no line zero to address.
        assert_eq!(set_task_state(src, 0, Done), Err(TaskError::NoSuchLine));
    }

    #[test]
    fn setting_the_state_it_already_has_writes_nothing() {
        let done = "- [x] done\n";
        let todo = "- [ ] todo\n";
        assert_eq!(
            set_task_state(done, line_of(done, "done"), Done),
            Err(TaskError::AlreadySet)
        );
        assert_eq!(
            set_task_state(todo, line_of(todo, "todo"), Todo),
            Err(TaskError::AlreadySet)
        );
    }

    #[test]
    fn a_document_without_a_trailing_newline_still_toggles() {
        let src = "- [ ] last";
        assert_eq!(
            set_task_state(src, line_of(src, "last"), Done).unwrap(),
            "- [x] last"
        );
    }

    /// The document is otherwise byte-identical: exactly one character differs.
    #[test]
    fn exactly_one_byte_changes() {
        let src = "intro\n\n- [ ] a\n- [ ] b\n\noutro\n";
        let out = set_task_state(src, line_of(src, "] b"), Done).unwrap();
        assert_eq!(src.len(), out.len());
        let differing = src.bytes().zip(out.bytes()).filter(|(a, b)| a != b).count();
        assert_eq!(differing, 1);
    }
}
