//! Thin helpers over `quick-xml`: a reader that reports errors in this
//! crate's terms, plus namespace-agnostic name and attribute lookups.

use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::DocxError;

/// How deep element recursion is allowed to go.
///
/// Real OOXML is nowhere near this deep. The limit exists so that a file with
/// pathological nesting exhausts the limit instead of the call stack -- a
/// stack overflow aborts the process, which would break the crate's promise
/// never to die on bad input.
pub(crate) const MAX_DEPTH: usize = 100;

/// A pull parser over one package part.
pub(crate) struct XmlReader<'a> {
    reader: Reader<&'a [u8]>,
    part: String,
}

impl<'a> XmlReader<'a> {
    pub(crate) fn new(text: &'a str, part: &str) -> Self {
        let mut reader = Reader::from_str(text);
        let config = reader.config_mut();
        config.check_end_names = true;
        config.expand_empty_elements = false;
        Self {
            reader,
            part: part.to_string(),
        }
    }

    /// Pull the next event, translating parse failures into [`DocxError`].
    pub(crate) fn next(&mut self) -> Result<Event<'a>, DocxError> {
        self.reader
            .read_event()
            .map_err(|error| DocxError::MalformedXml {
                part: self.part.clone(),
                message: error.to_string(),
            })
    }

    /// The error for input that ends while `container` is still open.
    pub(crate) fn unexpected_eof(&self, container: &str) -> DocxError {
        DocxError::MalformedXml {
            part: self.part.clone(),
            message: format!("input ended inside <{container}>"),
        }
    }

    /// Walk the elements inside the container whose `Start` event was just
    /// read, stopping at its end tag.
    pub(crate) fn scan<'s>(&'s mut self, container: &'s str) -> ElementScan<'s, 'a> {
        ElementScan {
            reader: self,
            container,
            depth: 1,
        }
    }

    /// Consume events up to and including the end tag matching the `Start`
    /// event that was just read.
    ///
    /// Depth is counted rather than name-matched, which is sound because the
    /// reader is configured to reject mismatched end tags.
    pub(crate) fn skip_element(&mut self, container: &str) -> Result<(), DocxError> {
        let mut depth = 1usize;
        loop {
            match self.next()? {
                Event::Start(_) => depth += 1,
                Event::End(_) if depth <= 1 => return Ok(()),
                Event::End(_) => depth -= 1,
                Event::Eof => return Err(self.unexpected_eof(container)),
                _ => {}
            }
        }
    }
}

/// A flat walk over the elements inside one container element.
pub(crate) struct ElementScan<'s, 'x> {
    reader: &'s mut XmlReader<'x>,
    container: &'s str,
    depth: usize,
}

impl<'x> ElementScan<'_, 'x> {
    /// The next element inside the container, or `None` at its end tag.
    ///
    /// Nested elements are yielded flattened, which is what OOXML property
    /// bags such as `w:pPr`, `w:rPr` and `w:tcPr` want: their meaning is in
    /// the element names they contain, not in how those are grouped.
    pub(crate) fn next_element(&mut self) -> Result<Option<BytesStart<'x>>, DocxError> {
        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.depth += 1;
                    return Ok(Some(element));
                }
                Event::Empty(element) => return Ok(Some(element)),
                Event::End(_) if self.depth <= 1 => return Ok(None),
                Event::End(_) => self.depth -= 1,
                Event::Eof => return Err(self.reader.unexpected_eof(self.container)),
                _ => {}
            }
        }
    }
}

/// One element boundary from a flat walk over a whole part.
pub(crate) enum ElementBoundary<'x> {
    /// A `<w:x>` or a self-closing `<w:x/>`, with its attributes.
    Open(BytesStart<'x>),
    /// A `</w:x>`.
    Close(BytesEnd<'x>),
}

/// A flat walk over every element of one part, ignoring the tree shape.
///
/// The parts read outside the document body -- styles, numbering,
/// relationships -- are shallow bags of elements whose meaning comes from the
/// element name and the definition currently open, not from the nesting. This
/// keeps the truncated-input check in one place instead of in each of them.
pub(crate) struct PartElements<'x> {
    reader: XmlReader<'x>,
    root: String,
    depth: usize,
}

impl<'x> PartElements<'x> {
    pub(crate) fn new(xml: &'x str, part: &str, root: &str) -> Self {
        Self {
            reader: XmlReader::new(xml, part),
            root: root.to_string(),
            depth: 0,
        }
    }

    /// The next boundary, or `None` once the input ends with everything it
    /// opened closed again. Input that stops mid-element is an error.
    pub(crate) fn next_boundary(&mut self) -> Result<Option<ElementBoundary<'x>>, DocxError> {
        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.depth += 1;
                    return Ok(Some(ElementBoundary::Open(element)));
                }
                Event::Empty(element) => return Ok(Some(ElementBoundary::Open(element))),
                Event::End(element) => {
                    self.depth = self.depth.saturating_sub(1);
                    return Ok(Some(ElementBoundary::Close(element)));
                }
                Event::Eof if self.depth > 0 => return Err(self.reader.unexpected_eof(&self.root)),
                Event::Eof => return Ok(None),
                _ => {}
            }
        }
    }
}

/// The local part of an element name, ignoring its namespace prefix.
pub(crate) fn local_name(element: &BytesStart) -> String {
    element.local_name().as_ref().to_string()
}

/// Look up an attribute by local name, ignoring its namespace prefix.
pub(crate) fn attribute(element: &BytesStart, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.local_name().as_ref() == wanted)
            .then(|| attribute_value(&attribute))
            .flatten()
    })
}

/// Look up an attribute that belongs to the relationships namespace, such as
/// `r:id` or `r:embed`.
///
/// Matching on the local name alone would collide with `w:id`, and matching
/// on the literal prefix `r:` would break for producers that bind the
/// namespace to a different prefix, so this accepts any prefix except `w`.
pub(crate) fn relationship_attribute(element: &BytesStart, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        let is_word_prefix = attribute
            .key
            .prefix()
            .is_some_and(|prefix| prefix.as_ref() == "w");
        let matches = attribute.key.local_name().as_ref() == wanted;
        (matches && !is_word_prefix)
            .then(|| attribute_value(&attribute))
            .flatten()
    })
}

/// The attribute's value with entity references resolved. A value this crate
/// cannot decode is treated as absent rather than fatal.
fn attribute_value(attribute: &quick_xml::events::attributes::Attribute) -> Option<String> {
    attribute
        .normalized_value(XmlVersion::Implicit1_0)
        .ok()
        .map(|value| value.into_owned())
}

/// Read the `w:val` of an OOXML on/off element such as `<w:b/>`.
///
/// An absent value means on; `0`, `false` and `off` mean off.
pub(crate) fn on_off_value(element: &BytesStart) -> bool {
    match attribute(element, "val") {
        None => true,
        Some(value) => !matches!(value.as_str(), "0" | "false" | "off"),
    }
}

/// Resolve an entity reference event -- `quick-xml` reports `&amp;` and
/// `&#38;` separately from surrounding text -- into the text it stands for.
///
/// Entities this crate cannot resolve are dropped rather than surfaced as an
/// error, in keeping with the degrade-don't-fail rule.
pub(crate) fn resolve_entity(reference: &str) -> Option<String> {
    let source = format!("&{reference};");
    quick_xml::escape::unescape(&source)
        .ok()
        .map(|resolved| resolved.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_predefined_and_numeric_entities() {
        assert_eq!(resolve_entity("amp").as_deref(), Some("&"));
        assert_eq!(resolve_entity("#60").as_deref(), Some("<"));
        assert_eq!(resolve_entity("nosuchentity"), None);
    }

    #[test]
    fn skip_element_reports_truncated_input() {
        let mut reader = XmlReader::new("<a><b>", "test.xml");
        assert!(matches!(reader.next(), Ok(Event::Start(_))));
        assert!(matches!(
            reader.skip_element("a"),
            Err(DocxError::MalformedXml { .. })
        ));
    }
}
