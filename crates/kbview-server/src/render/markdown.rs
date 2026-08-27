//! Markdown rendering.
//!
//! Obsidian-dialect syntax (wikilinks, embeds, callouts, tags) is rewritten in the source
//! before parsing, using the code-aware scanners in `kbview-core`. Standard syntax is left
//! entirely to comrak. The split matters: the scanners already know how to avoid code
//! blocks, and reusing them here means what the link graph believes and what the reader
//! sees come from the same logic.

use super::callouts;
use super::html::{encode_path, escape_attr, escape_text};
use kbview_core::index::Index;
use kbview_core::kinds::DocumentKind;
use kbview_core::links::{escape_stray_dollars, scan_tags, scan_wikilinks, WikiLink};
use kbview_core::model::Heading;
use std::path::Path;

/// How deep `![[Note]]` transclusion may nest before it stops expanding.
const MAX_TRANSCLUSION_DEPTH: usize = 2;

/// Comrak borrows from the arena it parses into; ours outlive the render.
type ParserOptions = comrak::Options<'static>;
type SyntaxPlugins = comrak::options::Plugins<'static>;

pub struct RenderContext<'a> {
    pub root_id: &'a str,
    pub path: &'a str,
    pub index: &'a Index,
}

pub struct RenderedMarkdown {
    pub html: String,
    pub headings: Vec<Heading>,
    /// Set when something was skipped, so the UI can say the render was incomplete
    /// rather than presenting a partial document as if it were whole.
    pub warning: Option<String>,
}

pub fn render(body: &str, ctx: &RenderContext) -> RenderedMarkdown {
    let mut warnings: Vec<String> = Vec::new();
    let prepared = prepare_source(body, ctx, 0, &mut warnings);

    let arena = comrak::Arena::new();
    let options = parser_options();
    let root = comrak::parse_document(&arena, &prepared, &options);

    rewrite_relative_links(root, ctx);
    let headings = collect_headings(root);
    render_math(root, &mut warnings);
    extract_mermaid(root);

    let mut buffer = String::new();
    let plugins = syntax_plugins();
    let html = match comrak::format_html_with_plugins(root, &options, &mut buffer, &plugins) {
        Ok(()) => buffer,
        Err(error) => {
            // Never return nothing: show the source so the document is still readable.
            warnings.push(format!("rendering failed: {error}"));
            format!("<pre>{}</pre>", escape_text(body))
        }
    };

    RenderedMarkdown {
        html,
        headings,
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
    }
}

fn parser_options() -> ParserOptions {
    let mut options = comrak::Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options.extension.superscript = true;
    options.extension.math_dollars = true;
    options.extension.description_lists = true;
    options.parse.smart = false;
    // Documents are the user's own files, behind authentication; Obsidian renders their
    // inline HTML too. See the accepted-risk note in the project README.
    options.render.r#unsafe = true;
    options.render.github_pre_lang = true;
    // Without a prefix set, comrak emits no heading ids at all, leaving the table of
    // contents and every `#heading` link inert. Empty means "ids, no prefix".
    options.extension.header_id_prefix = Some(String::new());
    options
}

fn syntax_plugins() -> SyntaxPlugins {
    use std::sync::OnceLock;
    // Loading syntect's syntax definitions is expensive, so it happens once per process.
    static ADAPTER: OnceLock<super::highlight::ClassedHighlighter> = OnceLock::new();
    let adapter = ADAPTER.get_or_init(super::highlight::ClassedHighlighter::new);

    let mut plugins = comrak::options::Plugins::default();
    plugins.render.codefence_syntax_highlighter = Some(adapter);
    plugins
}

/// Rewrite Obsidian syntax into HTML the parser will pass through untouched.
fn prepare_source(
    body: &str,
    ctx: &RenderContext,
    depth: usize,
    warnings: &mut Vec<String>,
) -> String {
    // Runs before the scanners below so their byte offsets refer to the escaped text.
    let with_callouts = escape_stray_dollars(&callouts::transform(body));
    if !ctx.index.wikilinks {
        return with_callouts;
    }

    // Collect every replacement first so byte offsets stay valid while scanning.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for link in scan_wikilinks(&with_callouts) {
        let replacement = render_wikilink(&link, ctx, depth, warnings);
        edits.push((link.start, link.end, replacement));
    }
    for tag in scan_tags(&with_callouts) {
        edits.push((tag.start, tag.end, render_tag(&tag.name, ctx)));
    }
    splice(with_callouts, edits)
}

/// Apply the collected replacements left to right.
fn splice(source: String, mut edits: Vec<(usize, usize, String)>) -> String {
    if edits.is_empty() {
        return source;
    }
    edits.sort_by_key(|(start, _, _)| *start);

    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end, replacement) in edits {
        if start < cursor {
            continue; // Overlapping match; the earlier one wins.
        }
        out.push_str(&source[cursor..start]);
        out.push_str(&replacement);
        cursor = end;
    }
    out.push_str(&source[cursor..]);
    out
}

fn render_wikilink(
    link: &WikiLink,
    ctx: &RenderContext,
    depth: usize,
    warnings: &mut Vec<String>,
) -> String {
    // A bare `[[#heading]]` points inside the current document.
    if link.target.is_empty() {
        return heading_link(link);
    }

    let Some(resolved) = ctx.index.resolver.resolve(ctx.path, &link.target) else {
        return unresolved_link(&link.display());
    };

    if link.embed {
        return render_embed(&resolved, ctx, depth, warnings);
    }
    document_link(link, &resolved, ctx)
}

fn heading_link(link: &WikiLink) -> String {
    let anchor = link.heading.as_deref().unwrap_or_default();
    format!(
        "<a class=\"wikilink\" href=\"#{}\">{}</a>",
        escape_attr(&anchor_for(anchor)),
        escape_text(&link.display())
    )
}

/// Obsidian shows unresolved links as clickable-but-missing; mirroring that is more
/// useful than hiding the fact that a link is broken.
fn unresolved_link(display: &str) -> String {
    format!(
        "<span class=\"wikilink wikilink-unresolved\" title=\"No document matches this link\">{}</span>",
        escape_text(display)
    )
}

fn document_link(link: &WikiLink, resolved: &str, ctx: &RenderContext) -> String {
    let fragment = link
        .heading
        .as_ref()
        .map(|heading| format!("#{}", anchor_for(heading)))
        .unwrap_or_default();

    format!(
        "<a class=\"wikilink\" href=\"/n/{}/{}{}\" data-path=\"{}\">{}</a>",
        escape_attr(ctx.root_id),
        encode_path(resolved),
        fragment,
        escape_attr(resolved),
        escape_text(&link.display())
    )
}

fn embedded_kind(resolved: &str, ctx: &RenderContext) -> DocumentKind {
    ctx.index
        .get(resolved)
        .map(|document| document.kind)
        .unwrap_or(DocumentKind::Binary)
}

fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn render_embed(
    resolved: &str,
    ctx: &RenderContext,
    depth: usize,
    warnings: &mut Vec<String>,
) -> String {
    let file_url = format!(
        "/api/file/{}/{}",
        escape_attr(ctx.root_id),
        encode_path(resolved)
    );
    let name = file_name_of(resolved);

    match embedded_kind(resolved, ctx) {
        DocumentKind::Image => format!(
            "<img class=\"embed embed-image\" src=\"{}\" alt=\"{}\" loading=\"lazy\">",
            escape_attr(&file_url),
            escape_attr(&name)
        ),
        DocumentKind::Pdf => format!(
            "<div class=\"embed embed-pdf\"><object data=\"{url}\" type=\"application/pdf\"></object>\
             <a href=\"{url}\" target=\"_blank\" rel=\"noopener noreferrer\">Open {name}</a></div>",
            url = escape_attr(&file_url),
            name = escape_text(&name),
        ),
        DocumentKind::Markdown => transclude(resolved, ctx, depth, warnings),
        _ => format!(
            "<a class=\"embed embed-file\" href=\"{}\" download>{}</a>",
            escape_attr(&file_url),
            escape_text(&name)
        ),
    }
}

/// Inline an embedded note's own rendered body, up to `MAX_TRANSCLUSION_DEPTH`.
fn transclude(
    resolved: &str,
    ctx: &RenderContext,
    depth: usize,
    warnings: &mut Vec<String>,
) -> String {
    let name = file_name_of(resolved);
    if depth >= MAX_TRANSCLUSION_DEPTH {
        warnings.push(format!(
            "embed of {resolved} not expanded: nested too deeply"
        ));
        return format!(
            "<a class=\"wikilink\" href=\"/n/{}/{}\">{}</a>",
            escape_attr(ctx.root_id),
            encode_path(resolved),
            escape_text(&name)
        );
    }

    let Some(document) = ctx.index.get(resolved) else {
        return missing_embed(&name);
    };
    let Some(content) = &document.content else {
        return missing_embed(&name);
    };

    let (_, body) = kbview_core::frontmatter::split(content);
    let inner_ctx = RenderContext {
        root_id: ctx.root_id,
        path: resolved,
        index: ctx.index,
    };
    let inner = prepare_source(body, &inner_ctx, depth + 1, warnings);
    format!(
        "\n<div class=\"embed embed-note\" data-path=\"{}\">\n\n{}\n\n</div>\n\n",
        escape_attr(resolved),
        inner.trim()
    )
}

fn missing_embed(name: &str) -> String {
    format!(
        "<span class=\"wikilink wikilink-unresolved\">{}</span>",
        escape_text(name)
    )
}

fn render_tag(name: &str, ctx: &RenderContext) -> String {
    format!(
        "<a class=\"tag\" href=\"/t/{}/{}\">#{}</a>",
        escape_attr(ctx.root_id),
        encode_path(name),
        escape_text(name)
    )
}

/// Point relative links and images at app routes so they navigate in-app rather than 404.
/// This runs for plain folders too, which is what makes a non-Obsidian folder usable.
fn rewrite_relative_links<'a>(node: &'a comrak::nodes::AstNode<'a>, ctx: &RenderContext) {
    use comrak::nodes::NodeValue;

    let mut replacement: Option<String> = None;
    {
        let ast = node.data.borrow();
        match &ast.value {
            NodeValue::Link(link) => {
                if let Some(resolved) = ctx.index.resolver.resolve_relative(ctx.path, &link.url) {
                    replacement = Some(format!("/n/{}/{}", ctx.root_id, encode_path(&resolved)));
                }
            }
            NodeValue::Image(image) => {
                if let Some(resolved) = ctx.index.resolver.resolve_relative(ctx.path, &image.url) {
                    replacement = Some(format!(
                        "/api/file/{}/{}",
                        ctx.root_id,
                        encode_path(&resolved)
                    ));
                }
            }
            _ => {}
        }
    }

    if let Some(url) = replacement {
        let mut ast = node.data.borrow_mut();
        match &mut ast.value {
            NodeValue::Link(link) => link.url = url,
            NodeValue::Image(image) => image.url = url,
            _ => {}
        }
    }

    for child in node.children() {
        rewrite_relative_links(child, ctx);
    }
}

/// Replace math nodes with MathML, which every current browser renders natively — no
/// client-side maths library, no web fonts, nothing for the phone to download.
fn render_math<'a>(node: &'a comrak::nodes::AstNode<'a>, warnings: &mut Vec<String>) {
    use comrak::nodes::NodeValue;

    let converted = {
        let ast = node.data.borrow();
        if let NodeValue::Math(math) = &ast.value {
            let flavour = if math.display_math {
                latex2mathml::DisplayStyle::Block
            } else {
                latex2mathml::DisplayStyle::Inline
            };
            match latex2mathml::latex_to_mathml(&math.literal, flavour) {
                Ok(mathml) => Some(mathml),
                Err(error) => {
                    warnings.push(format!("could not render maths: {error}"));
                    // Show the source rather than dropping the expression silently.
                    Some(format!(
                        "<code class=\"math-error\">{}</code>",
                        escape_text(&math.literal)
                    ))
                }
            }
        } else {
            None
        }
    };

    if let Some(mathml) = converted {
        node.data.borrow_mut().value = NodeValue::HtmlInline(mathml);
    }

    for child in node.children() {
        render_math(child, warnings);
    }
}

/// Mermaid fences become `<pre class="mermaid">`; the client imports mermaid lazily and
/// only when such an element is present.
fn extract_mermaid<'a>(node: &'a comrak::nodes::AstNode<'a>) {
    use comrak::nodes::NodeValue;

    let replacement = {
        let ast = node.data.borrow();
        match &ast.value {
            NodeValue::CodeBlock(block) if block.info.trim().eq_ignore_ascii_case("mermaid") => {
                Some(format!(
                    "<pre class=\"mermaid\">{}</pre>\n",
                    escape_text(&block.literal)
                ))
            }
            _ => None,
        }
    };

    if let Some(html) = replacement {
        node.data.borrow_mut().value = NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
            block_type: 6,
            literal: html,
        });
    }

    for child in node.children() {
        extract_mermaid(child);
    }
}

/// The id comrak will emit for a heading with this text.
///
/// Using the parser's own anchorizer rather than a hand-rolled slug means a
/// `[[Note#Some Heading]]` fragment and the rendered `id` agree by construction, instead
/// of agreeing only until one of the two implementations is changed.
fn anchor_for(heading: &str) -> String {
    comrak::Anchorizer::new().anchorize(heading)
}

fn collect_headings<'a>(node: &'a comrak::nodes::AstNode<'a>) -> Vec<Heading> {
    let mut out = Vec::new();
    // One anchorizer for the document, so repeated heading text gets the same `-1`
    // suffixes the renderer applies.
    let mut anchorizer = comrak::Anchorizer::new();
    walk_headings(node, &mut out, &mut anchorizer);
    out
}

fn walk_headings<'a>(
    node: &'a comrak::nodes::AstNode<'a>,
    out: &mut Vec<Heading>,
    anchorizer: &mut comrak::Anchorizer,
) {
    use comrak::nodes::NodeValue;

    let level = match &node.data.borrow().value {
        NodeValue::Heading(heading) => Some(heading.level),
        _ => None,
    };

    if let Some(level) = level {
        let text = node_text(node);
        if !text.is_empty() {
            let slug = anchorizer.anchorize(&text);
            out.push(Heading {
                depth: level,
                slug,
                text,
            });
        }
    }

    for child in node.children() {
        walk_headings(child, out, anchorizer);
    }
}

fn node_text<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    use comrak::nodes::NodeValue;

    let mut out = String::new();
    match &node.data.borrow().value {
        NodeValue::Text(text) => out.push_str(text),
        NodeValue::Code(code) => out.push_str(&code.literal),
        _ => {}
    }
    for child in node.children() {
        out.push_str(&node_text(child));
    }
    out
}
