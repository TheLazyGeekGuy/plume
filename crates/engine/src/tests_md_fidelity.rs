//! Plume: Word → Markdown fidelity, pinned on a synthetic report that holds what a real one
//! lost on conversion: numbered headings in custom styles, a Word table of contents, a grouped
//! diagram and an old-style text box with text, a table with merged cells and empty rows, prose
//! in "Arial Unicode MS", and comments starting at a paragraph's start. The package is written
//! here from ECMA-376 (no file made by Word); all text is invented.

#![cfg(not(target_arch = "wasm32"))]

use std::io::Write;

use crate::io;

const DOCUMENT: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
<w:p><w:pPr><w:pStyle w:val="TOCHeading"/></w:pPr><w:r><w:t xml:space="preserve">Contents</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="TOC1"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9062"/></w:tabs></w:pPr><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \o "1-3" \h \z \u </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:hyperlink w:anchor="_Toc100" w:history="1"><w:r><w:t xml:space="preserve">1.	Context and scope</w:t></w:r><w:r><w:t xml:space="preserve">	</w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Toc100 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t xml:space="preserve">2</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:hyperlink></w:p>
<w:p><w:pPr><w:pStyle w:val="TOC2"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9062"/></w:tabs></w:pPr><w:hyperlink w:anchor="_Toc101" w:history="1"><w:r><w:t xml:space="preserve">2.	Sources reviewed</w:t></w:r><w:r><w:t xml:space="preserve">	</w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Toc101 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t xml:space="preserve">3</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:hyperlink></w:p>
<w:p><w:pPr><w:pStyle w:val="TOC1"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9062"/></w:tabs></w:pPr><w:hyperlink w:anchor="_Toc102" w:history="1"><w:r><w:t xml:space="preserve">3.	Findings</w:t></w:r><w:r><w:t xml:space="preserve">	</w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Toc102 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t xml:space="preserve">4</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:hyperlink></w:p>
<w:p><w:pPr><w:pStyle w:val="TOC2"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9062"/></w:tabs></w:pPr><w:hyperlink w:anchor="_Toc103" w:history="1"><w:r><w:t xml:space="preserve">4.	Equipment and access</w:t></w:r><w:r><w:t xml:space="preserve">	</w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Toc103 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t xml:space="preserve">5</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:hyperlink><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="ReportTitle1"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="1" w:name="_Toc100"/><w:r><w:t xml:space="preserve">Context and scope</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>
<w:p><w:commentRangeStart w:id="0"/><w:r><w:t xml:space="preserve">The workshop opens </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">every Thursday</w:t></w:r><w:r><w:t xml:space="preserve"> at six.</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r><w:r><w:t xml:space="preserve"> Doors close at ten.</w:t></w:r></w:p>
<w:p><w:pPr><w:ind w:left="1440"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/></w:rPr><w:t xml:space="preserve">Members share the large table and the press.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Run </w:t></w:r><w:r><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/></w:rPr><w:t xml:space="preserve">make labels</w:t></w:r><w:r><w:t xml:space="preserve"> to print the shelf labels.</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="ReportTitle2"/><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="2" w:name="_Toc101"/><w:r><w:t xml:space="preserve">Sources reviewed</w:t></w:r><w:bookmarkEnd w:id="2"/></w:p>
<w:tbl><w:tblPr><w:tblW w:w="4500" w:type="dxa"/><w:tblBorders><w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/><w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Tool</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Booking</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Weekday</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Weekend</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:vMerge w:val="restart"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Press</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Members</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Everyone</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:vMerge/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Two hours</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">One hour</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Loom</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Members</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Members</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p></w:p></w:tc></w:tr></w:tbl>
<w:p><w:pPr><w:pStyle w:val="ReportTitle1"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="3" w:name="_Toc102"/><w:r><w:t xml:space="preserve">Findings</w:t></w:r><w:bookmarkEnd w:id="3"/></w:p>
<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3000000" cy="600000"/><wp:docPr id="5" name="Diagram"/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3000000" cy="600000"/><a:chOff x="0" y="0"/><a:chExt cx="3000000" cy="600000"/></a:xfrm></wpg:grpSpPr><wps:wsp><wps:cNvPr id="10" name="Box 0"/><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1400000" cy="600000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:txbx><w:txbxContent>
<w:p><w:r><w:t xml:space="preserve">Request a slot</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="11" name="Box 1"/><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="1500000" y="0"/><a:ext cx="1400000" cy="600000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:txbx><w:txbxContent>
<w:p><w:r><w:t xml:space="preserve">Confirm by mail</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp></wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
<w:p><w:commentRangeStart w:id="1"/><w:r><w:t xml:space="preserve">Most members prefer the evening sessions.</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p>
<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial Unicode MS" w:hAnsi="Arial Unicode MS"/></w:rPr><w:t xml:space="preserve">Prices are agreed together each spring.</w:t></w:r></w:p>
<w:p><w:r><w:pict><v:shape id="tb1" style="width:200pt;height:40pt" xmlns:v="urn:schemas-microsoft-com:vml"><v:textbox><w:txbxContent>
<w:p><w:r><w:t xml:space="preserve">Old style note box</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="ReportTitle2"/><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="4" w:name="_Toc103"/><w:r><w:t xml:space="preserve">Equipment and access</w:t></w:r><w:bookmarkEnd w:id="4"/></w:p>
<w:p><w:r><w:t xml:space="preserve">Keys are kept by the two coordinators.</w:t></w:r></w:p>
<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:body></w:document>"##;

const STYLES: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">

<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>

<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>

<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/></w:rPr></w:style>

<w:style w:type="paragraph" w:customStyle="1" w:styleId="ReportTitle1"><w:name w:val="Report Title 1"/><w:basedOn w:val="Heading1"/></w:style>

<w:style w:type="paragraph" w:customStyle="1" w:styleId="ReportTitle2"><w:name w:val="Report Title 2"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>

<w:style w:type="paragraph" w:styleId="TOC1"><w:name w:val="toc 1"/><w:basedOn w:val="Normal"/></w:style>

<w:style w:type="paragraph" w:styleId="TOC2"><w:name w:val="toc 2"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="220"/></w:pPr></w:style>

<w:style w:type="paragraph" w:styleId="TOCHeading"><w:name w:val="TOC Heading"/><w:basedOn w:val="Heading1"/><w:pPr><w:outlineLvl w:val="9"/></w:pPr></w:style>
</w:styles>"##;

const NUMBERING: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">

<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:pPr><w:ind w:left="1080" w:hanging="360"/></w:pPr></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1.%2."/><w:pPr><w:ind w:left="1440" w:hanging="720"/></w:pPr></w:lvl></w:abstractNum>

<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"##;

const COMMENTS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">

<w:comment w:id="0" w:author="Reviewer A" w:date="2026-03-02T10:00:00Z" w:initials="RA">
<w:p><w:r><w:t xml:space="preserve">Only during term time.</w:t></w:r></w:p></w:comment>

<w:comment w:id="1" w:author="Reviewer B" w:date="2026-03-03T11:30:00Z" w:initials="RB">
<w:p><w:r><w:t xml:space="preserve">Based on a small survey; treat with care.</w:t></w:r></w:p></w:comment>
</w:comments>"##;

const CONTENT_TYPES: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"##;

const ROOT_RELS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"##;

const DOC_RELS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"##;

fn report_docx() -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, text) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/_rels/document.xml.rels", DOC_RELS),
        ("word/document.xml", DOCUMENT),
        ("word/styles.xml", STYLES),
        ("word/numbering.xml", NUMBERING),
        ("word/comments.xml", COMMENTS),
    ] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(text.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn markdown() -> (String, Vec<String>) {
    let doc = io::open_bytes("report.docx", &report_docx()).unwrap();
    let b = wordcraft_formats::markdown::export_native(&doc, &Default::default());
    (b.text, b.report.lines())
}

/// Lines a Markdown reader takes as an indented code block: four spaces after a blank line,
/// outside a list.
fn indented_code(md: &str) -> Vec<String> {
    let mut prev_blank = true;
    let mut in_list = false;
    let mut out = Vec::new();
    for l in md.lines() {
        let item = l.starts_with("- ") || l.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if item {
            in_list = true;
        } else if !l.is_empty() && !l.starts_with(' ') {
            in_list = false;
        }
        if prev_blank && !in_list && l.starts_with("    ") {
            out.push(l.to_string());
        }
        prev_blank = l.is_empty();
    }
    out
}

#[test]
fn numbered_headings_in_custom_styles_stay_headings() {
    let (md, report) = markdown();
    for h in ["# Context and scope", "## Sources reviewed", "# Findings", "## Equipment and access"] {
        assert!(md.lines().any(|l| l == h), "{h} missing:\n{md}");
    }
    assert!(indented_code(&md).is_empty(), "prose read as code: {:?}\n{md}", indented_code(&md));
    assert!(!md.lines().any(|l| l.starts_with("1. ")), "a heading turned into a list item:\n{md}");
    assert!(report.iter().any(|l| l.contains("numbered headings")), "{report:?}");
}

#[test]
fn the_table_of_contents_links_to_the_headings() {
    let (md, _) = markdown();
    let headings: Vec<String> =
        md.lines().filter_map(|l| l.strip_prefix('#')).map(|r| wordcraft_formats::markdown::heading_slug(r.trim_start_matches('#'))).collect();
    let links: Vec<&str> = md
        .match_indices("](#")
        .map(|(i, _)| {
            let rest = &md[i + 3..];
            &rest[..rest.find(')').unwrap()]
        })
        .collect();
    assert_eq!(links.len(), 4, "{md}");
    for l in &links {
        assert!(headings.iter().any(|h| h == l), "#{l} has no heading: {headings:?}");
    }
    assert!(md.contains("- [Context and scope](#context-and-scope)\n    - [Sources reviewed](#sources-reviewed)"), "{md}");
    assert!(!md.contains("scope    2") && !md.contains("[1."), "page numbers and Word's numbers are gone:\n{md}");
}

#[test]
fn drawings_keep_their_text() {
    let (md, report) = markdown();
    for t in ["> Request a slot", "> Confirm by mail", "> Old style note box"] {
        assert!(md.contains(t), "{t} lost:\n{md}");
    }
    assert!(report.iter().any(|l| l.starts_with("2 × drawings' text")), "{report:?}");
}

#[test]
fn merged_cells_keep_their_associations() {
    let (md, _) = markdown();
    assert!(md.contains("| Tool | Booking – Weekday | Booking – Weekend |"), "{md}");
    assert!(md.contains("| Press | Members | Everyone |") && md.contains("| Press | Two hours | One hour |"), "{md}");
    assert!(!md.contains("|  |  |  |"), "empty grid rows dropped:\n{md}");
}

#[test]
fn only_code_fonts_make_code() {
    let (md, _) = markdown();
    assert!(md.contains("Run `make labels` to print"), "{md}");
    assert!(md.contains("\nPrices are agreed together each spring.\n"), "Arial Unicode MS is not a code font:\n{md}");
    assert!(md.contains("\nMembers share the large table"), "{md}");
    assert_eq!(md.matches('`').count(), 2, "{md}");
}

#[test]
fn comments_cover_their_whole_range_and_show() {
    let (md, _) = markdown();
    // The range starts at the paragraph's start; the line can't open with `<!--` (an HTML block).
    assert!(md.contains("\n&#8203;<!-- c1 -->The workshop opens **every Thursday** at six.<!-- /c1 --> Doors"), "{md}");
    assert!(md.contains("\n&#8203;<!-- c2 -->Most members prefer the evening sessions.<!-- /c2 -->"), "{md}");
    // Readers see them: author, date, text and the passage.
    assert!(md.contains("- **Reviewer A** · 2026-03-02 — Only during term time.\n    > The workshop opens every Thursday at six."), "{md}");
    assert!(md.contains("- **Reviewer B** · 2026-03-03 — Based on a small survey; treat with care."), "{md}");
    // Read back: the same ranges, no stray space, the readers' view dropped; written again, the same text.
    let doc = io::open_bytes("report.md", md.as_bytes()).unwrap();
    assert_eq!(doc.comments.len(), 2);
    let text = doc.plain_text(wordcraft_doc::StoryRef::Body);
    assert!(!text.contains('\u{200B}') && !text.contains("Reviewer A"), "{text}");
    let first =
        doc.body.iter().filter_map(|b| b.as_para()).find(|p| p.plain_text().starts_with("The workshop")).and_then(|p| p.objects.first().cloned());
    assert!(matches!(first, Some(wordcraft_doc::InlineObject::CommentStart { .. })), "the range starts at the paragraph's start: {first:?}");
    let again = wordcraft_formats::markdown::export_native(&doc, &Default::default()).text;
    assert_eq!(again, md);
}
