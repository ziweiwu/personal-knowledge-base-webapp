//! Turns an indexed document into the payload the client renders.
//!
//! One place decides, per kind, whether the server produces HTML or the browser handles
//! the bytes itself. Adding a format means adding an arm here and a viewer on the client.

use super::cache::{CachedRender, RenderKey};
use super::markdown::{self, RenderContext};
use super::text;
use crate::state::AppState;
use kbview_core::index::{Document, Index};
use kbview_core::kinds::DocumentKind;
use kbview_core::model::DocumentPayload;
use std::collections::BTreeMap;

pub fn build_payload(
    state: &AppState,
    root_id: &str,
    index: &Index,
    document: &Document,
) -> DocumentPayload {
    let Some(root) = state.root(root_id) else {
        return DocumentPayload {
            meta: document.meta(),
            html: None,
            frontmatter: BTreeMap::new(),
            headings: Vec::new(),
            backlinks: Vec::new(),
            outlinks: Vec::new(),
            render_warning: Some("this folder is no longer configured".into()),
        };
    };
    let key = RenderKey::new(root_id, &document.path, document.mtime_ms);
    let cached = state.renders.get(&key).unwrap_or_else(|| {
        let rendered = render_kind(root, root_id, index, document);
        state.renders.put(key.clone(), rendered.clone());
        rendered
    });

    let frontmatter: BTreeMap<String, String> = document.frontmatter.clone();

    DocumentPayload {
        meta: document.meta(),
        html: cached.html_option(),
        frontmatter,
        headings: cached.headings,
        backlinks: index.backlinks(&document.path),
        outlinks: index.outlinks(&document.path),
        render_warning: cached.warning,
    }
}

impl CachedRender {
    /// An empty body means "the browser displays this itself" (image, pdf) or "there is
    /// nothing to show" (binary) — not an empty document.
    fn html_option(&self) -> Option<String> {
        (!self.html.is_empty()).then(|| self.html.clone())
    }
}

/// Convert a Word document, keeping the failure visible rather than showing a blank pane.
fn render_docx(
    root: &kbview_core::config::RootConfig,
    root_id: &str,
    document: &Document,
) -> CachedRender {
    let media_base = format!(
        "/api/docx-media/{}/{}",
        root_id,
        super::html::encode_path(&document.path)
    );

    let Ok(absolute) = kbview_core::paths::resolve_in_root(&root.path, &document.path) else {
        return CachedRender {
            html: String::new(),
            headings: Vec::new(),
            warning: Some("could not locate this document".into()),
        };
    };

    match std::fs::read(&absolute)
        .map_err(|e| e.to_string())
        .and_then(|bytes| kbview_docx::convert(&bytes, &media_base).map_err(|e| e.to_string()))
    {
        Ok(converted) => CachedRender {
            html: format!("<div class=\"docx\">{}</div>", converted.html),
            headings: Vec::new(),
            warning: None,
        },
        Err(error) => CachedRender {
            html: String::new(),
            headings: Vec::new(),
            warning: Some(format!("could not convert this Word document: {error}")),
        },
    }
}

fn render_kind(
    root: &kbview_core::config::RootConfig,
    root_id: &str,
    index: &Index,
    document: &Document,
) -> CachedRender {
    match document.kind {
        DocumentKind::Markdown => render_markdown(root_id, index, document),
        DocumentKind::Csv => render_table(document),
        DocumentKind::Text => render_plain_text(document),
        DocumentKind::Docx => render_docx(root, root_id, document),
        // The browser renders these from their bytes; the client fetches /api/file.
        DocumentKind::Pdf | DocumentKind::Image | DocumentKind::Binary => CachedRender {
            html: String::new(),
            headings: Vec::new(),
            warning: None,
        },
    }
}

fn render_markdown(root_id: &str, index: &Index, document: &Document) -> CachedRender {
    let source = document.content.as_deref().unwrap_or_default();
    let (_, body) = kbview_core::frontmatter::split(source);
    let context = RenderContext {
        root_id,
        path: &document.path,
        index,
    };
    let rendered = markdown::render(body, &context);
    CachedRender {
        html: rendered.html,
        headings: rendered.headings,
        warning: rendered.warning,
    }
}

fn render_table(document: &Document) -> CachedRender {
    let source = document.content.as_deref().unwrap_or_default();
    let delimiter = if document.path.ends_with(".tsv") {
        '\t'
    } else {
        ','
    };
    match text::render_csv(source, delimiter) {
        Some(html) => CachedRender {
            html,
            headings: Vec::new(),
            warning: None,
        },
        // Malformed data is shown as text rather than as a broken table, and the reader is
        // told why it is not a table.
        None => CachedRender {
            html: text::render_text(source, &document.path),
            headings: Vec::new(),
            warning: Some("could not parse as a table; showing raw text".into()),
        },
    }
}

fn render_plain_text(document: &Document) -> CachedRender {
    CachedRender {
        html: text::render_text(
            document.content.as_deref().unwrap_or_default(),
            &document.path,
        ),
        headings: Vec::new(),
        warning: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kbview_core::config::RootConfig;

    /// Whether the folder under test treats `[[...]]` as a link or as literal text.
    #[derive(Clone, Copy, PartialEq)]
    enum Wikilinks {
        Enabled,
        Disabled,
    }

    fn build_index(
        label: &str,
        files: &[(&str, &str)],
        wikilinks: Wikilinks,
    ) -> (Index, RootConfig) {
        let root = std::env::temp_dir().join(format!("kbview-render-{label}"));
        let _ = std::fs::remove_dir_all(&root);
        for (path, body) in files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, body).unwrap();
        }
        let config = RootConfig {
            id: "kb".into(),
            name: "kb".into(),
            path: root,
            index_names: vec!["index.md".into()],
            wikilinks: Some(wikilinks == Wikilinks::Enabled),
            folder_notes: false,
            read_only: false,
        };
        (Index::build(&config), config)
    }

    fn render_body(
        label: &str,
        files: &[(&str, &str)],
        target: &str,
        wikilinks: Wikilinks,
    ) -> String {
        let (index, _) = build_index(label, files, wikilinks);
        let document = index
            .get(target)
            .expect("target document should be indexed");
        let source = document.content.as_deref().unwrap_or_default();
        let (_, body) = kbview_core::frontmatter::split(source);
        let context = RenderContext {
            root_id: "kb",
            path: target,
            index: &index,
        };
        markdown::render(body, &context).html
    }

    #[test]
    fn renders_a_resolved_wikilink_as_an_app_link() {
        let html = render_body(
            "wikilink",
            &[("a.md", "see [[Target]]\n"), ("Target.md", "# Target\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(html.contains("href=\"/n/kb/Target.md\""), "got {html}");
        assert!(html.contains("class=\"wikilink\""));
    }

    #[test]
    fn marks_an_unresolved_wikilink_instead_of_linking_it() {
        let html = render_body(
            "unresolved",
            &[("a.md", "see [[Nope]]\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(html.contains("wikilink-unresolved"), "got {html}");
        assert!(
            !html.contains("href=\"/n/kb/Nope"),
            "must not link to a document that does not exist"
        );
    }

    #[test]
    fn renders_an_image_embed_as_an_img_tag() {
        let html = render_body(
            "embed",
            &[("a.md", "![[pic.png]]\n"), ("pic.png", "not really a png")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(html.contains("<img"), "got {html}");
        assert!(html.contains("/api/file/kb/pic.png"));
    }

    #[test]
    fn transcludes_an_embedded_note() {
        let html = render_body(
            "transclude",
            &[
                ("a.md", "![[Other]]\n"),
                ("Other.md", "# Other\nInner content here.\n"),
            ],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(
            html.contains("Inner content here."),
            "embedded note body should appear: {html}"
        );
        assert!(html.contains("embed-note"));
    }

    #[test]
    fn a_transclusion_cycle_terminates() {
        let html = render_body(
            "cycle",
            &[("a.md", "![[b]]\n"), ("b.md", "![[a]]\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        // The guard is that this returns at all; depth-limited output is fine.
        assert!(!html.is_empty());
    }

    #[test]
    fn wikilinks_are_inert_in_a_plain_folder() {
        let html = render_body(
            "plain",
            &[("a.md", "see [[Target]]\n"), ("Target.md", "# Target\n")],
            "a.md",
            Wikilinks::Disabled,
        );
        assert!(
            !html.contains("class=\"wikilink\""),
            "plain folders have no wikilinks: {html}"
        );
        assert!(
            html.contains("[[Target]]"),
            "the literal text should survive"
        );
    }

    #[test]
    fn relative_links_are_rewritten_in_a_plain_folder() {
        let html = render_body(
            "relative",
            &[("docs/a.md", "[other](./b.md)\n"), ("docs/b.md", "# B\n")],
            "docs/a.md",
            Wikilinks::Disabled,
        );
        assert!(html.contains("href=\"/n/kb/docs/b.md\""), "got {html}");
    }

    #[test]
    fn a_relative_image_points_at_the_file_route() {
        let html = render_body(
            "relimage",
            &[
                ("docs/a.md", "![pic](../assets/p.png)\n"),
                ("assets/p.png", "x"),
            ],
            "docs/a.md",
            Wikilinks::Disabled,
        );
        assert!(
            html.contains("src=\"/api/file/kb/assets/p.png\""),
            "got {html}"
        );
    }

    #[test]
    fn callouts_render_as_styled_blocks() {
        let html = render_body(
            "callout",
            &[("a.md", "> [!warning] Careful\n> body\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(html.contains("data-callout=\"warning\""), "got {html}");
        assert!(html.contains("Careful"));
    }

    #[test]
    fn mermaid_fences_are_left_for_the_client() {
        let html = render_body(
            "mermaid",
            &[("a.md", "```mermaid\ngraph TD;\nA-->B;\n```\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(html.contains("<pre class=\"mermaid\">"), "got {html}");
        assert!(
            html.contains("A--&gt;B"),
            "diagram source must be escaped but present"
        );
    }

    #[test]
    fn maths_becomes_mathml_with_no_client_library() {
        let html = render_body(
            "math",
            &[("a.md", "inline $x^2$ here\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(html.contains("<math"), "expected MathML: {html}");
    }

    #[test]
    fn inline_tags_become_links() {
        let html = render_body(
            "tags",
            &[("a.md", "about #rust today\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(html.contains("class=\"tag\""), "got {html}");
        assert!(html.contains("/t/kb/rust"));
    }

    #[test]
    fn headings_are_collected_with_slugs() {
        let (index, _) = build_index(
            "headings",
            &[("a.md", "# One\n## Two Words\n")],
            Wikilinks::Enabled,
        );
        let document = index.get("a.md").unwrap();
        let (_, body) = kbview_core::frontmatter::split(document.content.as_deref().unwrap());
        let context = RenderContext {
            root_id: "kb",
            path: "a.md",
            index: &index,
        };
        let rendered = markdown::render(body, &context);
        assert_eq!(rendered.headings.len(), 2);
        assert_eq!(rendered.headings[1].slug, "two-words");
        assert_eq!(rendered.headings[1].depth, 2);
    }

    #[test]
    fn frontmatter_is_not_rendered_into_the_body() {
        let html = render_body(
            "fm",
            &[("a.md", "---\ntitle: Hidden\n---\n# Shown\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(
            !html.contains("Hidden"),
            "frontmatter must not appear as content: {html}"
        );
        assert!(html.contains("Shown"));
    }

    #[test]
    fn wikilinks_inside_code_blocks_are_not_linked() {
        let html = render_body(
            "codelink",
            &[("a.md", "```\n[[Target]]\n```\n"), ("Target.md", "# T\n")],
            "a.md",
            Wikilinks::Enabled,
        );
        assert!(
            !html.contains("class=\"wikilink\""),
            "a code sample must not become a link: {html}"
        );
    }
}
