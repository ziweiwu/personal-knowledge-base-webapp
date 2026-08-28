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
                Ok(mathml) => Some(wrap_math_row(&mathml)),
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

/// Give every rendered image `loading="lazy"` and `decoding="async"`.
///
/// This has to be in the HTML the browser first parses. The client used to set it after
/// writing the markup with `innerHTML`, by which point the fetches had already started —
/// so the attribute was present on inspection and had no effect whatever. Measured on two
/// documents whose second image sat 5000px down: the client-set version fetched both, the
/// server-set version fetched one.
///
/// comrak renders an image as `<img ... />` with no way to add attributes from the AST, so
/// this is a pass over the generated markup. It is comrak's own output plus whatever raw
/// HTML the document carried; an `<img` inside a code block is already escaped to `&lt;img`
/// by the time this runs and cannot match.
pub(super) fn lazy_load_images(html: &str) -> String {
    const TAG: &str = "<img";
    const ATTRIBUTES: &str = " loading=\"lazy\" decoding=\"async\"";

    /// Most documents carry a handful of images; this only sizes the initial allocation.
    const TYPICAL_IMAGES_PER_DOCUMENT: usize = 4;

    let mut out =
        String::with_capacity(html.len() + ATTRIBUTES.len() * TYPICAL_IMAGES_PER_DOCUMENT);
    let mut rest = html;
    while let Some(at) = rest.find(TAG) {
        let (before, tail) = rest.split_at(at);
        out.push_str(before);
        let Some(end) = tail.find('>') else {
            out.push_str(tail);
            return out;
        };
        let (tag, remainder) = tail.split_at(end + 1);

        out.push_str(TAG);
        // An embed already carries the attribute; a second copy would be invalid markup.
        if !tag.contains("loading=") {
            out.push_str(ATTRIBUTES);
        }
        out.push_str(&tag[TAG.len()..]);
        rest = remainder;
    }
    out.push_str(rest);
    out
}

/// Make rendered task checkboxes clickable, and tell the client which line of the file on
/// disk each one stands for.
///
/// comrak renders them `disabled`, which is right for a static page and wrong for a
/// reading app where ticking something off is the most common edit there is.
///
/// `tasks` comes from scanning the **raw** file with the same scanner the write path uses,
/// never from the parsed document: the parser only ever sees a prepared source, with
/// frontmatter stripped and callouts expanded, so its line numbers do not address the file
/// the toggle will write to.
///
/// That leaves the pairing to check — see `pairing_is_sound`. Where it does not hold,
/// nothing is injected and every box stays read-only: a checkbox that does nothing is a
/// small disappointment, whereas one wired to the wrong line edits the wrong task.
pub(super) fn enable_task_checkboxes(html: &str, tasks: &[kbview_core::tasks::TaskLine]) -> String {
    if tasks.is_empty() || !pairing_is_sound(&checkbox_tags(html), tasks) {
        return html.to_string();
    }

    let mut out = String::with_capacity(html.len() + tasks.len() * ATTRIBUTES_PER_BOX);
    let mut rest = html;
    for task in tasks {
        let Some((before, tag, remainder)) = split_at_checkbox(rest) else {
            break;
        };
        out.push_str(before);
        out.push_str(&rewritten_tag(tag, task.line));
        rest = remainder;
    }
    out.push_str(rest);
    out
}

/// Roughly the bytes the injected `data-task-line` and `class` attributes add per box.
const ATTRIBUTES_PER_BOX: usize = 32;

const CHECKBOX_MARKER: &str = "<input type=\"checkbox\"";

/// Whether the scanned tasks and the rendered checkboxes are describing the same things.
///
/// Matched in document order, so both the count and every box's ticked state must agree.
/// A transcluded document brings checkboxes the host file has no line for, and a literal
/// `<input type="checkbox">` in raw HTML shifts the pairing by one.
fn pairing_is_sound(rendered: &[&str], tasks: &[kbview_core::tasks::TaskLine]) -> bool {
    use kbview_core::tasks::TaskState;

    rendered.len() == tasks.len()
        && rendered.iter().zip(tasks).all(|(tag, task)| {
            let drawn_ticked = tag.contains(" checked");
            drawn_ticked == (task.state == TaskState::Done)
        })
}

fn rewritten_tag(tag: &str, line: usize) -> String {
    tag.replace(" disabled=\"\"", "").replace(
        "<input",
        &format!("<input data-task-line=\"{line}\" class=\"task-checkbox\""),
    )
}

/// Split `html` around its first checkbox tag: what precedes it, the tag, and the rest.
fn split_at_checkbox(html: &str) -> Option<(&str, &str, &str)> {
    let at = html.find(CHECKBOX_MARKER)?;
    let (before, tail) = html.split_at(at);
    let end = tail.find('>')?;
    let (tag, remainder) = tail.split_at(end + 1);
    Some((before, tag, remainder))
}

/// Every rendered checkbox tag, in document order, so the pairing can be checked before a
/// single one is rewritten.
fn checkbox_tags(html: &str) -> Vec<&str> {
    let mut tags = Vec::new();
    let mut rest = html;
    while let Some((_, tag, remainder)) = split_at_checkbox(rest) {
        tags.push(tag);
        rest = remainder;
    }
    tags
}

/// Wrap a `<math>` element's children in a single `<mrow>`.
///
/// `latex2mathml` emits its terms as direct children of `<math>`. A display-mode `<math>`
/// is a block box, and each direct child then becomes a block box too, so the equation
/// renders as a vertical stack of fragments — one term per line — instead of a line of
/// maths. One `<mrow>` gives the terms a single inline row to sit in.
fn wrap_math_row(mathml: &str) -> String {
    let Some(open_end) = mathml.find('>') else {
        return mathml.to_string();
    };
    let Some(close_start) = mathml.rfind("</math>") else {
        return mathml.to_string();
    };
    if close_start <= open_end {
        return mathml.to_string();
    }

    let (open, rest) = mathml.split_at(open_end + 1);
    let inner = &rest[..close_start - open_end - 1];
    // Already a single row: nothing to gain, and re-wrapping would nest pointlessly.
    if inner.trim_start().starts_with("<mrow") && inner.trim_end().ends_with("</mrow>") {
        return mathml.to_string();
    }
    format!("{open}<mrow>{inner}</mrow></math>")
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

#[cfg(test)]
mod math_tests {
    use super::wrap_math_row;

    /// The regression: bare terms under a display `<math>` each become a block box, so an
    /// equation renders as a vertical stack of fragments rather than a line of maths.
    #[test]
    fn terms_are_gathered_into_one_row() {
        let bare = "<math display=\"block\"><mi>a</mi><mo>+</mo><mi>b</mi></math>";
        assert_eq!(
            wrap_math_row(bare),
            "<math display=\"block\"><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow></math>"
        );
    }

    #[test]
    fn an_already_wrapped_expression_is_left_alone() {
        let wrapped = "<math><mrow><mi>a</mi></mrow></math>";
        assert_eq!(wrap_math_row(wrapped), wrapped);
    }

    #[test]
    fn malformed_input_is_passed_through_rather_than_mangled() {
        for input in ["", "<math>", "not math at all", "</math>"] {
            assert_eq!(wrap_math_row(input), input);
        }
    }

    #[test]
    fn attributes_on_the_math_element_survive() {
        let out = wrap_math_row("<math xmlns=\"x\" display=\"block\"><mi>a</mi></math>");
        assert!(
            out.starts_with("<math xmlns=\"x\" display=\"block\"><mrow>"),
            "got {out}"
        );
    }
}

#[cfg(test)]
mod lazy_image_tests {
    use super::lazy_load_images;

    #[test]
    fn every_image_gains_the_attributes() {
        let out = lazy_load_images("<p><img src=\"a.png\" alt=\"a\" /><img src=\"b.png\" /></p>");
        assert_eq!(out.matches("loading=\"lazy\"").count(), 2, "got {out}");
        assert_eq!(out.matches("decoding=\"async\"").count(), 2, "got {out}");
        assert!(
            out.contains("src=\"a.png\""),
            "the tag must survive intact: {out}"
        );
    }

    /// Obsidian embeds already carry it; a second copy would be invalid markup.
    #[test]
    fn an_image_that_already_declares_loading_is_left_alone() {
        let html = "<img class=\"embed\" src=\"a.png\" loading=\"lazy\">";
        assert_eq!(lazy_load_images(html), html);
    }

    /// An image written as a code sample is escaped before this runs, so it cannot match
    /// and must not be rewritten into something that renders.
    #[test]
    fn an_escaped_image_inside_a_code_block_is_untouched() {
        let html = "<pre><code>&lt;img src=\"a.png\"&gt;</code></pre>";
        assert_eq!(lazy_load_images(html), html);
    }

    #[test]
    fn a_document_with_no_images_is_returned_unchanged() {
        let html = "<p>nothing here</p>";
        assert_eq!(lazy_load_images(html), html);
    }

    #[test]
    fn a_truncated_tag_does_not_lose_the_tail() {
        let html = "<p>text</p><img src=\"a.png\"";
        assert!(
            lazy_load_images(html).ends_with("<img src=\"a.png\""),
            "content must not be dropped"
        );
    }
}

#[cfg(test)]
mod task_tests {
    use super::enable_task_checkboxes;
    use kbview_core::tasks::{TaskLine, TaskState};

    const BOX: &str = "<input type=\"checkbox\" disabled=\"\" />";
    const CHECKED: &str = "<input type=\"checkbox\" checked=\"\" disabled=\"\" />";

    /// Which line of the fixture file each checkbox in these tests stands for. The values
    /// only have to be distinct and plausible; nothing here parses a real document.
    const FIRST_TASK_LINE: usize = 3;
    const SECOND_TASK_LINE: usize = 4;

    fn todo(line: usize) -> TaskLine {
        TaskLine {
            line,
            state: TaskState::Todo,
        }
    }

    fn done(line: usize) -> TaskLine {
        TaskLine {
            line,
            state: TaskState::Done,
        }
    }

    #[test]
    fn each_checkbox_is_enabled_and_carries_its_line() {
        let html = format!("<ul><li>{BOX}a</li><li>{CHECKED}b</li></ul>");
        let out = enable_task_checkboxes(&html, &[todo(FIRST_TASK_LINE), done(SECOND_TASK_LINE)]);
        assert!(
            out.contains(&format!("data-task-line=\"{FIRST_TASK_LINE}\"")),
            "got {out}"
        );
        assert!(
            out.contains(&format!("data-task-line=\"{SECOND_TASK_LINE}\"")),
            "got {out}"
        );
        assert!(
            !out.contains("disabled"),
            "checkboxes must be clickable: {out}"
        );
        assert!(
            out.contains("checked=\"\""),
            "the checked state must survive"
        );
    }

    /// A literal checkbox in the document's own raw HTML would shift the pairing, so the
    /// injection is skipped rather than wiring a box to the wrong task.
    #[test]
    fn a_count_mismatch_leaves_every_checkbox_alone() {
        let html = format!("<p>{BOX}</p><ul><li>{BOX}a</li></ul>");
        let out = enable_task_checkboxes(&html, &[todo(FIRST_TASK_LINE)]);
        assert_eq!(out, html);
        assert!(out.contains("disabled"));
    }

    /// The counts can agree while the pairing is still wrong. Ticked state is the cheap
    /// second opinion, and disagreement means the two lists are not describing the same
    /// tasks — so none of them is wired up.
    #[test]
    fn a_state_mismatch_leaves_every_checkbox_alone() {
        let html = format!("<ul><li>{BOX}a</li><li>{CHECKED}b</li></ul>");
        let out = enable_task_checkboxes(&html, &[done(FIRST_TASK_LINE), todo(SECOND_TASK_LINE)]);
        assert_eq!(out, html);
        assert!(out.contains("disabled"));
    }

    #[test]
    fn a_document_with_no_tasks_is_untouched() {
        let html = "<p>nothing here</p>";
        assert_eq!(enable_task_checkboxes(html, &[]), html);
    }
}
