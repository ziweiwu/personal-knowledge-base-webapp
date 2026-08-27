//! Convert Microsoft Word `.docx` (OOXML) packages into clean, semantic HTML
//! plus a plain-text rendition suitable for a search index.
//!
//! The converter is deliberately forgiving: any OOXML construct it does not
//! understand degrades to the text it contains rather than aborting the
//! render. The only conditions that produce an [`Err`] are a package that
//! cannot be opened, a missing main document part, and XML that is not
//! well formed.
#![forbid(unsafe_code)]

mod document;
mod escape;
mod model;
mod numbering;
mod package;
mod path;
mod rels;
mod render;
mod styles;
mod xml;

use package::Package;

/// A converted Word document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocxDocument {
    /// Semantic HTML fragment. Not a full page: no `<html>`, `<head>` or
    /// `<body>` wrapper, and no inline styles or class attributes.
    pub html: String,
    /// Plain text rendition, one line per paragraph, for a search index.
    pub text: String,
    /// Every embedded image the HTML actually references, in first-use order.
    pub media: Vec<MediaRef>,
}

/// An embedded media part referenced by the converted HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRef {
    /// OOXML relationship id, e.g. `rId7`. This is what the `<img src>` ends
    /// with, and what [`extract_media`] takes.
    pub rel_id: String,
    /// Path of the part inside the zip, e.g. `word/media/image1.png`.
    pub zip_path: String,
    /// Media type guessed from the part's file extension.
    pub mime: String,
}

/// Everything that can go wrong reading a `.docx`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DocxError {
    /// The bytes are not a readable zip archive.
    #[error("not a valid .docx package: {0}")]
    InvalidZip(String),
    /// A part the caller asked for is not in the package.
    #[error("missing part: {0}")]
    MissingPart(String),
    /// A part is present but its XML is not well formed.
    #[error("malformed XML in {part}: {message}")]
    MalformedXml {
        /// Zip path of the offending part.
        part: String,
        /// Human-readable description of the problem.
        message: String,
    },
    /// A part is large enough that decompressing it is refused.
    #[error("part {0} is too large to decompress")]
    PartTooLarge(String),
    /// Reading the underlying bytes failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convert a `.docx` package into HTML and plain text.
///
/// `media_base` is a URL prefix owned by the caller; every embedded image
/// becomes `<img src="{media_base}/{rel_id}">`. The relationship ids that
/// appear there are listed in [`DocxDocument::media`], and the bytes behind
/// each one can be fetched with [`extract_media`].
pub fn convert(bytes: &[u8], media_base: &str) -> Result<DocxDocument, DocxError> {
    let mut package = Package::open(bytes)?;
    let main_part = package.main_document_path()?;
    let document_xml = package.read_text(&main_part)?;
    let relationships = package.relationships_for(&main_part)?;
    let styles = package.styles(&relationships)?;
    let numbering = package.numbering(&relationships)?;

    let context = document::DocumentContext {
        styles: &styles,
        numbering: &numbering,
        relationships: &relationships,
    };
    let blocks = document::parse(&document_xml, &main_part, &context)?;
    let rendered = render::render(&blocks, media_base, &relationships);

    Ok(DocxDocument {
        html: rendered.html,
        text: rendered.text,
        media: rendered.media,
    })
}

/// Extract one embedded media part on demand, by the relationship id that
/// [`convert`] put into the HTML.
pub fn extract_media(bytes: &[u8], rel_id: &str) -> Result<(Vec<u8>, String), DocxError> {
    let mut package = Package::open(bytes)?;
    let main_part = package.main_document_path()?;
    let relationships = package.relationships_for(&main_part)?;

    let zip_path = relationships
        .internal_target(rel_id)
        .ok_or_else(|| DocxError::MissingPart(format!("relationship {rel_id}")))?;
    let media_bytes = package.read_binary(&zip_path)?;
    let mime = path::mime_for_path(&zip_path).to_string();
    Ok((media_bytes, mime))
}
