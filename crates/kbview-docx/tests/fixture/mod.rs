#![allow(dead_code)]
//! Builds real `.docx` packages from XML strings.
//!
//! A `.docx` is a zip of XML parts, so a fixture is cheaper to write, read
//! and adjust as source than as a checked-in binary.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Namespace declarations every fixture document carries, so the XML looks
/// like what Word actually emits.
pub const NAMESPACES: &str = concat!(
    r#" xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#,
    r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#,
    r#" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#,
    r#" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing""#,
    r#" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006""#,
    r#" xmlns:v="urn:schemas-microsoft-com:vml""#,
    r#" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#,
);

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

pub struct DocxBuilder {
    parts: Vec<(String, Vec<u8>)>,
}

impl DocxBuilder {
    /// A package with the boilerplate parts and nothing else.
    pub fn new() -> Self {
        Self {
            parts: vec![
                ("[Content_Types].xml".to_string(), CONTENT_TYPES.into()),
                ("_rels/.rels".to_string(), PACKAGE_RELS.into()),
            ],
        }
    }

    /// A package with no parts at all, for testing what happens when the main
    /// document is missing.
    pub fn bare() -> Self {
        Self { parts: Vec::new() }
    }

    pub fn part(mut self, name: &str, content: &str) -> Self {
        self.parts.push((name.to_string(), content.into()));
        self
    }

    pub fn binary(mut self, name: &str, contents: &[u8]) -> Self {
        self.parts.push((name.to_string(), contents.to_vec()));
        self
    }

    /// Wrap `body` in a `w:document`/`w:body` and add it as the main part.
    pub fn body(self, body: &str) -> Self {
        let document = format!("<w:document{NAMESPACES}><w:body>{body}</w:body></w:document>");
        self.part("word/document.xml", &document)
    }

    /// Add `word/_rels/document.xml.rels` from bare `<Relationship>` elements.
    pub fn document_rels(self, relationships: &str) -> Self {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
        );
        self.part("word/_rels/document.xml.rels", &xml)
    }

    pub fn build(self) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (name, content) in self.parts {
            writer.start_file(name, options).expect("start zip entry");
            writer.write_all(&content).expect("write zip entry");
        }
        writer.finish().expect("finish zip").into_inner()
    }
}

/// A paragraph made of one plain run.
pub fn paragraph(text: &str) -> String {
    format!("<w:p><w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>")
}

/// A paragraph carrying a `pStyle`.
pub fn styled_paragraph(style_id: &str, text: &str) -> String {
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"{style_id}\"/></w:pPr>\
         <w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>"
    )
}

/// A paragraph that is a list item at `level` of list `num_id`.
pub fn list_paragraph(num_id: &str, level: u8, text: &str) -> String {
    format!(
        "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>\
         <w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>"
    )
}

/// A numbering part with bullets on `numId` 1 and decimals on `numId` 2.
pub const NUMBERING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="10">
    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
    <w:lvl w:ilvl="2"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:abstractNum w:abstractNumId="20">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="lowerLetter"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="10"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="20"/></w:num>
</w:numbering>"#;

/// A styles part using Word's `Heading1` id / `heading 1` name convention.
pub const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style>
  <w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/></w:style>
  <w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/></w:style>
  <w:style w:type="paragraph" w:styleId="Titre4"><w:name w:val="heading 4"/></w:style>
  <w:style w:type="character" w:styleId="Heading1Char"><w:name w:val="Heading 1 Char"/></w:style>
  <w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/></w:style>
</w:styles>"#;

/// The eight bytes that open every PNG, enough to stand in for an image.
pub const PNG_BYTES: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
