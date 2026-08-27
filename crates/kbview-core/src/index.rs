//! The in-memory document index.
//!
//! The whole corpus is small enough to hold in RAM (the reference vault is ~1 MB of text
//! across ~100 files), so there is no database and no incremental update path: any change
//! on disk triggers a full rebuild, which costs tens of milliseconds. That trade buys a
//! large amount of absent complexity.

use crate::config::RootConfig;
use crate::frontmatter;
use crate::kinds::{kind_for, DocumentKind};
use crate::links::{scan_tags, scan_wikilinks, Resolver};
use crate::model::{DocumentMeta, FolderEntry, LinkRef, TreeNode};
use crate::paths::is_excluded;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct Document {
    pub path: String,
    pub name: String,
    pub title: String,
    pub kind: DocumentKind,
    pub size: u64,
    pub mtime_ms: i64,
    pub frontmatter: BTreeMap<String, String>,
    pub tags: Vec<String>,
    /// Source text for editable and searchable kinds; `None` for binary content.
    pub content: Option<String>,
    /// Paths this document links to, already resolved. Unresolved links are dropped
    /// here but still render as broken links, so the graph never contains dead paths.
    pub outlinks: Vec<String>,
}

impl Document {
    pub fn meta(&self) -> DocumentMeta {
        DocumentMeta {
            path: self.path.clone(),
            name: self.name.clone(),
            title: self.title.clone(),
            kind: self.kind,
            size: self.size,
            mtime_ms: self.mtime_ms,
            editable: self.kind.is_editable(),
            tags: self.tags.clone(),
        }
    }
}

pub struct Index {
    pub root_id: String,
    pub root_name: String,
    pub wikilinks: bool,
    pub documents: BTreeMap<String, Document>,
    pub resolver: Resolver,
    backlinks: HashMap<String, Vec<String>>,
    pub tags: BTreeMap<String, Vec<String>>,
    /// Every directory in the root, so empty folders still appear in the tree.
    directories: BTreeSet<String>,
}

impl Index {
    pub fn build(root: &RootConfig) -> Self {
        let wikilinks = root.uses_wikilinks();
        let (mut documents, directories) = scan_root(&root.path);

        let resolver = Resolver::new(documents.keys().cloned().collect::<Vec<_>>());

        for document in documents.values_mut() {
            let mut outlinks = markdown_outlinks(document, &resolver);
            if wikilinks {
                outlinks.extend(wikilink_outlinks(document, &resolver));
            }
            outlinks.sort();
            outlinks.dedup();
            document.outlinks = outlinks;
        }

        let backlinks = invert_outlinks(&documents);
        let tags = group_paths_by_tag(&documents);

        Self {
            root_id: root.id.clone(),
            root_name: root.name.clone(),
            wikilinks,
            documents,
            resolver,
            backlinks,
            tags,
            directories,
        }
    }

    pub fn get(&self, path: &str) -> Option<&Document> {
        self.documents.get(path)
    }

    pub fn backlinks(&self, path: &str) -> Vec<LinkRef> {
        self.backlinks
            .get(path)
            .map(|paths| paths.iter().filter_map(|p| self.link_ref(p)).collect())
            .unwrap_or_default()
    }

    pub fn outlinks(&self, path: &str) -> Vec<LinkRef> {
        self.documents
            .get(path)
            .map(|d| d.outlinks.iter().filter_map(|p| self.link_ref(p)).collect())
            .unwrap_or_default()
    }

    fn link_ref(&self, path: &str) -> Option<LinkRef> {
        self.documents.get(path).map(|d| LinkRef {
            path: d.path.clone(),
            title: d.title.clone(),
        })
    }

    pub fn has_directory(&self, path: &str) -> bool {
        path.is_empty() || self.directories.contains(path)
    }

    /// Children of one folder: subfolders first, then documents, each alphabetical.
    pub fn folder_entries(&self, folder: &str) -> Vec<FolderEntry> {
        let prefix = child_prefix(folder);
        let mut entries = self.subfolder_entries(&prefix);
        entries.extend(self.document_entries(&prefix));
        entries
    }

    fn subfolder_entries(&self, prefix: &str) -> Vec<FolderEntry> {
        let mut folders: Vec<FolderEntry> = self
            .directories
            .iter()
            .filter(|dir| is_direct_child(dir, prefix))
            .map(|dir| FolderEntry {
                name: dir[prefix.len()..].to_string(),
                path: dir.clone(),
                is_dir: true,
                kind: None,
                size: 0,
                mtime_ms: 0,
                child_count: Some(self.count_children(dir)),
            })
            .collect();
        folders.sort_by_key(|entry| entry.name.to_lowercase());
        folders
    }

    fn document_entries(&self, prefix: &str) -> Vec<FolderEntry> {
        let mut files: Vec<FolderEntry> = self
            .documents
            .values()
            .filter(|doc| is_direct_child(&doc.path, prefix))
            .map(|doc| FolderEntry {
                name: doc.name.clone(),
                path: doc.path.clone(),
                is_dir: false,
                kind: Some(doc.kind),
                size: doc.size,
                mtime_ms: doc.mtime_ms,
                child_count: None,
            })
            .collect();
        files.sort_by_key(|entry| entry.name.to_lowercase());
        files
    }

    fn count_children(&self, folder: &str) -> usize {
        let prefix = format!("{folder}/");
        self.documents
            .keys()
            .filter(|p| p.starts_with(&prefix))
            .count()
    }

    /// The whole tree, for the sidebar. Small corpora mean this ships in one response.
    pub fn tree(&self) -> Vec<TreeNode> {
        self.tree_for("")
    }

    fn tree_for(&self, folder: &str) -> Vec<TreeNode> {
        self.folder_entries(folder)
            .into_iter()
            .map(|entry| TreeNode {
                name: entry.name,
                path: entry.path.clone(),
                is_dir: entry.is_dir,
                kind: entry.kind,
                children: if entry.is_dir {
                    self.tree_for(&entry.path)
                } else {
                    Vec::new()
                },
            })
            .collect()
    }

    /// The landing page for a folder: the first configured index name that exists, or a
    /// note named after the folder when folder-notes are enabled.
    pub fn index_document(&self, folder: &str, root: &RootConfig) -> Option<String> {
        let prefix = child_prefix(folder);

        for name in &root.index_names {
            let candidate = format!("{prefix}{name}");
            if let Some(found) = self
                .documents
                .keys()
                .find(|p| p.to_lowercase() == candidate.to_lowercase())
            {
                return Some(found.clone());
            }
        }

        if root.folder_notes && !folder.is_empty() {
            let leaf = folder.rsplit('/').next().unwrap_or(folder);
            let candidate = format!("{prefix}{leaf}.md");
            if self.documents.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }
}

fn scan_root(root: &Path) -> (BTreeMap<String, Document>, BTreeSet<String>) {
    let mut documents = BTreeMap::new();
    let mut directories = BTreeSet::new();

    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0
                || entry
                    .path()
                    .strip_prefix(root)
                    .map(|rel| !is_excluded(rel))
                    .unwrap_or(false)
        });

    for entry in walker.flatten() {
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let path = relative.to_string_lossy().replace('\\', "/");

        if entry.file_type().is_dir() {
            directories.insert(path);
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(document) = read_document(entry.path(), path) {
            documents.insert(document.path.clone(), document);
        }
    }
    (documents, directories)
}

fn read_document(absolute: &Path, path: String) -> Option<Document> {
    let metadata = std::fs::metadata(absolute).ok()?;
    let kind = kind_for(Path::new(&path));
    let name = Path::new(&path).file_name()?.to_string_lossy().to_string();
    let content = read_indexable_text(absolute, kind);

    let (frontmatter_map, tags, title) = match (&content, kind) {
        (Some(text), DocumentKind::Markdown) => describe_markdown(text, &path),
        _ => (BTreeMap::new(), Vec::new(), stem_title(&path)),
    };

    Some(Document {
        path,
        name,
        title,
        kind,
        size: metadata.len(),
        mtime_ms: modified_millis(&metadata),
        frontmatter: frontmatter_map,
        tags,
        content,
        outlinks: Vec::new(),
    })
}

fn modified_millis(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|since_epoch| since_epoch.as_millis() as i64)
        .unwrap_or(0)
}

/// Only text-bearing kinds are read; a 40 MB PDF is indexed by metadata alone.
fn read_indexable_text(absolute: &Path, kind: DocumentKind) -> Option<String> {
    if kind.is_editable() || kind == DocumentKind::Markdown {
        return std::fs::read_to_string(absolute).ok();
    }
    if kind != DocumentKind::Docx {
        return None;
    }
    // Word documents are not editable here, but their text is extracted so search covers
    // them. The extraction shares the parser used to render them, so what is searchable
    // and what is displayed cannot disagree.
    std::fs::read(absolute)
        .ok()
        .and_then(|bytes| kbview_docx::convert(&bytes, "").ok())
        .map(|document| document.text)
}

fn describe_markdown(text: &str, path: &str) -> (BTreeMap<String, String>, Vec<String>, String) {
    let (raw_fm, body) = frontmatter::split(text);
    let parsed = raw_fm.map(frontmatter::parse).unwrap_or_default();

    let mut tags: Vec<String> = parsed.values("tags");
    tags.extend(parsed.values("tag"));
    for tag in scan_tags(body) {
        tags.push(tag.name);
    }
    tags.sort();
    tags.dedup();

    let title = parsed
        .get("title")
        .map(str::to_string)
        .filter(|t| !t.is_empty())
        .or_else(|| first_heading(body))
        .unwrap_or_else(|| stem_title(path));

    (parsed.scalars, tags, title)
}

fn first_heading(body: &str) -> Option<String> {
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let heading = rest.trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
        }
    }
    None
}

fn stem_title(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// The paths a note's relative markdown links point at, dropping any that resolve nowhere.
fn markdown_outlinks(document: &Document, resolver: &Resolver) -> Vec<String> {
    let Some(content) = linkable_markdown(document) else {
        return Vec::new();
    };
    markdown_link_urls(content)
        .into_iter()
        .filter_map(|url| resolver.resolve_relative(&document.path, &url))
        .collect()
}

/// The paths a note's `[[wikilinks]]` point at, for roots that read that syntax.
fn wikilink_outlinks(document: &Document, resolver: &Resolver) -> Vec<String> {
    let Some(content) = linkable_markdown(document) else {
        return Vec::new();
    };
    scan_wikilinks(content)
        .into_iter()
        .filter(|link| !link.target.is_empty())
        .filter_map(|link| resolver.resolve(&document.path, &link.target))
        .collect()
}

/// Only markdown carries links the index follows, and only if its text was read.
fn linkable_markdown(document: &Document) -> Option<&str> {
    if document.kind != DocumentKind::Markdown {
        return None;
    }
    document.content.as_deref()
}

/// Who links to whom, read backwards. A self-link is not a backlink.
fn invert_outlinks(documents: &BTreeMap<String, Document>) -> HashMap<String, Vec<String>> {
    let mut backlinks: HashMap<String, Vec<String>> = HashMap::new();
    for document in documents.values() {
        for target in &document.outlinks {
            if target == &document.path {
                continue;
            }
            let sources = backlinks.entry(target.clone()).or_default();
            if !sources.contains(&document.path) {
                sources.push(document.path.clone());
            }
        }
    }
    for sources in backlinks.values_mut() {
        sources.sort();
    }
    backlinks
}

fn group_paths_by_tag(documents: &BTreeMap<String, Document>) -> BTreeMap<String, Vec<String>> {
    let mut tags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for document in documents.values() {
        for tag in &document.tags {
            tags.entry(tag.clone())
                .or_default()
                .push(document.path.clone());
        }
    }
    tags
}

/// The prefix every direct child of `folder` starts with; empty for the root itself.
fn child_prefix(folder: &str) -> String {
    if folder.is_empty() {
        String::new()
    } else {
        format!("{folder}/")
    }
}

fn is_direct_child(path: &str, prefix: &str) -> bool {
    let Some(rest) = path.strip_prefix(prefix) else {
        return false;
    };
    !rest.is_empty() && !rest.contains('/')
}

/// Extract inline link and image destinations using the same parser that renders them,
/// so what the graph believes and what the reader sees cannot drift apart.
pub fn markdown_link_urls(source: &str) -> Vec<String> {
    let arena = comrak::Arena::new();
    let options = comrak::Options::default();
    let root = comrak::parse_document(&arena, source, &options);

    let mut urls = Vec::new();
    collect_urls(root, &mut urls);
    urls
}

fn collect_urls<'a>(node: &'a comrak::nodes::AstNode<'a>, out: &mut Vec<String>) {
    match &node.data.borrow().value {
        comrak::nodes::NodeValue::Link(link) | comrak::nodes::NodeValue::Image(link) => {
            out.push(link.url.clone());
        }
        _ => {}
    }
    for child in node.children() {
        collect_urls(child, out);
    }
}
