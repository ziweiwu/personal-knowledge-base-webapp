//! `word/document.xml`: turning the body into the intermediate model.
//!
//! The parser is a recursive descent over `quick-xml` events. Its governing
//! rule is that an element it does not recognise is descended into rather
//! than rejected, so a document using a feature this crate has never heard of
//! still renders everything around it.
//!
//! Descending into the unknown is what makes the depth guard load-bearing.
//! Every function that can recurse checks [`MAX_DEPTH`] before it looks at
//! the element name, so a pathologically nested file exhausts the limit
//! rather than the call stack.

use quick_xml::events::{BytesStart, Event};

use crate::escape::sanitize_url;
use crate::model::{
    extend_inlines, push_text, Block, Formatting, Inline, LinkScope, ListMembership, Paragraph,
    Table, TableCell, TableRow, VerticalAlign,
};
use crate::numbering::{Numbering, MAX_LIST_LEVEL};
use crate::rels::Relationships;
use crate::styles::{outline_heading_level, Styles};
use crate::xml::{
    attribute, local_name, on_off_value, relationship_attribute, resolve_entity, XmlReader,
    MAX_DEPTH,
};
use crate::DocxError;

/// The other parts of the package that reading a body consults.
pub(crate) struct DocumentContext<'a> {
    pub styles: &'a Styles,
    pub numbering: &'a Numbering,
    pub relationships: &'a Relationships,
}

/// Parse the body of the main document part.
pub(crate) fn parse(
    xml: &str,
    part: &str,
    context: &DocumentContext,
) -> Result<Vec<Block>, DocxError> {
    let mut parser = DocumentParser {
        reader: XmlReader::new(xml, part),
        styles: context.styles,
        numbering: context.numbering,
        relationships: context.relationships,
    };
    parser.parse_body()
}

struct DocumentParser<'a, 'x> {
    reader: XmlReader<'x>,
    styles: &'a Styles,
    numbering: &'a Numbering,
    relationships: &'a Relationships,
}

/// A start tag paired with its namespace-stripped local name.
///
/// Every dispatcher needs both, and working the name out costs an
/// allocation, so it is worked out once where the tag is read.
struct StartTag<'x> {
    start: BytesStart<'x>,
    name: String,
}

impl<'x> StartTag<'x> {
    fn new(start: BytesStart<'x>) -> Self {
        let name = local_name(&start);
        Self { start, name }
    }
}

/// What an inline walk carries down: the formatting inherited from enclosing
/// runs, and how deep the recursion has already gone.
#[derive(Debug, Clone, Copy, Default)]
struct InlineContext {
    formatting: Formatting,
    depth: usize,
}

impl InlineContext {
    fn at_depth(depth: usize) -> Self {
        Self {
            formatting: Formatting::default(),
            depth,
        }
    }

    /// The context for one element further down.
    fn deeper(self) -> Self {
        Self {
            depth: self.depth + 1,
            ..self
        }
    }

    /// Whether the recursion guard has been reached.
    fn exhausted(self) -> bool {
        self.depth >= MAX_DEPTH
    }
}

/// Paragraph properties gathered from `w:pPr`.
#[derive(Default)]
struct ParagraphProperties {
    style_id: Option<String>,
    outline_level: Option<u8>,
    num_id: Option<String>,
    list_level: Option<u8>,
}

impl ParagraphProperties {
    /// Absorb one element of a `w:pPr` subtree.
    fn read(&mut self, element: &BytesStart) {
        match local_name(element).as_str() {
            "pStyle" => self.style_id = attribute(element, "val"),
            "outlineLvl" => self.outline_level = outline_heading_level(element),
            "ilvl" => {
                self.list_level =
                    attribute(element, "val").and_then(|value| value.parse::<u8>().ok());
            }
            // `w:numId` of 0 is Word's way of saying "not in a list".
            "numId" => self.num_id = attribute(element, "val").filter(|value| value.trim() != "0"),
            _ => {}
        }
    }
}

/// A `w:p` under construction.
#[derive(Default)]
struct ParagraphBuilder {
    properties: ParagraphProperties,
    inlines: Vec<Inline>,
}

/// Block-level content under construction, plus the runs that turned up
/// outside any paragraph.
#[derive(Default)]
struct BlockSink {
    blocks: Vec<Block>,
    stray_inlines: Vec<Inline>,
}

impl BlockSink {
    /// Runs outside any paragraph are malformed, but their text is still the
    /// author's, so they become a trailing paragraph.
    fn finish(mut self) -> Vec<Block> {
        if !self.stray_inlines.is_empty() {
            self.blocks.push(Block::Paragraph(Paragraph {
                inlines: self.stray_inlines,
                ..Paragraph::default()
            }));
        }
        self.blocks
    }
}

/// The pictures found in one drawing container, and the alt text to give
/// them.
struct ImageCollector<'o> {
    out: &'o mut Vec<Inline>,
    alt: String,
    seen: Vec<String>,
}

impl ImageCollector<'_> {
    fn new(out: &mut Vec<Inline>) -> ImageCollector<'_> {
        ImageCollector {
            out,
            alt: String::new(),
            seen: Vec::new(),
        }
    }

    /// Absorb one element of a `w:drawing`, `w:pict` or `w:object` subtree.
    fn read(&mut self, element: &BytesStart) {
        match local_name(element).as_str() {
            "docPr" | "cNvPr" => self.read_alt_text(element),
            "blip" => self.emit(relationship_attribute(element, "embed")),
            "imagedata" => self.emit(relationship_attribute(element, "id")),
            _ => {}
        }
    }

    /// The first description wins, so that a legacy fallback branch does not
    /// overwrite the alt text of the drawing it stands in for.
    fn read_alt_text(&mut self, element: &BytesStart) {
        if !self.alt.is_empty() {
            return;
        }
        self.alt = attribute(element, "descr")
            .or_else(|| attribute(element, "title"))
            .unwrap_or_default();
    }

    /// The same picture often appears twice, once per branch of an
    /// `mc:AlternateContent`; emit it once.
    fn emit(&mut self, rel_id: Option<String>) {
        let Some(rel_id) = rel_id.filter(|id| !self.seen.contains(id)) else {
            return;
        };
        self.seen.push(rel_id.clone());
        self.out.push(Inline::Image {
            rel_id,
            alt: self.alt.clone(),
        });
    }
}

impl DocumentParser<'_, '_> {
    /// Skip forward to `w:body` and parse it.
    ///
    /// A document with no body is empty, not broken; input that simply stops
    /// while elements are still open is broken.
    fn parse_body(&mut self) -> Result<Vec<Block>, DocxError> {
        let mut depth = 0usize;
        loop {
            match self.reader.next()? {
                Event::Start(element) if local_name(&element) == "body" => {
                    return self.parse_blocks("w:body", 1)
                }
                Event::Start(_) => depth += 1,
                Event::End(_) => depth = depth.saturating_sub(1),
                Event::Eof if depth > 0 => return Err(self.reader.unexpected_eof("w:document")),
                Event::Eof => return Ok(Vec::new()),
                _ => {}
            }
        }
    }

    /// Read block-level content until the end tag of `container`.
    fn parse_blocks(&mut self, container: &str, depth: usize) -> Result<Vec<Block>, DocxError> {
        let mut sink = BlockSink::default();
        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.parse_block(&StartTag::new(element), depth, &mut sink)?
                }
                Event::Empty(element) => push_empty_paragraph(&element, &mut sink.blocks),
                Event::End(_) => break,
                Event::Eof => return Err(self.reader.unexpected_eof(container)),
                _ => {}
            }
        }
        Ok(sink.finish())
    }

    /// Handle one block-level `Start` event.
    ///
    /// The depth guard comes before anything else, because the fallback arm
    /// descends into elements this crate does not know.
    fn parse_block(
        &mut self,
        tag: &StartTag,
        depth: usize,
        sink: &mut BlockSink,
    ) -> Result<(), DocxError> {
        if depth >= MAX_DEPTH {
            return self.reader.skip_element(&tag.name);
        }
        match tag.name.as_str() {
            "p" => sink
                .blocks
                .push(Block::Paragraph(self.parse_paragraph(depth + 1)?)),
            "tbl" => sink.blocks.push(Block::Table(self.parse_table(depth + 1)?)),
            "sectPr" | "bookmarkStart" | "bookmarkEnd" | "proofErr" | "del" => {
                self.reader.skip_element(&tag.name)?
            }
            "AlternateContent" => sink
                .blocks
                .extend(self.parse_alternate_content_blocks(depth + 1)?),
            "r" | "hyperlink" => self.dispatch_inline_start(
                tag,
                &mut sink.stray_inlines,
                InlineContext::at_depth(depth),
            )?,
            _ => sink.blocks.extend(self.parse_blocks(&tag.name, depth + 1)?),
        }
        Ok(())
    }

    fn parse_paragraph(&mut self, depth: usize) -> Result<Paragraph, DocxError> {
        let context = InlineContext::at_depth(depth);
        let mut builder = ParagraphBuilder::default();

        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.parse_paragraph_child(StartTag::new(element), &mut builder, context)?
                }
                Event::Empty(element) => {
                    push_empty_inline(&element, &mut builder.inlines, Formatting::default())
                }
                Event::End(_) => break,
                Event::Eof => return Err(self.reader.unexpected_eof("w:p")),
                _ => {}
            }
        }

        Ok(self.finish_paragraph(builder))
    }

    /// A `w:pPr` says what kind of paragraph this is; everything else in it
    /// is content.
    ///
    /// Reading the properties does not recurse, so it is reached without
    /// consulting the depth guard.
    fn parse_paragraph_child(
        &mut self,
        tag: StartTag,
        builder: &mut ParagraphBuilder,
        context: InlineContext,
    ) -> Result<(), DocxError> {
        if tag.name == "pPr" {
            return self.parse_paragraph_properties(&mut builder.properties);
        }
        self.dispatch_inline_start(&tag, &mut builder.inlines, context)
    }

    /// Scan the `w:pPr` subtree for the style, outline level and numbering.
    fn parse_paragraph_properties(
        &mut self,
        properties: &mut ParagraphProperties,
    ) -> Result<(), DocxError> {
        let mut elements = self.reader.scan("w:pPr");
        while let Some(element) = elements.next_element()? {
            properties.read(&element);
        }
        Ok(())
    }

    /// Classify a finished paragraph from the properties it declared.
    fn finish_paragraph(&self, builder: ParagraphBuilder) -> Paragraph {
        let properties = builder.properties;
        let heading_level = properties
            .style_id
            .as_deref()
            .and_then(|id| self.styles.heading_level(id))
            .or(properties.outline_level);

        let list = properties.num_id.as_deref().map(|num_id| {
            let level = properties.list_level.unwrap_or(0).min(MAX_LIST_LEVEL);
            ListMembership {
                level,
                ordered: self.numbering.is_ordered(num_id, level),
            }
        });

        Paragraph {
            heading_level,
            list,
            inlines: builder.inlines,
        }
    }

    /// Handle one inline-level `Start` event.
    ///
    /// The depth guard comes before the element name is looked at, because
    /// the fallback arm descends into unknown elements.
    fn dispatch_inline_start(
        &mut self,
        tag: &StartTag,
        out: &mut Vec<Inline>,
        context: InlineContext,
    ) -> Result<(), DocxError> {
        if context.exhausted() {
            return self.reader.skip_element(&tag.name);
        }
        match tag.name.as_str() {
            "r" => self.parse_run(out, context.deeper()),
            "hyperlink" => self.parse_hyperlink(&tag.start, out, context.deeper()),
            "drawing" | "pict" | "object" => self.parse_image_container(out, &tag.name),
            "t" => self.parse_text(&tag.start, out, context.formatting),
            "br" | "cr" => self.parse_line_break(out, &tag.name),
            "tab" | "noBreakHyphen" => self.parse_character(out, &tag.name, context.formatting),
            // Deleted text, field instructions and annotation anchors are not
            // part of what the document says.
            "del" | "delText" | "instrText" | "fldChar" | "softHyphen" | "sym" | "rPr" | "pPr"
            | "proofErr" | "bookmarkStart" | "bookmarkEnd" | "commentReference"
            | "commentRangeStart" | "commentRangeEnd" => self.reader.skip_element(&tag.name),
            "AlternateContent" => self.parse_alternate_content_inlines(out, context.deeper()),
            _ => self.parse_inline_children(out, &tag.name, context.deeper()),
        }
    }

    /// Read inline content until the end tag of `container`.
    fn parse_inline_children(
        &mut self,
        out: &mut Vec<Inline>,
        container: &str,
        context: InlineContext,
    ) -> Result<(), DocxError> {
        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.dispatch_inline_start(&StartTag::new(element), out, context)?
                }
                Event::Empty(element) => push_empty_inline(&element, out, context.formatting),
                Event::End(_) => return Ok(()),
                Event::Eof => return Err(self.reader.unexpected_eof(container)),
                _ => {}
            }
        }
    }

    fn parse_run(
        &mut self,
        out: &mut Vec<Inline>,
        mut context: InlineContext,
    ) -> Result<(), DocxError> {
        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.parse_run_child(StartTag::new(element), out, &mut context)?
                }
                Event::Empty(element) => push_empty_inline(&element, out, context.formatting),
                Event::End(_) => return Ok(()),
                Event::Eof => return Err(self.reader.unexpected_eof("w:r")),
                _ => {}
            }
        }
    }

    /// A `w:rPr` sets the formatting for the rest of the run; everything else
    /// in it is content.
    ///
    /// Reading the properties does not recurse, so it is reached without
    /// consulting the depth guard.
    fn parse_run_child(
        &mut self,
        tag: StartTag,
        out: &mut Vec<Inline>,
        context: &mut InlineContext,
    ) -> Result<(), DocxError> {
        if tag.name == "rPr" {
            return self.parse_run_properties(&mut context.formatting);
        }
        self.dispatch_inline_start(&tag, out, *context)
    }

    /// Scan the `w:rPr` subtree for the character formatting this crate maps
    /// to semantic HTML.
    fn parse_run_properties(&mut self, formatting: &mut Formatting) -> Result<(), DocxError> {
        let mut elements = self.reader.scan("w:rPr");
        while let Some(element) = elements.next_element()? {
            apply_run_property(formatting, &element);
        }
        Ok(())
    }

    /// Read a `w:t` and append what it says.
    fn parse_text(
        &mut self,
        element: &BytesStart,
        out: &mut Vec<Inline>,
        formatting: Formatting,
    ) -> Result<(), DocxError> {
        let text = self.read_text_element(element, "w:t")?;
        push_text(out, &text, formatting);
        Ok(())
    }

    fn parse_line_break(&mut self, out: &mut Vec<Inline>, name: &str) -> Result<(), DocxError> {
        out.push(Inline::LineBreak);
        self.reader.skip_element(name)
    }

    /// `w:tab` and `w:noBreakHyphen` are single characters that Word stores
    /// as elements of their own.
    fn parse_character(
        &mut self,
        out: &mut Vec<Inline>,
        name: &str,
        formatting: Formatting,
    ) -> Result<(), DocxError> {
        let character = if name == "tab" { "\t" } else { "-" };
        push_text(out, character, formatting);
        self.reader.skip_element(name)
    }

    /// Read the text of a `w:t`, honouring `xml:space`.
    ///
    /// Without `xml:space="preserve"` the surrounding whitespace is not
    /// significant, which is why Word writes the attribute whenever it is.
    fn read_text_element(
        &mut self,
        element: &BytesStart,
        container: &str,
    ) -> Result<String, DocxError> {
        let preserve = attribute(element, "space").as_deref() == Some("preserve");
        let mut text = String::new();
        let mut depth = 1usize;

        loop {
            match self.reader.next()? {
                Event::Text(chunk) => text.push_str(&chunk.xml10_content()),
                Event::CData(chunk) => text.push_str(&chunk.xml10_content()),
                Event::GeneralRef(reference) => {
                    push_resolved_entity(&mut text, &reference.xml10_content())
                }
                Event::Start(_) => depth += 1,
                Event::End(_) if depth <= 1 => break,
                Event::End(_) => depth -= 1,
                Event::Eof => return Err(self.reader.unexpected_eof(container)),
                _ => {}
            }
        }

        Ok(if preserve {
            text
        } else {
            text.trim().to_string()
        })
    }

    fn parse_hyperlink(
        &mut self,
        element: &BytesStart,
        out: &mut Vec<Inline>,
        context: InlineContext,
    ) -> Result<(), DocxError> {
        let link = self.hyperlink_target(element);

        let mut children = Vec::new();
        self.parse_inline_children(&mut children, "w:hyperlink", context)?;

        match link {
            Some((href, scope)) => out.push(Inline::Link {
                href,
                scope,
                children,
            }),
            // An unresolvable link still has readable text.
            None => extend_inlines(out, children),
        }
        Ok(())
    }

    /// Where a `w:hyperlink` points: an external URL named by the
    /// relationships part, or an anchor inside the document.
    fn hyperlink_target(&self, element: &BytesStart) -> Option<(String, LinkScope)> {
        let anchor = attribute(element, "anchor");
        let external_target = relationship_attribute(element, "id")
            .and_then(|id| self.relationships.get(&id))
            .filter(|relationship| relationship.external)
            .map(|relationship| match anchor.as_deref() {
                Some(fragment) => format!("{}#{fragment}", relationship.target),
                None => relationship.target.clone(),
            });

        match external_target {
            Some(target) => sanitize_url(&target).map(|href| (href, LinkScope::External)),
            None => anchor
                .as_deref()
                .and_then(|fragment| sanitize_url(&format!("#{fragment}")))
                .map(|href| (href, LinkScope::InDocument)),
        }
    }

    /// Pull image references out of a `w:drawing`, `w:pict` or `w:object`.
    ///
    /// The subtree is walked iteratively rather than descended into, because
    /// DrawingML reuses local names such as `p` and `t` for its own purposes
    /// and none of it is document text.
    fn parse_image_container(
        &mut self,
        out: &mut Vec<Inline>,
        container: &str,
    ) -> Result<(), DocxError> {
        let mut collector = ImageCollector::new(out);
        let mut elements = self.reader.scan(container);
        while let Some(element) = elements.next_element()? {
            collector.read(&element);
        }
        Ok(())
    }

    /// Take the first branch of an `mc:AlternateContent` and discard the
    /// rest, so a drawing with a legacy fallback is not rendered twice.
    fn parse_alternate_content_inlines(
        &mut self,
        out: &mut Vec<Inline>,
        context: InlineContext,
    ) -> Result<(), DocxError> {
        let mut taken = false;
        loop {
            match self.reader.next()? {
                Event::Start(element) if taken || context.exhausted() => {
                    self.reader.skip_element(&local_name(&element))?
                }
                Event::Start(element) => {
                    taken = true;
                    self.parse_inline_children(out, &local_name(&element), context.deeper())?;
                }
                Event::End(_) => return Ok(()),
                Event::Eof => return Err(self.reader.unexpected_eof("mc:AlternateContent")),
                _ => {}
            }
        }
    }

    fn parse_alternate_content_blocks(&mut self, depth: usize) -> Result<Vec<Block>, DocxError> {
        let mut blocks = Vec::new();
        let mut taken = false;
        loop {
            match self.reader.next()? {
                Event::Start(element) if taken || depth >= MAX_DEPTH => {
                    self.reader.skip_element(&local_name(&element))?
                }
                Event::Start(element) => {
                    taken = true;
                    blocks = self.parse_blocks(&local_name(&element), depth + 1)?;
                }
                Event::End(_) => return Ok(blocks),
                Event::Eof => return Err(self.reader.unexpected_eof("mc:AlternateContent")),
                _ => {}
            }
        }
    }

    fn parse_table(&mut self, depth: usize) -> Result<Table, DocxError> {
        let mut rows = Vec::new();
        self.parse_table_rows(&mut rows, "w:tbl", depth)?;
        Ok(Table { rows })
    }

    fn parse_table_rows(
        &mut self,
        rows: &mut Vec<TableRow>,
        container: &str,
        depth: usize,
    ) -> Result<(), DocxError> {
        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.parse_table_child(&StartTag::new(element), rows, depth)?
                }
                Event::End(_) => return Ok(()),
                Event::Eof => return Err(self.reader.unexpected_eof(container)),
                _ => {}
            }
        }
    }

    /// Handle one `Start` inside a `w:tbl` or inside a wrapper around one.
    ///
    /// The depth guard comes before the element name is looked at, because
    /// the fallback arm descends into wrapper elements.
    fn parse_table_child(
        &mut self,
        tag: &StartTag,
        rows: &mut Vec<TableRow>,
        depth: usize,
    ) -> Result<(), DocxError> {
        if depth >= MAX_DEPTH {
            return self.reader.skip_element(&tag.name);
        }
        match tag.name.as_str() {
            "tr" => rows.push(self.parse_table_row(depth + 1)?),
            "tblPr" | "tblPrEx" | "tblGrid" | "bookmarkStart" | "bookmarkEnd" => {
                self.reader.skip_element(&tag.name)?
            }
            // Rows are often wrapped in `w:sdt` or `w:customXml`.
            _ => self.parse_table_rows(rows, &tag.name, depth + 1)?,
        }
        Ok(())
    }

    fn parse_table_row(&mut self, depth: usize) -> Result<TableRow, DocxError> {
        let mut cells = Vec::new();
        loop {
            match self.reader.next()? {
                Event::Start(element) if depth >= MAX_DEPTH => {
                    self.reader.skip_element(&local_name(&element))?
                }
                Event::Start(element) if local_name(&element) == "tc" => {
                    cells.push(self.parse_table_cell(depth + 1)?)
                }
                // Anything else in a row is properties, not content.
                Event::Start(element) => self.reader.skip_element(&local_name(&element))?,
                Event::End(_) => return Ok(TableRow { cells }),
                Event::Eof => return Err(self.reader.unexpected_eof("w:tr")),
                _ => {}
            }
        }
    }

    fn parse_table_cell(&mut self, depth: usize) -> Result<TableCell, DocxError> {
        let mut cell = TableCell {
            blocks: Vec::new(),
            colspan: 1,
        };
        loop {
            match self.reader.next()? {
                Event::Start(element) => {
                    self.parse_cell_child(&StartTag::new(element), &mut cell, depth)?
                }
                Event::Empty(element) => push_empty_paragraph(&element, &mut cell.blocks),
                Event::End(_) => return Ok(cell),
                Event::Eof => return Err(self.reader.unexpected_eof("w:tc")),
                _ => {}
            }
        }
    }

    /// Handle one `Start` inside a `w:tc`.
    ///
    /// The depth guard comes before the element name is looked at, because
    /// the fallback arm descends into unknown elements.
    fn parse_cell_child(
        &mut self,
        tag: &StartTag,
        cell: &mut TableCell,
        depth: usize,
    ) -> Result<(), DocxError> {
        if depth >= MAX_DEPTH {
            return self.reader.skip_element(&tag.name);
        }
        match tag.name.as_str() {
            "tcPr" => cell.colspan = self.parse_cell_grid_span()?,
            "p" => cell
                .blocks
                .push(Block::Paragraph(self.parse_paragraph(depth + 1)?)),
            "tbl" => cell.blocks.push(Block::Table(self.parse_table(depth + 1)?)),
            _ => cell.blocks.extend(self.parse_blocks(&tag.name, depth + 1)?),
        }
        Ok(())
    }

    fn parse_cell_grid_span(&mut self) -> Result<u32, DocxError> {
        let mut span = 1u32;
        let mut elements = self.reader.scan("w:tcPr");
        while let Some(element) = elements.next_element()? {
            if local_name(&element) == "gridSpan" {
                span = grid_span_of(&element);
            }
        }
        Ok(span)
    }
}

/// Absorb one element of a `w:rPr` subtree. Everything this crate does not
/// map to semantic HTML -- fonts, colours, sizes -- is presentation the web
/// app supplies itself.
fn apply_run_property(formatting: &mut Formatting, element: &BytesStart) {
    match local_name(element).as_str() {
        "b" => formatting.bold = on_off_value(element),
        "i" => formatting.italic = on_off_value(element),
        "strike" | "dstrike" => formatting.strike = on_off_value(element),
        "u" => {
            formatting.underline = attribute(element, "val").is_none_or(|value| value != "none");
        }
        "vertAlign" => formatting.vertical_align = vertical_align_of(element),
        _ => {}
    }
}

fn vertical_align_of(element: &BytesStart) -> VerticalAlign {
    match attribute(element, "val").as_deref() {
        Some("superscript") => VerticalAlign::Superscript,
        Some("subscript") => VerticalAlign::Subscript,
        _ => VerticalAlign::Baseline,
    }
}

/// A `w:gridSpan` is a colspan; anything unreadable means one column.
fn grid_span_of(element: &BytesStart) -> u32 {
    attribute(element, "val")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1)
}

/// Append what an entity reference stands for, dropping the references this
/// crate cannot resolve rather than failing on them.
fn push_resolved_entity(text: &mut String, reference: &str) {
    if let Some(resolved) = resolve_entity(reference) {
        text.push_str(&resolved);
    }
}

/// A self-closing `<w:p/>` is an empty paragraph, not a missing one.
fn push_empty_paragraph(element: &BytesStart, blocks: &mut Vec<Block>) {
    if local_name(element) == "p" {
        blocks.push(Block::Paragraph(Paragraph::default()));
    }
}

/// Handle an inline-level element that arrived as a self-closing tag.
fn push_empty_inline(element: &BytesStart, out: &mut Vec<Inline>, formatting: Formatting) {
    match local_name(element).as_str() {
        "br" | "cr" => out.push(Inline::LineBreak),
        "tab" => push_text(out, "\t", formatting),
        "noBreakHyphen" => push_text(out, "-", formatting),
        _ => {}
    }
}
