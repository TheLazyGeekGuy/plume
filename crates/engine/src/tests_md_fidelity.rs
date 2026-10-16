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
<w:p><w:pPr><w:pStyle w:val="TOC3"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9062"/></w:tabs></w:pPr><w:hyperlink w:anchor="_Toc104" w:history="1"><w:r><w:t xml:space="preserve">3.	Interviews</w:t></w:r><w:r><w:t xml:space="preserve">	</w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Toc104 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t xml:space="preserve">4</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:hyperlink></w:p>
<w:p><w:pPr><w:pStyle w:val="TOC1"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9062"/></w:tabs></w:pPr><w:hyperlink w:anchor="_Toc102" w:history="1"><w:r><w:t xml:space="preserve">4.	Findings</w:t></w:r><w:r><w:t xml:space="preserve">	</w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Toc102 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t xml:space="preserve">5</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:hyperlink></w:p>
<w:p><w:pPr><w:pStyle w:val="TOC2"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9062"/></w:tabs></w:pPr><w:hyperlink w:anchor="_Toc103" w:history="1"><w:r><w:t xml:space="preserve">5.	Equipment and access</w:t></w:r><w:r><w:t xml:space="preserve">	</w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Toc103 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t xml:space="preserve">6</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:hyperlink><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="ReportTitle1"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="1" w:name="_Toc100"/><w:r><w:t xml:space="preserve">Context and scope</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>
<w:p><w:commentRangeStart w:id="0"/><w:r><w:t xml:space="preserve">The workshop opens </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">every Thursday</w:t></w:r><w:r><w:t xml:space="preserve"> at six.</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r><w:r><w:t xml:space="preserve"> Doors close at ten.</w:t></w:r></w:p>
<w:p><w:pPr><w:ind w:left="1440"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/></w:rPr><w:t xml:space="preserve">Members share the large table and the press.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Run </w:t></w:r><w:r><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/></w:rPr><w:t xml:space="preserve">make labels</w:t></w:r><w:r><w:t xml:space="preserve"> to print the shelf labels.</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="ReportTitle2"/><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="2" w:name="_Toc101"/><w:r><w:t xml:space="preserve">Sources reviewed</w:t></w:r><w:bookmarkEnd w:id="2"/></w:p>
<w:p><w:pPr><w:pStyle w:val="ReportTitle3"/><w:numPr><w:ilvl w:val="2"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="5" w:name="_Toc104"/><w:r><w:t xml:space="preserve">Interviews</w:t></w:r><w:bookmarkEnd w:id="5"/></w:p>
<w:p><w:r><w:t xml:space="preserve">Twelve members were interviewed.  Most  answered twice. </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t xml:space="preserve">Noted </w:t></w:r><w:r><w:t xml:space="preserve"> later.</w:t></w:r></w:p>
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
<w:p><w:r><w:t xml:space="preserve">Coordinators:</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblBorders><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Name</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Contact</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Ana</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">0601</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">ana@example.org</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Ben</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">0602</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">ben@example.org</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:t xml:space="preserve">Opening hours:</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblW w:w="4500" w:type="dxa"/><w:tblBorders><w:top w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="750"/><w:gridCol w:w="750"/><w:gridCol w:w="750"/><w:gridCol w:w="750"/><w:gridCol w:w="750"/><w:gridCol w:w="750"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Room</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Hours</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Contact</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="4500" w:type="dxa"/><w:gridSpan w:val="6"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Morning slots</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/><w:vMerge w:val="restart"/></w:tcPr>
<w:p><w:commentRangeStart w:id="2"/><w:r><w:t xml:space="preserve">Print room</w:t></w:r><w:commentRangeEnd w:id="2"/><w:r><w:commentReference w:id="2"/></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">8–12</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Front desk</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/><w:vMerge/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">13–17</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Front desk</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/><w:vMerge w:val="restart"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Long description of the dye room that runs over several lines of text in the original</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">9–11</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Ana</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/><w:vMerge/></w:tcPr>
<w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">14–16</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t xml:space="preserve">Ana</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:pPr><w:pStyle w:val="ReportTitle1"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr><w:ind w:left="1080" w:hanging="360"/></w:pPr><w:bookmarkStart w:id="3" w:name="_Toc102"/><w:r><w:t xml:space="preserve">Findings</w:t></w:r><w:bookmarkEnd w:id="3"/></w:p>
<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3000000" cy="600000"/><wp:docPr id="5" name="Diagram"/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3000000" cy="1300000"/><a:chOff x="0" y="0"/><a:chExt cx="3000000" cy="1300000"/></a:xfrm></wpg:grpSpPr><wps:wsp><wps:cNvPr id="11" name="Box 1"/><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="1500000" y="0"/><a:ext cx="1400000" cy="600000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:txbx><w:txbxContent>
<w:p><w:r><w:t xml:space="preserve">Confirm by mail</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="10" name="Box 0"/><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="700000"/><a:ext cx="1400000" cy="600000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:txbx><w:txbxContent>
<w:p><w:r><w:t xml:space="preserve">Done: the slot is yours</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="10" name="Box 0"/><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1400000" cy="600000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:txbx><w:txbxContent>
<w:p><w:r><w:t xml:space="preserve">Request a slot</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp></wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
<w:p><w:commentRangeStart w:id="1"/><w:r><w:t xml:space="preserve">Most members prefer the evening sessions.</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p>
<w:p><w:r><w:drawing><wp:inline><wp:extent cx="3000000" cy="900000"/><wp:docPr id="7" name="Canvas"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"><lc:lockedCanvas xmlns:lc="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"><a:nvGrpSpPr><a:cNvPr id="0" name=""/><a:cNvGrpSpPr/></a:nvGrpSpPr><a:grpSpPr/><a:sp><a:nvSpPr><a:cNvPr id="2" name="Step 1"/><a:cNvSpPr/></a:nvSpPr><a:spPr/><a:txSp><a:txBody><a:bodyPr/><a:p><a:r><a:t>Book the room</a:t></a:r></a:p></a:txBody></a:txSp></a:sp><a:sp><a:nvSpPr><a:cNvPr id="3" name="Step 2"/><a:cNvSpPr/></a:nvSpPr><a:spPr/><a:txSp><a:txBody><a:bodyPr/><a:p><a:r><a:t>Collect the key</a:t></a:r><a:br/><a:r><a:t>at the desk</a:t></a:r></a:p></a:txBody></a:txSp></a:sp></lc:lockedCanvas></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
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

<w:style w:type="paragraph" w:styleId="TOC2"><w:name w:val="toc 2"/><w:basedOn w:val="TOC1"/><w:pPr><w:ind w:left="220"/></w:pPr></w:style>

<w:style w:type="paragraph" w:styleId="TOC3"><w:name w:val="toc 3"/><w:basedOn w:val="TOC2"/></w:style>

<w:style w:type="paragraph" w:customStyle="1" w:styleId="ReportTitle3"><w:name w:val="Report Title 3"/><w:basedOn w:val="ReportTitle2"/><w:pPr><w:outlineLvl w:val="2"/></w:pPr></w:style>

<w:style w:type="paragraph" w:styleId="TOCHeading"><w:name w:val="TOC Heading"/><w:basedOn w:val="Heading1"/><w:pPr><w:outlineLvl w:val="9"/></w:pPr></w:style>
</w:styles>"##;

const NUMBERING: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">

<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:pPr><w:ind w:left="1080" w:hanging="360"/></w:pPr></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1.%2."/><w:pPr><w:ind w:left="1440" w:hanging="720"/></w:pPr></w:lvl><w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1.%2.%3."/><w:pPr><w:ind w:left="2160" w:hanging="720"/></w:pPr></w:lvl></w:abstractNum>

<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"##;

const COMMENTS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">

<w:comment w:id="0" w:author="Reviewer A" w:date="2026-03-02T10:00:00Z" w:initials="RA">
<w:p><w:r><w:t xml:space="preserve">Only during term time.</w:t></w:r></w:p></w:comment>

<w:comment w:id="1" w:author="Reviewer B" w:date="2026-03-03T11:30:00Z" w:initials="RB">
<w:p><w:r><w:t xml:space="preserve">Based on a small survey; treat with care.</w:t></w:r></w:p></w:comment>

<w:comment w:id="2" w:author="Reviewer C" w:date="2026-03-04T09:00:00Z" w:initials="RC">
<w:p><w:r><w:t xml:space="preserve">Closed in August.</w:t></w:r></w:p></w:comment>
</w:comments>"##;

/// Spaces Markdown drops when reading: at a paragraph's or a cell's start and end, after a line
/// break, a paragraph of spaces only.
const SPACES: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:t xml:space="preserve">   Leading spaces here.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Trailing spaces here.   </w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Two  spaces and three   spaces.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Name</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t xml:space="preserve">value</w:t></w:r></w:p>
<w:p><w:r><w:tab/></w:r><w:r><w:t xml:space="preserve">Starts with a tab.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Price: 12 € and 5 %.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Plain </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve"> bold with spaces </w:t></w:r><w:r><w:t xml:space="preserve"> after.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Word </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t xml:space="preserve">italic </w:t></w:r><w:r><w:t xml:space="preserve">next.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Run </w:t></w:r><w:r><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/></w:rPr><w:t xml:space="preserve"> make all </w:t></w:r><w:r><w:t xml:space="preserve"> now.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Line one </w:t></w:r><w:r><w:br/></w:r><w:r><w:t xml:space="preserve"> line two.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">    </w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Before </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">   </w:t></w:r><w:r><w:t xml:space="preserve">after.</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Ideographic　space.</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblBorders><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:tc>
<w:p><w:r><w:t xml:space="preserve"> A </w:t></w:r></w:p></w:tc><w:tc>
<w:p><w:r><w:t xml:space="preserve">B  b</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc>
<w:p><w:r><w:t xml:space="preserve">  1</w:t></w:r></w:p></w:tc><w:tc>
<w:p><w:r><w:t xml:space="preserve">2 </w:t></w:r><w:r><w:br/></w:r><w:r><w:t xml:space="preserve"> 3</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:t xml:space="preserve">Ends with a break</w:t><w:br/></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Ends with a no-break space </w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblBorders><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:tc><w:p><w:r><w:t xml:space="preserve">Cell with a break</w:t><w:br/></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t xml:space="preserve">Cell with a no-break space </w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t xml:space="preserve">Two lines</w:t><w:br/><w:t xml:space="preserve">in a cell</w:t><w:br/></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t xml:space="preserve">x</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:sectPr/></w:body></w:document>"##;

const CONTENT_TYPES: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"##;

const ROOT_RELS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"##;

const DOC_RELS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"##;

/// Tables whose every row Word marks as a repeated header (`w:tblHeader` on the records too),
/// with comment ranges over several cells; a band row under a two-level header; and a locked canvas whose shapes the file lists out
/// of their drawn order.
const FLAGGED: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body>
<w:p><w:r><w:t xml:space="preserve">Records:</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/><w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Référence</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Titre</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:commentRangeStart w:id="0"/><w:r><w:t xml:space="preserve">Section A</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Dossier</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:commentRangeStart w:id="1"/><w:r><w:t xml:space="preserve">R01</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Document Alpha</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Objet Alpha</w:t></w:r><w:commentRangeEnd w:id="1"/><w:r><w:commentReference w:id="1"/></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">R02</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Document Beta</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Objet Beta</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">R03</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Document Gamma</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Objet Gamma</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:t xml:space="preserve">Band:</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/><w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Code</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Title</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Category A</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">A01</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Document Alpha</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Description Alpha</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">A02</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Document Beta</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Description Beta</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:t xml:space="preserve">Codes:</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/><w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Code</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Libellé</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">K01</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Poste 1</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">K02</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Poste 2</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">K03</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">Poste 3</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:drawing><wp:inline><wp:extent cx="4200000" cy="900000"/><wp:docPr id="7" name="Canvas"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"><lc:lockedCanvas xmlns:lc="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"><a:nvGrpSpPr><a:cNvPr id="0" name=""/><a:cNvGrpSpPr/></a:nvGrpSpPr><a:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="4200000" cy="900000"/><a:chOff x="0" y="0"/><a:chExt cx="4200000" cy="900000"/></a:xfrm></a:grpSpPr><a:grpSp><a:nvGrpSpPr><a:cNvPr id="1" name="Steps"/><a:cNvGrpSpPr/></a:nvGrpSpPr><a:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="4200000" cy="900000"/><a:chOff x="0" y="0"/><a:chExt cx="4200000" cy="900000"/></a:xfrm></a:grpSpPr><a:sp><a:nvSpPr><a:cNvPr id="10" name="Step 9"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="3600000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>9</a:t></a:r></a:p><a:p><a:r><a:t>Label 9</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="11" name="Step 6"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="2400000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>6</a:t></a:r></a:p><a:p><a:r><a:t>Label 6</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="12" name="Step 8"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="3200000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>8</a:t></a:r></a:p><a:p><a:r><a:t>Label 8</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="13" name="Step 2"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="800000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>2</a:t></a:r></a:p><a:p><a:r><a:t>Label 2</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="14" name="Step 3"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="1200000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>3</a:t></a:r></a:p><a:p><a:r><a:t>Label 3</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="15" name="Step 5"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="2000000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>5</a:t></a:r></a:p><a:p><a:r><a:t>Label 5</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="16" name="Step 4"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="1600000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>4</a:t></a:r></a:p><a:p><a:r><a:t>Label 4</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="17" name="Step 7"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="2800000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>7</a:t></a:r></a:p><a:p><a:r><a:t>Label 7</a:t></a:r></a:p></a:txBody></a:sp><a:sp><a:nvSpPr><a:cNvPr id="18" name="Step 1"/><a:cNvSpPr/></a:nvSpPr><a:spPr><a:xfrm><a:off x="400000" y="200000"/><a:ext cx="300000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></a:spPr><a:txBody><a:bodyPr/><a:p><a:r><a:t>1</a:t></a:r></a:p><a:p><a:r><a:t>Label 1</a:t></a:r></a:p></a:txBody></a:sp></a:grpSp></lc:lockedCanvas></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
<w:sectPr/></w:body></w:document>"##;

const FLAGGED_COMMENTS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="0" w:author="Reviewer D" w:date="2026-03-05T09:00:00Z"><w:p><w:r><w:t xml:space="preserve">Rename the section.</w:t></w:r></w:p></w:comment><w:comment w:id="1" w:author="Reviewer E" w:date="2026-03-06T09:00:00Z"><w:p><w:r><w:t xml:space="preserve">Check the first record.</w:t></w:r></w:p></w:comment></w:comments>"##;

fn flagged_docx() -> Vec<u8> {
    package(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/_rels/document.xml.rels", DOC_RELS),
        ("word/document.xml", FLAGGED),
        ("word/styles.xml", STYLES),
        ("word/numbering.xml", NUMBERING),
        ("word/comments.xml", FLAGGED_COMMENTS),
    ])
}

fn report_docx() -> Vec<u8> {
    package(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/_rels/document.xml.rels", DOC_RELS),
        ("word/document.xml", DOCUMENT),
        ("word/styles.xml", STYLES),
        ("word/numbering.xml", NUMBERING),
        ("word/comments.xml", COMMENTS),
    ])
}

fn package(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, text) in parts.iter().copied() {
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
    for h in ["# Context and scope", "## Sources reviewed", "### Interviews", "# Findings", "## Equipment and access"] {
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
    assert_eq!(links.len(), 5, "{md}");
    for l in &links {
        assert!(headings.iter().any(|h| h == l), "#{l} has no heading: {headings:?}");
    }
    // "toc 2" is based on "toc 1": its own level counts, so the contents go three levels deep.
    assert!(
        md.contains("- [Context and scope](#context-and-scope)\n    - [Sources reviewed](#sources-reviewed)\n        - [Interviews](#interviews)\n- [Findings](#findings)"),
        "{md}"
    );
    assert!(!md.contains("scope    2") && !md.contains("[1."), "page numbers and Word's numbers are gone:\n{md}");
}

#[test]
fn drawings_keep_their_text() {
    let (md, report) = markdown();
    // A group, Word 2007's locked canvas (DrawingML text), an old VML text box.
    // A group read as it shows (rows top to bottom, each left to right), not in the file's order.
    assert!(md.contains("> Request a slot\n\n> Confirm by mail\n\n> Done: the slot is yours"), "{md}");
    for t in ["> Book the room", "> Collect the key at the desk", "> Old style note box"] {
        assert!(md.contains(t), "{t} lost:\n{md}");
    }
    assert!(report.iter().any(|l| l.starts_with("3 × drawings' text")), "{report:?}");
}

#[test]
fn merged_cells_keep_their_associations() {
    let (md, _) = markdown();
    assert!(md.contains("| Tool | Booking – Weekday | Booking – Weekend |"), "{md}");
    assert!(md.contains("| Press | Members | Everyone |") && md.contains("| Press | Two hours | One hour |"), "{md}");
    assert!(!md.contains("|  |  |  |"), "empty grid rows dropped:\n{md}");
    // Six narrow grid columns read as three; a row over the whole width is written once; a
    // short label is repeated, a long text isn't; a copy carries no comment markers.
    assert!(md.contains("| Room | Hours | Contact |\n| --- | --- | --- |\n| Morning slots |  |  |\n"), "{md}");
    assert!(md.contains("| <!-- c2 -->Print room<!-- /c2 --> | 8–12 | Front desk |\n| Print room | 13–17 | Front desk |"), "{md}");
    assert_eq!((md.matches("<!-- c2 -->").count(), md.matches("<!-- /c2 -->").count()), (1, 1), "{md}");
    assert_eq!(md.matches("Long description of the dye room").count(), 1, "{md}");
    assert!(md.contains("\n|  | 14–16 | Ana |"), "{md}");
    // A wide header cell over a row of values: that row is the first record, not a header.
    assert!(md.contains("| Name | Contact | Contact |\n| --- | --- | --- |\n| Ana | 0601 | ana@example.org |\n| Ben |"), "{md}");
}

/// Written, read back and written again: the same text from the first save on.
#[test]
fn the_first_save_is_already_stable() {
    let spaces = package(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        (
            "word/_rels/document.xml.rels",
            r##"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"##,
        ),
        ("word/document.xml", SPACES),
    ]);
    for (name, bytes) in [("report.docx", report_docx()), ("spaces.docx", spaces)] {
        let doc = io::open_bytes(name, &bytes).unwrap();
        let first = wordcraft_formats::markdown::export_native(&doc, &Default::default()).text;
        let back = io::open_bytes("copy.md", first.as_bytes()).unwrap();
        let second = wordcraft_formats::markdown::export_native(&back, &Default::default()).text;
        assert_eq!(second, first, "{name}");
        if name == "spaces.docx" {
            // A line break closing a paragraph or a cell has no Markdown form (`\` before a
            // blank line is a backslash): it goes. A no-break space at the end is text, kept.
            assert!(first.contains("\nEnds with a break\n\nEnds with a no-break space\u{a0}\n"), "{first}");
            assert!(
                first.contains("| Cell with a break | Cell with a no-break space\u{a0} |\n| --- | --- |\n| Two lines in a cell | x |\n"),
                "{first}"
            );
            assert!(!first.contains("\\\n\n") && !first.contains("\\ |"), "no break left dangling:\n{first}");
        }
    }
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
    assert!(md.contains("\n&#8203;<!-- c3 -->Most members prefer the evening sessions.<!-- /c3 -->"), "{md}");
    // Readers see them: author, date, text and the passage.
    assert!(md.contains("- **Reviewer A** · 2026-03-02 — Only during term time.\n    > The workshop opens every Thursday at six."), "{md}");
    assert!(md.contains("- **Reviewer B** · 2026-03-03 — Based on a small survey; treat with care."), "{md}");
    // Read back: the same ranges, no stray space, the readers' view dropped; written again, the same text.
    let doc = io::open_bytes("report.md", md.as_bytes()).unwrap();
    assert_eq!(doc.comments.len(), 3);
    let text = doc.plain_text(wordcraft_doc::StoryRef::Body);
    assert!(!text.contains('\u{200B}') && !text.contains("Reviewer A"), "{text}");
    let first =
        doc.body.iter().filter_map(|b| b.as_para()).find(|p| p.plain_text().starts_with("The workshop")).and_then(|p| p.objects.first().cloned());
    assert!(matches!(first, Some(wordcraft_doc::InlineObject::CommentStart { .. })), "the range starts at the paragraph's start: {first:?}");
    let again = wordcraft_formats::markdown::export_native(&doc, &Default::default()).text;
    assert_eq!(again, md);
}

/// Every row flagged as a repeated header says nothing about the table's shape: the records
/// stay rows, one each, under a one-row header.
#[test]
fn repeated_header_flags_on_every_row_do_not_swallow_the_records() {
    let doc = io::open_bytes("flagged.docx", &flagged_docx()).unwrap();
    let md = wordcraft_formats::markdown::export_native(&doc, &Default::default()).text;
    assert!(md.contains("| Référence | Titre | Titre |\n| --- | --- | --- |\n| <!-- c1 -->Section A | Dossier<!-- /c1 --> |  |\n"), "{md}");
    for r in [
        "| <!-- c2 -->R01 | Document Alpha | Objet Alpha<!-- /c2 --> |",
        "| R02 | Document Beta | Objet Beta |",
        "| R03 | Document Gamma | Objet Gamma |",
    ] {
        assert!(md.contains(&format!("\n{r}\n")), "{r} is a row of its own:\n{md}");
    }
    assert!(md.contains("| Code | Libellé |\n| --- | --- |\n| K01 | Poste 1 |\n| K02 | Poste 2 |\n| K03 | Poste 3 |\n"), "{md}");
    // A band row with an empty first cell under the header: the record under it is a record.
    assert!(
        md.contains("| Code | Title – Category A | Title – Category A |\n| --- | --- | --- |\n| A01 | Document Alpha | Description Alpha |\n| A02 | Document Beta | Description Beta |\n"),
        "{md}"
    );
}

/// A comment over several cells covers the same text after export: the markers keep their
/// order, so the ranges neither nest nor grow.
#[test]
fn comments_over_several_cells_cover_the_same_text() {
    let doc = io::open_bytes("flagged.docx", &flagged_docx()).unwrap();
    let md = wordcraft_formats::markdown::export_native(&doc, &Default::default()).text;
    assert!(md.contains("Rename the section.\n    > Section A Dossier\n"), "{md}");
    assert!(md.contains("Check the first record.\n    > R01 Document Alpha Objet Alpha\n"), "{md}");
    let order: Vec<&str> = ["<!-- c1 -->", "<!-- /c1 -->", "<!-- c2 -->", "<!-- /c2 -->"]
        .into_iter()
        .map(|m| (md.find(m).unwrap(), m))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect();
    assert_eq!(order, ["<!-- c1 -->", "<!-- /c1 -->", "<!-- c2 -->", "<!-- /c2 -->"], "{md}");
    // Read back, the ranges cover the same text (the readers' view is rebuilt from them).
    let back = io::open_bytes("flagged.md", md.as_bytes()).unwrap();
    assert_eq!(back.comments.len(), 2);
    let again = wordcraft_formats::markdown::export_native(&back, &Default::default()).text;
    assert_eq!(again, md);
}

/// A locked canvas's shapes read in the drawing's order (by their places), each shape's lines
/// together, not in the file's order.
#[test]
fn a_locked_canvas_reads_in_the_drawn_order() {
    let doc = io::open_bytes("flagged.docx", &flagged_docx()).unwrap();
    let md = wordcraft_formats::markdown::export_native(&doc, &Default::default()).text;
    let quotes: Vec<&str> = md.lines().filter(|l| l.starts_with("> ")).collect();
    let expected: Vec<String> = (1..=9).flat_map(|n| [format!("> {n}"), format!("> Label {n}")]).collect();
    assert_eq!(quotes, expected, "{md}");
}
