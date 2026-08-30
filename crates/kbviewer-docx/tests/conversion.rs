//! End-to-end conversion tests, driven by real `.docx` packages that the
//! `fixture` module builds in memory.

mod fixture;

use fixture::{
    list_paragraph, paragraph, styled_paragraph, DocxBuilder, NAMESPACES, NUMBERING, PNG_BYTES,
    STYLES,
};
use kbviewer_docx::{convert, extract_media, DocxError};

/// Convert a body, with the standard styles and numbering parts present.
fn convert_body(body: &str) -> kbviewer_docx::DocxDocument {
    let bytes = DocxBuilder::new()
        .part("word/styles.xml", STYLES)
        .part("word/numbering.xml", NUMBERING)
        .document_rels(
            r#"<Relationship Id="rIdS" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
               <Relationship Id="rIdN" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#,
        )
        .body(body)
        .build();
    convert(&bytes, "/media").expect("conversion should succeed")
}

// ---------------------------------------------------------------- headings

#[test]
fn maps_heading_styles_to_heading_elements() {
    let body = [
        styled_paragraph("Heading1", "Chapter"),
        styled_paragraph("Heading2", "Section"),
        styled_paragraph("Heading3", "Subsection"),
        styled_paragraph("Titre4", "Localised style id"),
        styled_paragraph("Quote", "Not a heading"),
    ]
    .concat();

    let html = convert_body(&body).html;

    assert!(html.contains("<h1>Chapter</h1>"), "{html}");
    assert!(html.contains("<h2>Section</h2>"), "{html}");
    assert!(html.contains("<h3>Subsection</h3>"), "{html}");
    // Resolved through the style's `w:name`, not its id.
    assert!(html.contains("<h4>Localised style id</h4>"), "{html}");
    assert!(html.contains("<p>Not a heading</p>"), "{html}");
}

#[test]
fn reads_heading_levels_from_the_style_id_when_there_is_no_styles_part() {
    let body = [
        styled_paragraph("Heading1", "Id style"),
        styled_paragraph("heading 2", "Spaced style"),
        styled_paragraph("Heading-3", "Hyphenated style"),
    ]
    .concat();

    let bytes = DocxBuilder::new().body(&body).build();
    let html = convert(&bytes, "/media").unwrap().html;

    assert!(html.contains("<h1>Id style</h1>"), "{html}");
    assert!(html.contains("<h2>Spaced style</h2>"), "{html}");
    assert!(html.contains("<h3>Hyphenated style</h3>"), "{html}");
}

#[test]
fn a_character_style_named_like_a_heading_does_not_make_a_heading() {
    let body = styled_paragraph("Heading1Char", "Body text");
    let html = convert_body(&body).html;
    assert!(html.contains("<p>Body text</p>"), "{html}");
}

// -------------------------------------------------------------- formatting

#[test]
fn merges_adjacent_runs_that_share_formatting() {
    // Word splits runs at spell-check boundaries, often mid-word.
    let body = "<w:p>\
        <w:r><w:rPr><w:b/></w:rPr><w:t>Bo</w:t></w:r>\
        <w:r><w:rPr><w:b/></w:rPr><w:t>ld</w:t></w:r>\
        <w:r><w:t xml:space=\"preserve\"> then plain</w:t></w:r>\
        </w:p>";

    let html = convert_body(body).html;

    assert_eq!(html.trim(), "<p><strong>Bold</strong> then plain</p>");
    assert_eq!(html.matches("<strong>").count(), 1);
}

#[test]
fn maps_run_properties_to_semantic_tags() {
    let body = "<w:p>\
        <w:r><w:rPr><w:i/></w:rPr><w:t>it</w:t></w:r>\
        <w:r><w:rPr><w:u w:val=\"single\"/></w:rPr><w:t>un</w:t></w:r>\
        <w:r><w:rPr><w:strike/></w:rPr><w:t>st</w:t></w:r>\
        <w:r><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr><w:t>up</w:t></w:r>\
        <w:r><w:rPr><w:vertAlign w:val=\"subscript\"/></w:rPr><w:t>dn</w:t></w:r>\
        <w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>both</w:t></w:r>\
        </w:p>";

    let html = convert_body(body).html;

    assert!(html.contains("<em>it</em>"), "{html}");
    assert!(html.contains("<u>un</u>"), "{html}");
    assert!(html.contains("<s>st</s>"), "{html}");
    assert!(html.contains("<sup>up</sup>"), "{html}");
    assert!(html.contains("<sub>dn</sub>"), "{html}");
    assert!(html.contains("<strong><em>both</em></strong>"), "{html}");
    assert!(
        !html.contains("style="),
        "output must carry no inline styles"
    );
}

#[test]
fn honours_toggles_that_are_explicitly_off() {
    let body = "<w:p>\
        <w:r><w:rPr><w:b w:val=\"0\"/><w:u w:val=\"none\"/></w:rPr><w:t>plain</w:t></w:r>\
        </w:p>";

    let html = convert_body(body).html;

    assert_eq!(html.trim(), "<p>plain</p>");
}

#[test]
fn honours_xml_space_on_text_runs() {
    let body = "<w:p>\
        <w:r><w:t xml:space=\"preserve\">kept </w:t></w:r>\
        <w:r><w:t>  trimmed  </w:t></w:r>\
        </w:p>";

    let document = convert_body(body);

    assert_eq!(document.html.trim(), "<p>kept trimmed</p>");
    assert_eq!(document.text, "kept trimmed");
}

#[test]
fn renders_breaks_and_tabs() {
    let body = "<w:p><w:r><w:t>a</w:t><w:br/><w:t>b</w:t><w:tab/><w:t>c</w:t></w:r></w:p>";
    let document = convert_body(body);

    assert!(document.html.contains("a<br>b\tc"), "{}", document.html);
    assert_eq!(document.text, "a\nb\tc");
}

// ------------------------------------------------------------------- lists

#[test]
fn nests_ordered_lists() {
    let body = [
        list_paragraph("2", 0, "one"),
        list_paragraph("2", 1, "one a"),
        list_paragraph("2", 0, "two"),
    ]
    .concat();

    let html = convert_body(&body).html.replace('\n', "");

    assert_eq!(
        html,
        "<ol><li>one<ol><li>one a</li></ol></li><li>two</li></ol>"
    );
}

#[test]
fn nests_unordered_lists_and_closes_them_before_body_text() {
    let body = [
        list_paragraph("1", 0, "alpha"),
        list_paragraph("1", 1, "alpha one"),
        list_paragraph("1", 2, "alpha one i"),
        list_paragraph("1", 0, "beta"),
        paragraph("after"),
    ]
    .concat();

    let html = convert_body(&body).html.replace('\n', "");

    assert_eq!(
        html,
        "<ul><li>alpha<ul><li>alpha one<ul><li>alpha one i</li></ul></li></ul></li>\
         <li>beta</li></ul><p>after</p>"
    );
}

#[test]
fn a_change_of_marker_style_starts_a_new_list() {
    let body = [
        list_paragraph("1", 0, "bullet"),
        list_paragraph("2", 0, "number"),
    ]
    .concat();

    let html = convert_body(&body).html.replace('\n', "");

    assert_eq!(html, "<ul><li>bullet</li></ul><ol><li>number</li></ol>");
}

#[test]
fn numbering_id_zero_is_not_a_list() {
    let body = list_paragraph("0", 0, "plain paragraph");
    let html = convert_body(&body).html;
    assert_eq!(html.trim(), "<p>plain paragraph</p>");
}

// ------------------------------------------------------------------ tables

#[test]
fn renders_tables_including_multi_paragraph_and_spanning_cells() {
    let body = format!(
        "<w:tbl><w:tblPr><w:tblStyle w:val=\"Grid\"/></w:tblPr>\
         <w:tblGrid><w:gridCol w:w=\"100\"/><w:gridCol w:w=\"100\"/></w:tblGrid>\
         <w:tr><w:tc>{}</w:tc><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr>{}{}</w:tc></w:tr>\
         <w:tr><w:tc>{}</w:tc></w:tr></w:tbl>",
        paragraph("A"),
        paragraph("B one"),
        paragraph("B two"),
        paragraph("C"),
    );

    let document = convert_body(&body);
    let html = document.html.replace('\n', "");

    assert!(html.starts_with("<table><tr><td>A</td>"), "{html}");
    assert!(
        html.contains("<td colspan=\"2\"><p>B one</p><p>B two</p></td>"),
        "{html}"
    );
    assert!(html.ends_with("<tr><td>C</td></tr></table>"), "{html}");
    assert_eq!(document.text, "A\nB one\nB two\nC");
}

#[test]
fn renders_a_table_nested_in_a_cell() {
    let inner = format!(
        "<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>",
        paragraph("deep")
    );
    let body = format!("<w:tbl><w:tr><w:tc>{inner}</w:tc></w:tr></w:tbl>");

    let html = convert_body(&body).html.replace('\n', "");

    assert_eq!(
        html,
        "<table><tr><td><table><tr><td>deep</td></tr></table></td></tr></table>"
    );
}

// ------------------------------------------------------ images and links

/// A body with one hyperlink and one inline image, plus the relationships
/// they need.
fn linked_document() -> Vec<u8> {
    let body = format!(
        "{}{}",
        "<w:p><w:hyperlink r:id=\"rId9\"><w:r><w:t>Example site</w:t></w:r></w:hyperlink></w:p>",
        "<w:p><w:r><w:drawing><wp:inline>\
           <wp:docPr id=\"1\" name=\"Picture 1\" descr=\"A photo of a cat\"/>\
           <a:graphic><a:graphicData><pic:pic><pic:blipFill>\
             <a:blip r:embed=\"rId8\"/>\
           </pic:blipFill></pic:pic></a:graphicData></a:graphic>\
         </wp:inline></w:drawing></w:r></w:p>",
    );

    DocxBuilder::new()
        .document_rels(
            r#"<Relationship Id="rId8" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
               <Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/page?a=1&amp;b=2" TargetMode="External"/>"#,
        )
        .binary("word/media/image1.png", PNG_BYTES)
        .body(&body)
        .build()
}

#[test]
fn renders_images_and_external_hyperlinks() {
    let document = convert(&linked_document(), "/files/doc7/media").unwrap();

    assert!(
        document.html.contains(
            "<a href=\"https://example.com/page?a=1&amp;b=2\" rel=\"noopener noreferrer\">Example site</a>"
        ),
        "{}",
        document.html
    );
    assert!(
        document
            .html
            .contains("<img src=\"/files/doc7/media/rId8\" alt=\"A photo of a cat\">"),
        "{}",
        document.html
    );

    assert_eq!(document.media.len(), 1);
    assert_eq!(document.media[0].rel_id, "rId8");
    assert_eq!(document.media[0].zip_path, "word/media/image1.png");
    assert_eq!(document.media[0].mime, "image/png");
}

#[test]
fn media_base_with_a_trailing_slash_does_not_double_up() {
    let document = convert(&linked_document(), "/media/").unwrap();
    assert!(
        document.html.contains("src=\"/media/rId8\""),
        "{}",
        document.html
    );
}

#[test]
fn extracts_media_bytes_on_demand() {
    let (bytes, mime) = extract_media(&linked_document(), "rId8").unwrap();
    assert_eq!(bytes, PNG_BYTES);
    assert_eq!(mime, "image/png");
}

#[test]
fn extracting_an_unknown_relationship_is_an_error() {
    assert!(matches!(
        extract_media(&linked_document(), "rId404"),
        Err(DocxError::MissingPart(_))
    ));
    // A hyperlink relationship points at a URL, not at a part.
    assert!(extract_media(&linked_document(), "rId9").is_err());
}

#[test]
fn an_image_whose_relationship_is_missing_is_dropped_rather_than_broken() {
    let body = "<w:p><w:r><w:drawing><wp:inline><a:blip r:embed=\"rIdGone\"/></wp:inline></w:drawing></w:r>\
                <w:r><w:t>caption</w:t></w:r></w:p>";
    let document = convert_body(body);

    assert!(!document.html.contains("<img"), "{}", document.html);
    assert!(document.html.contains("caption"), "{}", document.html);
    assert!(document.media.is_empty());
}

#[test]
fn an_image_offered_in_two_alternate_branches_is_rendered_once() {
    let body = "<w:p><w:r><mc:AlternateContent>\
        <mc:Choice Requires=\"wps\"><w:drawing><wp:inline><a:blip r:embed=\"rId8\"/></wp:inline></w:drawing></mc:Choice>\
        <mc:Fallback><w:pict><v:shape><v:imagedata r:id=\"rId8\"/></v:shape></w:pict></mc:Fallback>\
        </mc:AlternateContent></w:r></w:p>";

    let bytes = DocxBuilder::new()
        .document_rels(
            r#"<Relationship Id="rId8" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
        )
        .binary("word/media/image1.png", PNG_BYTES)
        .body(body)
        .build();
    let document = convert(&bytes, "/media").unwrap();

    assert_eq!(
        document.html.matches("<img").count(),
        1,
        "{}",
        document.html
    );
    assert_eq!(document.media.len(), 1);
}

#[test]
fn internal_anchors_link_without_a_noopener_rel() {
    let body =
        "<w:p><w:hyperlink w:anchor=\"section-2\"><w:r><w:t>Jump</w:t></w:r></w:hyperlink></w:p>";
    let html = convert_body(body).html;

    assert!(html.contains("<a href=\"#section-2\">Jump</a>"), "{html}");
}

#[test]
fn refuses_to_emit_a_script_url_as_a_link() {
    let body =
        "<w:p><w:hyperlink r:id=\"rIdEvil\"><w:r><w:t>Click me</w:t></w:r></w:hyperlink></w:p>";
    let bytes = DocxBuilder::new()
        .document_rels(
            r#"<Relationship Id="rIdEvil" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="javascript:alert(1)" TargetMode="External"/>"#,
        )
        .body(body)
        .build();

    let html = convert(&bytes, "/media").unwrap().html;

    assert!(!html.contains("javascript"), "{html}");
    // The link text is still readable.
    assert!(html.contains("Click me"), "{html}");
}

// ---------------------------------------------------------------- escaping

#[test]
fn escapes_html_metacharacters_in_document_text() {
    // As it appears in the XML: the author typed `<script>alert("x") & 'y'`.
    let body = paragraph("&lt;script&gt;alert(&quot;x&quot;) &amp; &apos;y&apos;&lt;/script&gt;");
    let document = convert_body(&body);

    assert!(
        !document.html.contains("<script>"),
        "author text must never become markup: {}",
        document.html
    );
    assert_eq!(
        document.html.trim(),
        "<p>&lt;script&gt;alert(&quot;x&quot;) &amp; &#39;y&#39;&lt;/script&gt;</p>"
    );
    // The search index gets the text the author actually typed.
    assert_eq!(document.text, "<script>alert(\"x\") & 'y'</script>");
}

#[test]
fn escapes_quotes_that_would_break_out_of_an_attribute() {
    let body = "<w:p><w:r><w:drawing><wp:inline>\
        <wp:docPr id=\"1\" name=\"p\" descr=\"&quot; onerror=alert(1) x=&quot;\"/>\
        <a:blip r:embed=\"rId8\"/></wp:inline></w:drawing></w:r></w:p>";

    let bytes = DocxBuilder::new()
        .document_rels(
            r#"<Relationship Id="rId8" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
        )
        .binary("word/media/image1.png", PNG_BYTES)
        .body(body)
        .build();

    let html = convert(&bytes, "/media").unwrap().html;

    // The quotes are neutralised, so the payload stays inside `alt`.
    assert!(!html.contains("alt=\"\" onerror"), "{html}");
    assert!(
        html.contains("alt=\"&quot; onerror=alert(1) x=&quot;\">"),
        "{html}"
    );
}

// ------------------------------------------------------- degrading safely

#[test]
fn an_empty_document_converts_to_nothing() {
    let document = convert_body("");

    assert_eq!(document.html, "");
    assert_eq!(document.text, "");
    assert!(document.media.is_empty());
}

#[test]
fn an_empty_paragraph_survives_as_an_empty_paragraph() {
    let document = convert_body("<w:p/><w:p><w:pPr/></w:p>");
    assert_eq!(document.html, "<p></p>\n<p></p>\n");
    assert_eq!(document.text, "");
}

#[test]
fn unknown_block_elements_keep_the_paragraphs_inside_them() {
    // A structured document tag, which this crate does not model.
    let body = format!(
        "<w:sdt><w:sdtPr><w:alias w:val=\"Ignored\"/></w:sdtPr><w:sdtContent>{}</w:sdtContent></w:sdt>",
        paragraph("inside a content control")
    );

    let html = convert_body(&body).html;

    assert!(html.contains("<p>inside a content control</p>"), "{html}");
}

#[test]
fn unknown_inline_elements_keep_their_text_and_formatting() {
    let body = "<w:p><w:ins w:author=\"someone\">\
        <w:smartTag w:element=\"place\">\
          <w:r><w:rPr><w:b/></w:rPr><w:t>tracked insertion</w:t></w:r>\
        </w:smartTag></w:ins></w:p>";

    let html = convert_body(body).html;

    assert_eq!(html.trim(), "<p><strong>tracked insertion</strong></p>");
}

#[test]
fn deleted_text_is_not_part_of_what_the_document_says() {
    let body = "<w:p><w:del><w:r><w:delText>gone</w:delText></w:r></w:del>\
                <w:r><w:t>kept</w:t></w:r></w:p>";

    let document = convert_body(body);

    assert_eq!(document.html.trim(), "<p>kept</p>");
    assert!(!document.text.contains("gone"));
}

#[test]
fn pathologically_nested_tables_are_bounded_rather_than_fatal() {
    let body = format!(
        "{}{}{}",
        "<w:tbl><w:tr><w:tc>".repeat(400),
        paragraph("buried"),
        "</w:tc></w:tr></w:tbl>".repeat(400)
    );

    let bytes = DocxBuilder::new().body(&body).build();
    assert!(convert(&bytes, "/media").is_ok());
}

#[test]
fn pathologically_nested_runs_are_bounded_rather_than_fatal() {
    let body = format!(
        "<w:p>{}<w:t>buried</w:t>{}</w:p>",
        "<w:r>".repeat(2000),
        "</w:r>".repeat(2000)
    );

    let bytes = DocxBuilder::new().body(&body).build();
    assert!(convert(&bytes, "/media").is_ok());
}

#[test]
fn pathologically_nested_hyperlinks_are_bounded_rather_than_fatal() {
    let body = format!(
        "<w:p>{}<w:r><w:t>buried</w:t></w:r>{}</w:p>",
        "<w:hyperlink w:anchor=\"a\">".repeat(2000),
        "</w:hyperlink>".repeat(2000)
    );

    let bytes = DocxBuilder::new().body(&body).build();
    assert!(convert(&bytes, "/media").is_ok());
}

#[test]
fn pathological_nesting_is_bounded_rather_than_fatal() {
    let body = format!(
        "{}{}{}",
        "<w:sdt>".repeat(500),
        paragraph("buried"),
        "</w:sdt>".repeat(500)
    );

    // The point is that this returns rather than overflowing the stack.
    let bytes = DocxBuilder::new().body(&body).build();
    assert!(convert(&bytes, "/media").is_ok());
}

// ------------------------------------------------------------ text output

#[test]
fn plain_text_is_one_line_per_paragraph() {
    let body = [
        styled_paragraph("Heading1", "Title"),
        paragraph("First."),
        "<w:p/>".to_string(),
        paragraph("Second."),
        list_paragraph("1", 0, "Bullet."),
    ]
    .concat();

    let document = convert_body(&body);

    assert_eq!(document.text, "Title\nFirst.\nSecond.\nBullet.");
}

// ----------------------------------------------- malformed input is an Err

#[test]
fn bytes_that_are_not_a_zip_are_an_error() {
    assert!(matches!(
        convert(b"this is not a zip file at all", "/media"),
        Err(DocxError::InvalidZip(_))
    ));
    assert!(matches!(
        convert(&[], "/media"),
        Err(DocxError::InvalidZip(_))
    ));
}

#[test]
fn a_truncated_package_is_an_error() {
    let mut bytes = convert_body_bytes(&paragraph("hello"));
    bytes.truncate(bytes.len() / 2);

    assert!(convert(&bytes, "/media").is_err());
    assert!(extract_media(&bytes, "rId8").is_err());
}

#[test]
fn a_package_without_a_main_document_is_an_error() {
    let bytes = DocxBuilder::new().build();
    assert!(matches!(
        convert(&bytes, "/media"),
        Err(DocxError::MissingPart(part)) if part == "word/document.xml"
    ));

    let empty_zip = DocxBuilder::bare().build();
    assert!(matches!(
        convert(&empty_zip, "/media"),
        Err(DocxError::MissingPart(_))
    ));
}

#[test]
fn truncated_xml_is_an_error() {
    let truncated = format!("<w:document{NAMESPACES}><w:body><w:p><w:r><w:t>hello");
    let bytes = DocxBuilder::new()
        .part("word/document.xml", &truncated)
        .build();

    assert!(matches!(
        convert(&bytes, "/media"),
        Err(DocxError::MalformedXml { .. })
    ));
}

#[test]
fn mismatched_end_tags_are_an_error() {
    let unbalanced = format!("<w:document{NAMESPACES}><w:body><w:p></w:tbl></w:body></w:document>");
    let bytes = DocxBuilder::new()
        .part("word/document.xml", &unbalanced)
        .build();

    assert!(matches!(
        convert(&bytes, "/media"),
        Err(DocxError::MalformedXml { .. })
    ));
}

#[test]
fn a_document_part_that_is_not_utf8_is_an_error() {
    let bytes = DocxBuilder::new()
        .binary("word/document.xml", &[0xff, 0xfe, 0x00, 0x41])
        .build();

    assert!(matches!(
        convert(&bytes, "/media"),
        Err(DocxError::MalformedXml { .. })
    ));
}

#[test]
fn unreadable_styles_and_numbering_cost_only_their_own_features() {
    let bytes = DocxBuilder::new()
        .part("word/styles.xml", "<w:styles><w:style")
        .part("word/numbering.xml", "<w:numbering")
        .body(&paragraph("still here"))
        .build();

    let document = convert(&bytes, "/media").expect("the body still renders");
    assert!(document.html.contains("still here"), "{}", document.html);
}

fn convert_body_bytes(body: &str) -> Vec<u8> {
    DocxBuilder::new().body(body).build()
}
