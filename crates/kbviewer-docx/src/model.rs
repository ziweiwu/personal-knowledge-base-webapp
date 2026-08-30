//! The intermediate document model.
//!
//! Parsing produces this; rendering consumes it. Keeping the two apart is
//! what makes run merging and list nesting tractable, since both need to look
//! at neighbouring items rather than at a single event.

/// Baseline position of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum VerticalAlign {
    #[default]
    Baseline,
    Superscript,
    Subscript,
}

/// Character formatting carried by a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Formatting {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub vertical_align: VerticalAlign,
}

/// A piece of paragraph content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inline {
    Text {
        text: String,
        formatting: Formatting,
    },
    LineBreak,
    Image {
        rel_id: String,
        alt: String,
    },
    Link {
        href: String,
        scope: LinkScope,
        children: Vec<Inline>,
    },
}

/// How far a link reaches, which is what decides its `rel` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkScope {
    /// A target outside the document, which gets `rel="noopener noreferrer"`.
    External,
    /// An anchor within the document, which does not need one.
    InDocument,
}

/// Which list a paragraph belongs to, and how deeply nested it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListMembership {
    pub level: u8,
    pub ordered: bool,
}

/// A `w:p`, already classified as body text, a heading, or a list item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Paragraph {
    /// `Some(1..=6)` renders as `<h1>`..`<h6>`.
    pub heading_level: Option<u8>,
    pub list: Option<ListMembership>,
    pub inlines: Vec<Inline>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableCell {
    pub blocks: Vec<Block>,
    pub colspan: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Table {
    pub rows: Vec<TableRow>,
}

/// A block-level item of document content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Block {
    Paragraph(Paragraph),
    Table(Table),
}

/// Append text to an inline sequence, merging it into the previous run when
/// the formatting is identical.
///
/// Word splits runs aggressively -- often mid-word, at spell-check
/// boundaries -- so without this the output is one `<span>`-worth of markup
/// per syllable.
pub(crate) fn push_text(inlines: &mut Vec<Inline>, text: &str, formatting: Formatting) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text {
        text: previous,
        formatting: previous_formatting,
    }) = inlines.last_mut()
    {
        if *previous_formatting == formatting {
            previous.push_str(text);
            return;
        }
    }
    inlines.push(Inline::Text {
        text: text.to_string(),
        formatting,
    });
}

/// Append one inline sequence to another, merging across the seam when the
/// formatting allows it.
pub(crate) fn extend_inlines(out: &mut Vec<Inline>, incoming: Vec<Inline>) {
    for inline in incoming {
        match inline {
            Inline::Text { text, formatting } => push_text(out, &text, formatting),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_adjacent_runs_with_equal_formatting() {
        let bold = Formatting {
            bold: true,
            ..Formatting::default()
        };
        let mut inlines = Vec::new();
        push_text(&mut inlines, "Hel", bold);
        push_text(&mut inlines, "lo", bold);
        push_text(&mut inlines, " there", Formatting::default());

        assert_eq!(inlines.len(), 2);
        assert_eq!(
            inlines[0],
            Inline::Text {
                text: "Hello".to_string(),
                formatting: bold
            }
        );
    }

    #[test]
    fn ignores_empty_text() {
        let mut inlines = Vec::new();
        push_text(&mut inlines, "", Formatting::default());
        assert!(inlines.is_empty());
    }
}
