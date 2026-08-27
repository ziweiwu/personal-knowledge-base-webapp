//! Every type that crosses the wire.
//!
//! These derive `TS`, so `cargo test` regenerates `web/src/api/types.ts`. The frontend
//! never hand-writes an API type — if a field changes here and the bindings are not
//! regenerated, CI fails on the diff.

use crate::kinds::DocumentKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct RootInfo {
    pub id: String,
    pub name: String,
    /// True when wikilinks, backlinks, callouts and tags are active for this root.
    pub obsidian_mode: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct Heading {
    pub depth: u8,
    pub text: String,
    pub slug: String,
}

/// A reference from one document to another, used for both backlinks and outlinks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct LinkRef {
    pub path: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct DocumentMeta {
    pub path: String,
    pub name: String,
    pub title: String,
    pub kind: DocumentKind,
    #[ts(type = "number")]
    pub size: u64,
    /// Milliseconds since the epoch. Doubles as the optimistic-concurrency token:
    /// a save must echo the value it read, or it is rejected as a conflict.
    #[ts(type = "number")]
    pub mtime_ms: i64,
    pub editable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct DocumentPayload {
    pub meta: DocumentMeta,
    /// Rendered HTML. `None` for kinds the browser displays itself (image, pdf) or
    /// cannot display at all (binary).
    pub html: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frontmatter: BTreeMap<String, String>,
    pub headings: Vec<Heading>,
    pub backlinks: Vec<LinkRef>,
    pub outlinks: Vec<LinkRef>,
    /// Set when a renderer degraded — the document still displays, but something was
    /// skipped and the UI should say so rather than pretend the render was complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind: Option<DocumentKind>,
    #[ts(type = "number")]
    pub size: u64,
    #[ts(type = "number")]
    pub mtime_ms: i64,
    /// Number of documents inside, for directories.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub child_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct FolderListing {
    pub path: String,
    pub name: String,
    /// The folder's landing page, when an index file was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub index: Option<DocumentPayload>,
    /// Children, with the index file itself omitted so it is not listed below its own content.
    pub entries: Vec<FolderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind: Option<DocumentKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub kind: DocumentKind,
    /// Matched text with surrounding context; `**` marks the match for the UI to style.
    pub snippet: String,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct SaveRequest {
    pub content: String,
    /// The `mtimeMs` the client last read. A mismatch means someone else wrote the file.
    #[ts(type = "number")]
    pub base_mtime_ms: i64,
}

/// Returned with HTTP 409 when a save would overwrite a change made elsewhere —
/// typically Obsidian editing the same note. Both sides are included so the UI can
/// offer a real choice instead of silently losing one of them.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct SaveConflict {
    pub path: String,
    pub your_content: String,
    pub disk_content: String,
    #[ts(type = "number")]
    pub disk_mtime_ms: i64,
}

/// Tick or untick one task-list checkbox.
///
/// Deliberately not a content write: the request names a line and the state it wants, so
/// the narrowest possible edit is the only one this route can make.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct TaskToggleRequest {
    /// 1-based line in the markdown source, as rendered into `data-task-line`.
    #[ts(type = "number")]
    pub line: usize,
    pub checked: bool,
    /// The `mtimeMs` the client last read, exactly as a save carries it.
    #[ts(type = "number")]
    pub base_mtime_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct RenameRequest {
    pub from: String,
    pub to: String,
    /// Rewrite inbound links so the rename does not break references.
    #[serde(default = "default_true")]
    pub update_links: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct RenameResult {
    pub from: String,
    pub to: String,
    /// Documents whose links were rewritten as part of the move.
    pub updated: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct SessionInfo {
    pub email: String,
}

/// Pushed over SSE when the watcher sees the folder change on disk.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct ChangeEvent {
    pub root_id: String,
    pub paths: Vec<String>,
    /// Set when this app made the change, so the client that made it can ignore the echo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "types.ts")]
pub struct ApiError {
    pub error: String,
    pub message: String,
}
