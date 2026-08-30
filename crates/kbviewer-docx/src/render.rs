//! Turning the intermediate model into semantic HTML and plain text.

use std::collections::HashSet;

use crate::escape::{is_safe_rel_id, push_escaped};
use crate::model::{
    Block, Formatting, Inline, LinkScope, ListMembership, Paragraph, Table, VerticalAlign,
};
use crate::path::mime_for_path;
use crate::rels::Relationships;
use crate::MediaRef;

pub(crate) struct Rendered {
    pub html: String,
    pub text: String,
    pub media: Vec<MediaRef>,
}

pub(crate) fn render(
    blocks: &[Block],
    media_base: &str,
    relationships: &Relationships,
) -> Rendered {
    let mut renderer = Renderer {
        html: String::new(),
        media_base: media_base.trim_end_matches('/').to_string(),
        relationships,
        media: Vec::new(),
        seen_media: HashSet::new(),
        text_lines: Vec::new(),
    };

    renderer.render_blocks(blocks);

    Rendered {
        html: renderer.html,
        text: renderer.text_lines.join("\n"),
        media: renderer.media,
    }
}

struct Renderer<'a> {
    html: String,
    media_base: String,
    relationships: &'a Relationships,
    media: Vec<MediaRef>,
    seen_media: HashSet<String>,
    text_lines: Vec<String>,
}

impl Renderer<'_> {
    fn render_blocks(&mut self, blocks: &[Block]) {
        let mut open_lists: Vec<ListMembership> = Vec::new();

        for block in blocks {
            match block {
                Block::Paragraph(paragraph) => self.render_paragraph(paragraph, &mut open_lists),
                Block::Table(table) => {
                    close_all_lists(&mut open_lists, &mut self.html);
                    self.render_table(table);
                }
            }
        }

        close_all_lists(&mut open_lists, &mut self.html);
    }

    /// Render a paragraph, opening or closing whatever list markup its
    /// membership implies.
    fn render_paragraph(&mut self, paragraph: &Paragraph, open_lists: &mut Vec<ListMembership>) {
        let Some(membership) = paragraph.list else {
            close_all_lists(open_lists, &mut self.html);
            self.render_standalone_paragraph(paragraph);
            return;
        };
        open_list_item(open_lists, membership, &mut self.html);
        let line = self.render_paragraph_content(paragraph);
        self.record_text(line);
    }

    fn render_standalone_paragraph(&mut self, paragraph: &Paragraph) {
        let tag = match paragraph.heading_level {
            Some(level) => format!("h{level}"),
            None => "p".to_string(),
        };
        self.html.push('<');
        self.html.push_str(&tag);
        self.html.push('>');
        let line = self.render_paragraph_content(paragraph);
        self.html.push_str("</");
        self.html.push_str(&tag);
        self.html.push_str(">\n");
        self.record_text(line);
    }

    /// Render a paragraph's inlines and return the same content as plain text.
    fn render_paragraph_content(&mut self, paragraph: &Paragraph) -> String {
        let mut line = String::new();
        self.render_inlines(&paragraph.inlines, &mut line);
        line
    }

    fn record_text(&mut self, line: String) {
        if !line.trim().is_empty() {
            self.text_lines.push(line);
        }
    }

    fn render_table(&mut self, table: &Table) {
        self.html.push_str("<table>");
        for row in &table.rows {
            self.html.push_str("<tr>");
            for cell in &row.cells {
                self.html.push_str("<td");
                if cell.colspan > 1 {
                    self.html
                        .push_str(&format!(" colspan=\"{}\"", cell.colspan));
                }
                self.html.push('>');
                self.render_cell_content(&cell.blocks);
                self.html.push_str("</td>");
            }
            self.html.push_str("</tr>");
        }
        self.html.push_str("</table>\n");
    }

    /// A cell holding a single plain paragraph is written bare; anything
    /// richer keeps its block structure.
    fn render_cell_content(&mut self, blocks: &[Block]) {
        if let [Block::Paragraph(paragraph)] = blocks {
            if paragraph.heading_level.is_none() && paragraph.list.is_none() {
                let line = self.render_paragraph_content(paragraph);
                self.record_text(line);
                return;
            }
        }
        self.render_blocks(blocks);
    }

    fn render_inlines(&mut self, inlines: &[Inline], text: &mut String) {
        for inline in inlines {
            match inline {
                Inline::Text {
                    text: content,
                    formatting,
                } => self.render_text(content, *formatting, text),
                Inline::LineBreak => {
                    self.html.push_str("<br>");
                    text.push('\n');
                }
                Inline::Image { rel_id, alt } => self.render_image(rel_id, alt, text),
                Inline::Link {
                    href,
                    scope,
                    children,
                } => {
                    self.open_anchor(href, *scope);
                    self.render_inlines(children, text);
                    self.html.push_str("</a>");
                }
            }
        }
    }

    /// Emit a run wrapped in the tags its formatting maps to.
    fn render_text(&mut self, content: &str, formatting: Formatting, text: &mut String) {
        let tags = formatting_tags(formatting);
        for tag in &tags {
            self.html.push('<');
            self.html.push_str(tag);
            self.html.push('>');
        }
        push_escaped(&mut self.html, content);
        for tag in tags.iter().rev() {
            self.html.push_str("</");
            self.html.push_str(tag);
            self.html.push('>');
        }
        text.push_str(content);
    }

    fn open_anchor(&mut self, href: &str, scope: LinkScope) {
        self.html.push_str("<a href=\"");
        push_escaped(&mut self.html, href);
        self.html.push('"');
        self.html.push_str(anchor_rel_attribute(scope));
        self.html.push('>');
    }

    /// Emit an `<img>` and register the media part behind it.
    ///
    /// An image whose relationship does not resolve to a part in the package
    /// is dropped: the app could only serve a broken link for it.
    fn render_image(&mut self, rel_id: &str, alt: &str, text: &mut String) {
        if !is_safe_rel_id(rel_id) {
            return;
        }
        let Some(zip_path) = self.relationships.internal_target(rel_id) else {
            return;
        };

        if self.seen_media.insert(rel_id.to_string()) {
            self.media.push(MediaRef {
                rel_id: rel_id.to_string(),
                mime: mime_for_path(&zip_path).to_string(),
                zip_path,
            });
        }

        self.html.push_str("<img src=\"");
        push_escaped(&mut self.html, &self.media_base);
        self.html.push('/');
        push_escaped(&mut self.html, rel_id);
        self.html.push_str("\" alt=\"");
        push_escaped(&mut self.html, alt);
        self.html.push_str("\">");
        text.push_str(alt);
    }
}

/// What an anchor needs beyond its `href`, if anything.
fn anchor_rel_attribute(scope: LinkScope) -> &'static str {
    match scope {
        LinkScope::External => " rel=\"noopener noreferrer\"",
        LinkScope::InDocument => "",
    }
}

/// The HTML tags a run's formatting maps to, outermost first.
fn formatting_tags(formatting: Formatting) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if formatting.bold {
        tags.push("strong");
    }
    if formatting.italic {
        tags.push("em");
    }
    if formatting.underline {
        tags.push("u");
    }
    if formatting.strike {
        tags.push("s");
    }
    match formatting.vertical_align {
        VerticalAlign::Superscript => tags.push("sup"),
        VerticalAlign::Subscript => tags.push("sub"),
        VerticalAlign::Baseline => {}
    }
    tags
}

fn list_close_tag(membership: ListMembership) -> &'static str {
    if membership.ordered {
        "</ol>"
    } else {
        "</ul>"
    }
}

/// Open whatever lists and items are needed for the next list paragraph.
///
/// Word does not mark where a list starts or ends; it marks each paragraph
/// with a level and a definition, and the nesting has to be reconstructed
/// from the sequence.
fn open_list_item(open_lists: &mut Vec<ListMembership>, item: ListMembership, html: &mut String) {
    while open_lists.last().is_some_and(|top| top.level > item.level) {
        html.push_str("</li>");
        if let Some(closed) = open_lists.pop() {
            html.push_str(list_close_tag(closed));
        }
    }

    if let Some(top) = open_lists.last().copied() {
        if top.level == item.level {
            html.push_str("</li>");
            // A switch between bullets and numbers at the same level is a new
            // list, not a continuation.
            if top.ordered != item.ordered {
                open_lists.pop();
                html.push_str(list_close_tag(top));
            }
        }
    }

    if open_lists.last().is_none_or(|top| top.level < item.level) {
        html.push_str(if item.ordered { "<ol>" } else { "<ul>" });
        open_lists.push(item);
    }

    html.push_str("<li>");
}

/// Close every open list, ending the run of list markup with a newline the
/// way every other block-level element does.
fn close_all_lists(open_lists: &mut Vec<ListMembership>, html: &mut String) {
    let had_lists = !open_lists.is_empty();
    while let Some(closed) = open_lists.pop() {
        html.push_str("</li>");
        html.push_str(list_close_tag(closed));
    }
    if had_lists {
        html.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list_html(items: &[(u8, bool)]) -> String {
        let mut html = String::new();
        let mut open_lists = Vec::new();
        for (level, ordered) in items {
            open_list_item(
                &mut open_lists,
                ListMembership {
                    level: *level,
                    ordered: *ordered,
                },
                &mut html,
            );
            html.push('x');
        }
        close_all_lists(&mut open_lists, &mut html);
        html
    }

    #[test]
    fn nests_and_unnests_lists() {
        assert_eq!(
            list_html(&[(0, false), (1, false), (0, false)]).replace('\n', ""),
            "<ul><li>x<ul><li>x</li></ul></li><li>x</li></ul>"
        );
    }

    #[test]
    fn starts_a_new_list_when_the_marker_style_changes() {
        assert_eq!(
            list_html(&[(0, false), (0, true)]).replace('\n', ""),
            "<ul><li>x</li></ul><ol><li>x</li></ol>"
        );
    }

    #[test]
    fn tolerates_a_level_that_jumps_more_than_one_step() {
        assert_eq!(
            list_html(&[(0, true), (2, true)]).replace('\n', ""),
            "<ol><li>x<ol><li>x</li></ol></li></ol>"
        );
    }
}
