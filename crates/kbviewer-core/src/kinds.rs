//! Mapping from a filename to how the app renders and displays it.

use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

/// How a document is rendered server-side and which viewer the client picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "types.ts")]
pub enum DocumentKind {
    /// Rendered through the comrak pipeline.
    Markdown,
    /// Converted from OOXML to HTML by `kbviewer-docx`.
    Docx,
    /// Served inline; the browser's own viewer displays it.
    Pdf,
    /// Served directly.
    Image,
    /// Parsed into a table, falling back to `Text` when malformed.
    Csv,
    /// Syntax-highlighted server-side.
    Text,
    /// Listed and downloadable, but not rendered.
    Binary,
}

impl DocumentKind {
    /// Whether the web editor may open this kind for editing.
    pub fn is_editable(self) -> bool {
        matches!(self, Self::Markdown | Self::Text | Self::Csv)
    }

    /// Whether the indexer should extract text from it for search.
    pub fn is_searchable(self) -> bool {
        matches!(self, Self::Markdown | Self::Text | Self::Csv | Self::Docx)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Docx => "docx",
            Self::Pdf => "pdf",
            Self::Image => "image",
            Self::Csv => "csv",
            Self::Text => "text",
            Self::Binary => "binary",
        }
    }
}

const MARKDOWN: &[&str] = &["md", "markdown", "mdown", "mkd"];
const IMAGE: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp", "heic", "heif",
];
const TEXT: &[&str] = &[
    "txt",
    "text",
    "log",
    "json",
    "jsonc",
    "yaml",
    "yml",
    "toml",
    "ini",
    "cfg",
    "conf",
    "env",
    "rs",
    "ts",
    "tsx",
    "js",
    "jsx",
    "mjs",
    "cjs",
    "py",
    "rb",
    "go",
    "java",
    "kt",
    "swift",
    "c",
    "h",
    "cpp",
    "hpp",
    "cc",
    "cs",
    "php",
    "sh",
    "bash",
    "zsh",
    "fish",
    "sql",
    "html",
    "htm",
    "css",
    "scss",
    "less",
    "xml",
    "gradle",
    "dockerfile",
    "makefile",
    "lock",
    "diff",
    "patch",
];

/// Files with no extension that are conventionally plain text.
const EXTENSIONLESS_TEXT: &[&str] = &[
    "readme",
    "license",
    "licence",
    "makefile",
    "dockerfile",
    "notice",
    "authors",
];

pub fn kind_for(path: &Path) -> DocumentKind {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if extension.is_empty() {
        let stem = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        return if EXTENSIONLESS_TEXT.contains(&stem.as_str()) {
            DocumentKind::Text
        } else {
            DocumentKind::Binary
        };
    }

    match extension.as_str() {
        e if MARKDOWN.contains(&e) => DocumentKind::Markdown,
        "docx" => DocumentKind::Docx,
        "pdf" => DocumentKind::Pdf,
        "csv" | "tsv" => DocumentKind::Csv,
        e if IMAGE.contains(&e) => DocumentKind::Image,
        e if TEXT.contains(&e) => DocumentKind::Text,
        _ => DocumentKind::Binary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(name: &str) -> DocumentKind {
        kind_for(Path::new(name))
    }

    #[test]
    fn maps_the_documented_extensions() {
        assert_eq!(kind("notes/a.md"), DocumentKind::Markdown);
        assert_eq!(kind("report.docx"), DocumentKind::Docx);
        assert_eq!(kind("paper.pdf"), DocumentKind::Pdf);
        assert_eq!(kind("photo.JPEG"), DocumentKind::Image);
        assert_eq!(kind("data.csv"), DocumentKind::Csv);
        assert_eq!(kind("main.rs"), DocumentKind::Text);
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert_eq!(kind("A.MD"), DocumentKind::Markdown);
        assert_eq!(kind("B.PdF"), DocumentKind::Pdf);
    }

    #[test]
    fn unknown_types_fall_back_to_download() {
        assert_eq!(kind("archive.zip"), DocumentKind::Binary);
        assert_eq!(kind("legacy.doc"), DocumentKind::Binary);
        assert_eq!(kind("sheet.xlsx"), DocumentKind::Binary);
    }

    #[test]
    fn recognises_conventional_extensionless_files() {
        assert_eq!(kind("README"), DocumentKind::Text);
        assert_eq!(kind("Makefile"), DocumentKind::Text);
        assert_eq!(kind("some-binary"), DocumentKind::Binary);
    }

    #[test]
    fn only_renderable_text_is_editable() {
        assert!(DocumentKind::Markdown.is_editable());
        assert!(!DocumentKind::Pdf.is_editable());
        assert!(!DocumentKind::Image.is_editable());
        assert!(!DocumentKind::Binary.is_editable());
    }

    #[test]
    fn docx_is_searchable_but_not_editable() {
        assert!(DocumentKind::Docx.is_searchable());
        assert!(!DocumentKind::Docx.is_editable());
    }
}
