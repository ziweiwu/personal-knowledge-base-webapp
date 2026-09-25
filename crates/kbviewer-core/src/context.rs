//! The line a link appears on, cut to a readable width around the link.
//!
//! A backlink that only names the linking note makes the reader open it to learn why
//! it links here. The line it links from usually answers that on its own.

/// Longest snippet a backlink carries, in characters. Enough for a sentence either
/// side of the link; a whole paragraph would repeat the note one click away.
pub const MAX_CONTEXT_CHARS: usize = 160;

/// The line of `source` holding the byte range `start..end`, trimmed of surrounding
/// whitespace, and cut down to [`MAX_CONTEXT_CHARS`] centred on that range when the
/// line runs longer. A cut end is marked with an ellipsis so the sentence visibly
/// continues. The cut is made in characters, never bytes, so a CJK line is never
/// split inside a codepoint.
pub fn line_around(source: &str, start: usize, end: usize) -> String {
    let line_start = source[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |at| start + at);
    let line = &source[line_start..line_end];
    let trimmed = line.trim();
    let leading = line.len() - line.trim_start().len();
    // The link is never whitespace, so it always survives the trim.
    let link_from = source[line_start + leading..start].chars().count();
    let link_to = link_from + source[start..end].chars().count();
    window(trimmed, link_from, link_to)
}

fn window(text: &str, link_from: usize, link_to: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_CONTEXT_CHARS {
        return text.to_string();
    }
    let spare = MAX_CONTEXT_CHARS.saturating_sub(link_to - link_from);
    let window_end = (link_from.saturating_sub(spare / 2) + MAX_CONTEXT_CHARS).min(chars.len());
    let window_start = window_end - MAX_CONTEXT_CHARS;
    let cut_start = window_start > 0;
    let cut_end = window_end < chars.len();
    // The ellipses are paid for out of the budget, so the result never exceeds it.
    let body = &chars[window_start + usize::from(cut_start)..window_end - usize::from(cut_end)];
    let mut out = String::with_capacity(MAX_CONTEXT_CHARS * 2);
    if cut_start {
        out.push('…');
    }
    out.extend(body);
    if cut_end {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = "[[target]]";

    fn count(text: &str) -> usize {
        text.chars().count()
    }

    /// `word` repeated until it alone overruns the snippet budget.
    fn past_the_limit(word: &str) -> String {
        word.repeat(MAX_CONTEXT_CHARS / count(word) + 1)
    }

    #[test]
    fn a_short_line_comes_back_whole_and_trimmed() {
        let source = "# Title\n\n   See [[target]] for more.  \n\nLater.\n";
        let start = source.find("[[").unwrap();
        assert_eq!(
            line_around(source, start, start + LINK.len()),
            "See [[target]] for more."
        );
    }

    #[test]
    fn a_long_line_is_cut_to_the_limit_around_the_link() {
        let before = past_the_limit("before ");
        let after = past_the_limit(" after");
        let source = format!("{before}{LINK}{after}\n");
        let start = before.len();
        let snippet = line_around(&source, start, start + LINK.len());
        assert_eq!(count(&snippet), MAX_CONTEXT_CHARS);
        assert!(
            snippet.starts_with('…') && snippet.ends_with('…'),
            "{snippet}"
        );
        let link_at = snippet.find(LINK).expect("the link survives the cut");
        let centre = count(&snippet[..link_at]);
        let half = MAX_CONTEXT_CHARS / 2;
        assert!(
            (half - LINK.len()..=half).contains(&centre),
            "link sits mid-window, at {centre}"
        );
    }

    #[test]
    fn a_link_near_the_start_keeps_the_window_inside_the_line() {
        let lead = "See ";
        let source = format!("{lead}{LINK} {}\n", past_the_limit("tail "));
        let snippet = line_around(&source, lead.len(), lead.len() + LINK.len());
        assert!(snippet.starts_with("See [[target]]"), "{snippet}");
        assert!(snippet.ends_with('…'));
        assert_eq!(count(&snippet), MAX_CONTEXT_CHARS);
    }

    #[test]
    fn a_cjk_line_is_cut_between_characters() {
        let link = "[[目标]]";
        let text = past_the_limit("知识管理");
        let source = format!("{text}{link}{text}\n");
        let start = text.len();
        let snippet = line_around(&source, start, start + link.len());
        assert_eq!(count(&snippet), MAX_CONTEXT_CHARS);
        assert!(snippet.contains(link));
    }
}
