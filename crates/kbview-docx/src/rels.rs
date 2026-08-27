//! `_rels/*.rels` parts: relationship id to target resolution.

use std::collections::HashMap;

use quick_xml::events::BytesStart;

use crate::path::resolve_relative;
use crate::xml::{attribute, local_name, ElementBoundary, PartElements};
use crate::DocxError;

#[derive(Debug, Clone)]
pub(crate) struct Relationship {
    pub relationship_type: String,
    pub target: String,
    /// `TargetMode="External"`: the target is a URL, not a package part.
    pub external: bool,
}

/// The relationships declared by one part, plus the directory they resolve
/// against.
#[derive(Debug, Default)]
pub(crate) struct Relationships {
    by_id: HashMap<String, Relationship>,
    base_dir: String,
}

impl Relationships {
    /// An empty set, used when a part declares no relationships at all.
    pub(crate) fn empty(base_dir: &str) -> Self {
        Self {
            by_id: HashMap::new(),
            base_dir: base_dir.to_string(),
        }
    }

    pub(crate) fn parse(xml: &str, part: &str, base_dir: &str) -> Result<Self, DocxError> {
        let mut elements = PartElements::new(xml, part, "Relationships");
        let mut by_id = HashMap::new();

        while let Some(boundary) = elements.next_boundary()? {
            let ElementBoundary::Open(element) = boundary else {
                continue;
            };
            if let Some((id, relationship)) = read_relationship(&element) {
                by_id.insert(id, relationship);
            }
        }

        Ok(Self {
            by_id,
            base_dir: base_dir.to_string(),
        })
    }

    pub(crate) fn get(&self, rel_id: &str) -> Option<&Relationship> {
        self.by_id.get(rel_id)
    }

    /// The zip path a relationship points at, or `None` when it is external.
    pub(crate) fn internal_target(&self, rel_id: &str) -> Option<String> {
        let relationship = self.by_id.get(rel_id)?;
        if relationship.external {
            return None;
        }
        Some(resolve_relative(&self.base_dir, &relationship.target))
    }

    /// The first internal target whose relationship type ends with `suffix`,
    /// e.g. `styles` or `officeDocument`.
    ///
    /// Results are sorted by id so that a package declaring two of the same
    /// type still converts deterministically.
    pub(crate) fn find_by_type(&self, suffix: &str) -> Option<String> {
        let mut candidates: Vec<(&String, &Relationship)> = self
            .by_id
            .iter()
            .filter(|(_, relationship)| {
                !relationship.external && relationship.relationship_type.ends_with(suffix)
            })
            .collect();
        candidates.sort_by_key(|(id, _)| *id);
        candidates
            .first()
            .map(|(_, relationship)| resolve_relative(&self.base_dir, &relationship.target))
    }
}

/// Read one `<Relationship>`, or `None` for an element that is not one or
/// that is missing the id or target it is defined by.
fn read_relationship(element: &BytesStart) -> Option<(String, Relationship)> {
    if local_name(element) != "Relationship" {
        return None;
    }
    let id = attribute(element, "Id")?;
    let target = attribute(element, "Target")?;
    let external =
        attribute(element, "TargetMode").is_some_and(|mode| mode.eq_ignore_ascii_case("external"));
    Some((
        id,
        Relationship {
            relationship_type: attribute(element, "Type").unwrap_or_default(),
            target,
            external,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELS: &str = r#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/" TargetMode="External"/>
</Relationships>"#;

    #[test]
    fn resolves_internal_and_external_targets() {
        let relationships =
            Relationships::parse(RELS, "word/_rels/document.xml.rels", "word").unwrap();
        assert_eq!(
            relationships.internal_target("rId2").as_deref(),
            Some("word/media/image1.png")
        );
        assert_eq!(relationships.internal_target("rId3"), None);
        assert!(relationships.get("rId3").unwrap().external);
        assert_eq!(
            relationships.find_by_type("styles").as_deref(),
            Some("word/styles.xml")
        );
        assert_eq!(relationships.find_by_type("numbering"), None);
    }

    #[test]
    fn truncated_rels_are_an_error_not_a_panic() {
        let result = Relationships::parse("<Relationships><Relationship", "x.rels", "word");
        assert!(result.is_err());
    }
}
