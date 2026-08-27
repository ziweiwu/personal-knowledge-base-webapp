//! `word/numbering.xml`: deciding whether a numbered paragraph is a bullet
//! list or an ordered list.

use std::collections::HashMap;

use quick_xml::events::{BytesEnd, BytesStart};

use crate::xml::{attribute, local_name, ElementBoundary, PartElements};
use crate::DocxError;

/// Word allows nine list levels, 0 through 8.
pub(crate) const MAX_LIST_LEVEL: u8 = 8;

#[derive(Debug, Default)]
pub(crate) struct Numbering {
    /// `w:numId` to `w:abstractNumId`.
    concrete_to_abstract: HashMap<String, String>,
    /// (`w:abstractNumId`, level) to "is an ordered list".
    ordered_levels: HashMap<(String, u8), bool>,
}

/// The `w:abstractNum`, `w:lvl` and `w:num` open around the element being
/// read. A numbering part states each fact once and expects the reader to
/// remember which definition it belongs to.
#[derive(Default)]
struct OpenDefinitions {
    abstract_id: Option<String>,
    level: Option<u8>,
    concrete_id: Option<String>,
}

impl OpenDefinitions {
    fn close(&mut self, element: &BytesEnd) {
        match element.local_name().as_ref() {
            "abstractNum" => self.abstract_id = None,
            "lvl" => self.level = None,
            "num" => self.concrete_id = None,
            _ => {}
        }
    }
}

impl Numbering {
    pub(crate) fn parse(xml: &str, part: &str) -> Result<Self, DocxError> {
        let mut elements = PartElements::new(xml, part, "numbering");
        let mut numbering = Self::default();
        let mut open = OpenDefinitions::default();

        while let Some(boundary) = elements.next_boundary()? {
            match boundary {
                ElementBoundary::Open(element) => numbering.read_element(&element, &mut open),
                ElementBoundary::Close(element) => open.close(&element),
            }
        }

        Ok(numbering)
    }

    /// Absorb one element of the numbering part.
    fn read_element(&mut self, element: &BytesStart, open: &mut OpenDefinitions) {
        match local_name(element).as_str() {
            "abstractNum" => open.abstract_id = attribute(element, "abstractNumId"),
            "num" => open.concrete_id = attribute(element, "numId"),
            "abstractNumId" => self.link_concrete_definition(element, open),
            "lvl" => {
                open.level = attribute(element, "ilvl").and_then(|value| value.parse::<u8>().ok());
            }
            "numFmt" => self.record_level_format(element, open),
            _ => {}
        }
    }

    /// Inside `w:num`, `w:abstractNumId` points at the abstract definition;
    /// the id is in `w:val`, not in an attribute of the same name.
    fn link_concrete_definition(&mut self, element: &BytesStart, open: &OpenDefinitions) {
        if let (Some(concrete), Some(value)) =
            (open.concrete_id.as_ref(), attribute(element, "val"))
        {
            self.concrete_to_abstract.insert(concrete.clone(), value);
        }
    }

    fn record_level_format(&mut self, element: &BytesStart, open: &OpenDefinitions) {
        if let (Some(abstract_num), Some(level), Some(format)) = (
            open.abstract_id.as_ref(),
            open.level,
            attribute(element, "val"),
        ) {
            let ordered = !matches!(format.as_str(), "bullet" | "none");
            self.ordered_levels
                .insert((abstract_num.clone(), level), ordered);
        }
    }

    /// Whether the list a paragraph belongs to is ordered.
    ///
    /// Unknown definitions fall back to a bullet list: an unmarked `<ul>` is a
    /// far less wrong guess than inventing numbers the document never had.
    pub(crate) fn is_ordered(&self, num_id: &str, level: u8) -> bool {
        let Some(abstract_num) = self.concrete_to_abstract.get(num_id) else {
            return false;
        };
        self.ordered_levels
            .get(&(abstract_num.clone(), level))
            .copied()
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NUMBERING: &str = r#"<w:numbering xmlns:w="x">
      <w:abstractNum w:abstractNumId="0">
        <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
        <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
      </w:abstractNum>
      <w:abstractNum w:abstractNumId="1">
        <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
        <w:lvl w:ilvl="1"><w:numFmt w:val="lowerLetter"/></w:lvl>
      </w:abstractNum>
      <w:num w:numId="3"><w:abstractNumId w:val="0"/></w:num>
      <w:num w:numId="4"><w:abstractNumId w:val="1"/></w:num>
    </w:numbering>"#;

    #[test]
    fn distinguishes_bullets_from_numbers() {
        let numbering = Numbering::parse(NUMBERING, "word/numbering.xml").unwrap();
        assert!(!numbering.is_ordered("3", 0));
        assert!(!numbering.is_ordered("3", 1));
        assert!(numbering.is_ordered("4", 0));
        assert!(numbering.is_ordered("4", 1));
    }

    #[test]
    fn unknown_definitions_fall_back_to_bullets() {
        let numbering = Numbering::default();
        assert!(!numbering.is_ordered("99", 0));
    }
}
