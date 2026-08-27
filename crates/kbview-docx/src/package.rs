//! Zip access: locating and reading the parts of a `.docx` package.

use std::io::{Cursor, Read};

use zip::result::ZipError;
use zip::ZipArchive;

use crate::numbering::Numbering;
use crate::path::{parent_dir, rels_path_for, resolve_relative};
use crate::rels::Relationships;
use crate::styles::Styles;
use crate::DocxError;

/// Refuse to decompress a single part beyond this size. Guards against a zip
/// bomb, which would otherwise be an out-of-memory abort rather than an error.
const MAX_PART_BYTES: u64 = 64 * 1024 * 1024;

/// How much to reserve up front when reading a part.
const INITIAL_READ_CAPACITY: u64 = 1024 * 1024;

/// Where the main document part lives when the package does not say.
const DEFAULT_MAIN_PART: &str = "word/document.xml";
const PACKAGE_RELS: &str = "_rels/.rels";

pub(crate) struct Package<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
}

impl<'a> Package<'a> {
    pub(crate) fn open(bytes: &'a [u8]) -> Result<Self, DocxError> {
        let archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| DocxError::InvalidZip(error.to_string()))?;
        Ok(Self { archive })
    }

    /// Read a part's raw bytes.
    pub(crate) fn read_binary(&mut self, part: &str) -> Result<Vec<u8>, DocxError> {
        let mut file = match self.archive.by_name(part) {
            Ok(file) => file,
            Err(ZipError::FileNotFound) => return Err(DocxError::MissingPart(part.to_string())),
            Err(error) => return Err(DocxError::InvalidZip(error.to_string())),
        };

        if file.size() > MAX_PART_BYTES {
            return Err(DocxError::PartTooLarge(part.to_string()));
        }
        // The header's size is a claim, not a fact, so grow into it rather
        // than reserving what it asks for.
        let mut data = Vec::with_capacity(file.size().min(INITIAL_READ_CAPACITY) as usize);
        let read = file
            .by_ref()
            .take(MAX_PART_BYTES + 1)
            .read_to_end(&mut data)?;
        if read as u64 > MAX_PART_BYTES {
            return Err(DocxError::PartTooLarge(part.to_string()));
        }
        Ok(data)
    }

    /// Read a part as UTF-8 text, stripping a byte-order mark if present.
    pub(crate) fn read_text(&mut self, part: &str) -> Result<String, DocxError> {
        let part_bytes = self.read_binary(part)?;
        let text = String::from_utf8(part_bytes).map_err(|_| DocxError::MalformedXml {
            part: part.to_string(),
            message: "part is not valid UTF-8".to_string(),
        })?;
        Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
    }

    /// Read a part that is allowed to be absent.
    fn read_optional_text(&mut self, part: &str) -> Result<Option<String>, DocxError> {
        match self.read_text(part) {
            Ok(text) => Ok(Some(text)),
            Err(DocxError::MissingPart(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Locate the main document part.
    ///
    /// The package relationships name it, which matters for producers that do
    /// not use the conventional `word/document.xml`; that path is the
    /// fallback when the package relationships are missing or unhelpful.
    pub(crate) fn main_document_path(&mut self) -> Result<String, DocxError> {
        let declared = match self.read_optional_text(PACKAGE_RELS)? {
            Some(xml) => Relationships::parse(&xml, PACKAGE_RELS, "")
                .ok()
                .and_then(|relationships| relationships.find_by_type("officeDocument")),
            None => None,
        };

        let candidate = declared.unwrap_or_else(|| DEFAULT_MAIN_PART.to_string());
        if self.archive.by_name(&candidate).is_ok() {
            return Ok(candidate);
        }
        if candidate != DEFAULT_MAIN_PART && self.archive.by_name(DEFAULT_MAIN_PART).is_ok() {
            return Ok(DEFAULT_MAIN_PART.to_string());
        }
        Err(DocxError::MissingPart(DEFAULT_MAIN_PART.to_string()))
    }

    /// The relationships declared by a part; empty when it declares none.
    pub(crate) fn relationships_for(&mut self, part: &str) -> Result<Relationships, DocxError> {
        let base_dir = parent_dir(part).to_string();
        let rels_part = rels_path_for(part);
        match self.read_optional_text(&rels_part)? {
            Some(xml) => Relationships::parse(&xml, &rels_part, &base_dir),
            None => Ok(Relationships::empty(&base_dir)),
        }
    }

    pub(crate) fn styles(&mut self, relationships: &Relationships) -> Result<Styles, DocxError> {
        let part = relationships
            .find_by_type("styles")
            .unwrap_or_else(|| resolve_relative("word", "styles.xml"));
        // A styles part that will not parse costs headings, not the render.
        Ok(match self.read_optional_text(&part)? {
            Some(xml) => Styles::parse(&xml, &part).unwrap_or_default(),
            None => Styles::default(),
        })
    }

    pub(crate) fn numbering(
        &mut self,
        relationships: &Relationships,
    ) -> Result<Numbering, DocxError> {
        let part = relationships
            .find_by_type("numbering")
            .unwrap_or_else(|| resolve_relative("word", "numbering.xml"));
        // Likewise: unreadable numbering costs list markers, not the render.
        Ok(match self.read_optional_text(&part)? {
            Some(xml) => Numbering::parse(&xml, &part).unwrap_or_default(),
            None => Numbering::default(),
        })
    }
}
