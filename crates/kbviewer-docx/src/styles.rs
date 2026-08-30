//! `word/styles.xml`: mapping paragraph style ids onto heading levels.

use std::collections::HashMap;

use quick_xml::events::{BytesEnd, BytesStart};

use crate::xml::{attribute, local_name, ElementBoundary, PartElements};
use crate::DocxError;

/// Deepest heading HTML has.
const MAX_HEADING_LEVEL: u8 = 6;

#[derive(Debug, Default)]
pub(crate) struct Styles {
    /// Normalised style id to heading level.
    heading_levels: HashMap<String, u8>,
}

impl Styles {
    pub(crate) fn parse(xml: &str, part: &str) -> Result<Self, DocxError> {
        let mut elements = PartElements::new(xml, part, "styles");
        let mut heading_levels = HashMap::new();
        let mut open = OpenStyle::default();

        while let Some(boundary) = elements.next_boundary()? {
            match boundary {
                ElementBoundary::Open(element) => open.read_element(&element),
                ElementBoundary::Close(element) => open.close(&element, &mut heading_levels),
            }
        }

        Ok(Self { heading_levels })
    }

    /// The heading level a paragraph style implies, if any.
    ///
    /// Falls back to reading the style id itself, so a document that does not
    /// ship a styles part still gets headings out of `Heading2`.
    pub(crate) fn heading_level(&self, style_id: &str) -> Option<u8> {
        self.heading_levels
            .get(&normalize(style_id))
            .copied()
            .or_else(|| heading_level_of(style_id))
    }
}

/// The `w:style` the walk is currently inside.
struct OpenStyle {
    id: Option<String>,
    name: Option<String>,
    outline_level: Option<u8>,
    /// Character styles such as "Heading 2 Char" must not turn a run into a
    /// heading, so only paragraph styles count.
    is_paragraph_style: bool,
}

impl Default for OpenStyle {
    fn default() -> Self {
        Self {
            id: None,
            name: None,
            outline_level: None,
            is_paragraph_style: true,
        }
    }
}

impl OpenStyle {
    /// Absorb one element of the styles part.
    fn read_element(&mut self, element: &BytesStart) {
        match local_name(element).as_str() {
            "style" => self.begin(element),
            "name" if self.id.is_some() => self.name = attribute(element, "val"),
            "outlineLvl" if self.id.is_some() => {
                self.outline_level = outline_heading_level(element);
            }
            _ => {}
        }
    }

    fn begin(&mut self, element: &BytesStart) {
        self.id = attribute(element, "styleId");
        self.name = None;
        self.outline_level = None;
        self.is_paragraph_style = attribute(element, "type")
            .map(|kind| kind == "paragraph")
            .unwrap_or(true);
    }

    /// `</w:style>` is where a style's heading level is decided, because the
    /// name and the outline level both arrive after the id.
    fn close(&mut self, element: &BytesEnd, heading_levels: &mut HashMap<String, u8>) {
        if element.local_name().as_ref() != "style" {
            return;
        }
        if let (Some(id), true) = (self.id.take(), self.is_paragraph_style) {
            if let Some(level) = self.heading_level(&id) {
                heading_levels.insert(normalize(&id), level);
            }
        }
        self.name = None;
        self.outline_level = None;
        self.is_paragraph_style = true;
    }

    fn heading_level(&self, id: &str) -> Option<u8> {
        heading_level_of(id)
            .or_else(|| self.name.as_deref().and_then(heading_level_of))
            .or(self.outline_level)
    }
}

/// The heading level a `w:outlineLvl` asks for, if HTML has a heading that
/// deep. The attribute counts from zero, where `<h1>` is level one.
pub(crate) fn outline_heading_level(element: &BytesStart) -> Option<u8> {
    attribute(element, "val")
        .and_then(|value| value.parse::<u8>().ok())
        .filter(|level| *level < MAX_HEADING_LEVEL)
        .map(|level| level + 1)
}

/// Fold a style id or name into a comparable key: `Heading2`, `heading 2` and
/// `Heading-2` all become `heading2`.
fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Read a heading level straight out of a style id or display name.
fn heading_level_of(value: &str) -> Option<u8> {
    let normalized = normalize(value);
    if let Some(digits) = normalized.strip_prefix("heading") {
        return digits
            .parse::<u8>()
            .ok()
            .filter(|level| (1..=MAX_HEADING_LEVEL).contains(level));
    }
    // A document title is the one non-`Heading` style worth promoting: it is
    // the outline's root. `Subtitle` is deliberately left alone, because it
    // is usually a byline or a date rather than a section.
    (normalized == "title").then_some(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A level away from both ends of the range, so that a spaced-out style
    /// name is exercised on something other than the boundary values.
    const MIDDLE_HEADING_LEVEL: u8 = 3;

    #[test]
    fn reads_heading_levels_from_both_naming_conventions() {
        let styles = Styles::default();
        assert_eq!(styles.heading_level("Heading1"), Some(1));
        assert_eq!(
            styles.heading_level(&format!("heading {MIDDLE_HEADING_LEVEL}")),
            Some(MIDDLE_HEADING_LEVEL)
        );
        assert_eq!(styles.heading_level("Heading-2"), Some(2));
        assert_eq!(styles.heading_level("Heading9"), None);
        assert_eq!(styles.heading_level("BodyText"), None);
    }

    #[test]
    fn promotes_the_title_style_but_not_the_subtitle_style() {
        let styles = Styles::default();
        assert_eq!(styles.heading_level("Title"), Some(1));
        assert_eq!(styles.heading_level("Subtitle"), None);
    }

    #[test]
    fn reads_heading_level_from_the_style_name_and_outline_level() {
        let xml = r#"<w:styles xmlns:w="x">
          <w:style w:type="paragraph" w:styleId="Ttulo2"><w:name w:val="heading 2"/></w:style>
          <w:style w:type="paragraph" w:styleId="ChapterMark"><w:name w:val="Chapter Mark"/>
            <w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style>
          <w:style w:type="character" w:styleId="Heading2Char"><w:name w:val="Heading 2 Char"/></w:style>
        </w:styles>"#;
        let styles = Styles::parse(xml, "word/styles.xml").unwrap();
        assert_eq!(styles.heading_level("Ttulo2"), Some(2));
        assert_eq!(styles.heading_level("ChapterMark"), Some(1));
        assert_eq!(styles.heading_level("Heading2Char"), None);
    }

    #[test]
    fn truncated_styles_are_an_error_not_a_panic() {
        assert!(Styles::parse("<w:styles><w:style", "word/styles.xml").is_err());
    }
}
