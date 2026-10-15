//! Plume: old Word documents lay pages out in tables, often tables inside tables, which
//! Markdown can't hold (it flattens them into one cell). A model reads such a table and says
//! what it is — a layout (a form, a letterhead) or data — and rewrites it as headings,
//! paragraphs, lists and plain tables. Plume then checks the rewrite word for word: every word of
//! the table is still there, and no word was made up; otherwise the table stays as it was.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use wordcraft_doc::{Block, Document, InlineObject, Paragraph, Table, props::VMerge};

/// Tables nested deeper than this aren't sent (the request would be too long to be useful).
pub const MAX_DEPTH: usize = 6;
/// Largest table description sent to a model, in characters (about 10,000 tokens).
pub const MAX_CHARS: usize = 40_000;

/// A table in the body worth restructuring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Its index in the document's body.
    pub index: usize,
    pub rows: usize,
    pub cols: usize,
    /// 1 for a plain table, 2 with tables in its cells, …
    pub depth: usize,
    /// Cells span several columns or rows.
    pub merged: bool,
    pub words: usize,
    /// Why it can't be sent, if it can't (pictures inside, too large…).
    pub blocked: Option<String>,
}

fn depth(t: &Table, d: usize) -> usize {
    if d > MAX_DEPTH {
        return d;
    }
    let inner = t.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.blocks).filter_map(|b| b.as_table()).map(|i| depth(i, d + 1)).max();
    inner.unwrap_or(d)
}

fn merged(t: &Table) -> bool {
    t.rows.iter().flat_map(|r| &r.cells).any(|c| c.span() > 1 || c.props.vmerge != VMerge::None)
}

/// Every paragraph of the table, nested tables included (bounded by [`MAX_DEPTH`]).
fn each_para<'a>(t: &'a Table, d: usize, f: &mut dyn FnMut(&'a wordcraft_doc::Paragraph)) {
    if d > MAX_DEPTH {
        return;
    }
    for c in t.rows.iter().flat_map(|r| &r.cells) {
        for b in &c.blocks {
            match &**b {
                Block::Para(p) => f(p),
                Block::Table(i) => each_para(i, d + 1, f),
            }
        }
    }
}

/// What the rewrite can't carry: content that isn't text. Bookmarks, content controls and
/// fields (their result is kept as text, as Markdown keeps it) go without loss of words.
fn object_kind(o: &InlineObject) -> Option<&'static str> {
    match o {
        InlineObject::Image { .. } | InlineObject::Graphic { .. } | InlineObject::Shape { .. } | InlineObject::Group { .. } => Some("pictures"),
        InlineObject::NoteRef { .. } => Some("notes"),
        InlineObject::CommentStart { .. } | InlineObject::CommentEnd { .. } => Some("comments"),
        InlineObject::Equation { .. } => Some("equations"),
        InlineObject::Ruby { .. } | InlineObject::Opaque { .. } => Some("objects"),
        _ => None,
    }
}

/// Why this table must stay as it is: content the rewrite can't carry.
fn blocker(t: &Table) -> Option<String> {
    let mut kinds: Vec<&str> = Vec::new();
    let mut tracked = false;
    let mut text = false;
    each_para(t, 1, &mut |p| {
        for k in p.objects.iter().filter_map(object_kind) {
            if !kinds.contains(&k) {
                kinds.push(k);
            }
        }
        tracked |= p.runs.iter().any(|r| r.props.ins.is_some() || r.props.del.is_some());
        text |= words(&p.plain_text()).next().is_some();
    });
    if !kinds.is_empty() {
        return Some(format!("{} in its cells", kinds.join(", ")));
    }

    if tracked {
        return Some("tracked changes in its cells".into());
    }
    if depth(t, 1) > MAX_DEPTH {
        return Some(format!("tables nested more than {MAX_DEPTH} deep"));
    }
    if !text {
        return Some("no text".into());
    }
    if describe(t).to_string().chars().count() > MAX_CHARS {
        return Some("too large to send in one request".into());
    }
    None
}

/// The words of `text`, lower case: runs of letters and digits.
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase)
}

fn word_count(t: &Table) -> BTreeMap<String, usize> {
    let mut n = BTreeMap::new();
    each_para(t, 1, &mut |p| {
        for w in words(&p.plain_text()) {
            *n.entry(w).or_insert(0) += 1;
        }
    });
    n
}

/// The paragraph's visible text with its bold, italic and links as Markdown (`**…**`, `*…*`,
/// `[…](…)`), so the rewrite keeps them.
fn inline_md(p: &Paragraph) -> String {
    // Runs with the same emphasis and link are one span (Word splits runs for many reasons).
    let mut spans: Vec<((bool, bool, Option<&str>), String)> = Vec::new();
    let (mut off, mut k) = (0usize, 0usize);
    for r in &p.runs {
        let piece = p.text.get(off..off.saturating_add(r.len)).unwrap_or("");
        off = off.saturating_add(r.len);
        let mut t = String::new();
        for c in piece.chars() {
            if c == wordcraft_doc::para::OBJ {
                if let Some(o) = p.objects.get(k) {
                    t.push_str(o.plain_text());
                }
                k += 1;
            } else {
                t.push(c);
            }
        }
        let fmt = (r.props.bold == Some(true), r.props.italic == Some(true), r.props.link.as_deref().filter(|l| !l.is_empty()));
        match spans.last_mut() {
            Some((f, text)) if *f == fmt => text.push_str(&t),
            _ => spans.push((fmt, t)),
        }
    }
    let mut out = String::new();
    for ((bold, italic, link), t) in spans {
        let core = t.trim();
        if core.is_empty() || (!bold && !italic && link.is_none()) {
            out.push_str(&t);
            continue;
        }
        let lead = t.get(..t.len() - t.trim_start().len()).unwrap_or("");
        let trail = t.get(t.trim_end().len()..).unwrap_or("");
        let mark = match (bold, italic) {
            (true, true) => "***",
            (true, false) => "**",
            (false, true) => "*",
            (false, false) => "",
        };
        let mut m = format!("{mark}{core}{mark}");
        if let Some(url) = link {
            m = format!("[{m}]({})", url.replace([' ', ')'], ""));
        }
        out.push_str(lead);
        out.push_str(&m);
        out.push_str(trail);
    }
    out
}

/// The body's tables that Markdown can't hold as they are: tables in tables, merged cells.
pub fn candidates(doc: &Document) -> Vec<Candidate> {
    doc.body
        .iter()
        .enumerate()
        .filter_map(|(index, b)| {
            let t = b.as_table()?;
            let depth = depth(t, 1);
            let merged = merged(t);
            if depth < 2 && !merged {
                return None;
            }
            Some(Candidate {
                index,
                rows: t.rows.len(),
                cols: t.grid.len().max(t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0)),
                depth,
                merged,
                words: word_count(t).values().sum(),
                blocked: blocker(t),
            })
        })
        .collect()
}

/// The table as the model sees it: rows of cells; a cell holds paragraphs (strings) and inner
/// tables; `span` = columns covered, `merged` = continues the cell above.
pub fn describe(t: &Table) -> Value {
    fn table(t: &Table, d: usize) -> Value {
        let rows: Vec<Value> = t
            .rows
            .iter()
            .map(|r| {
                Value::Array(
                    r.cells
                        .iter()
                        .map(|c| {
                            let mut content = Vec::new();
                            for b in &c.blocks {
                                match &**b {
                                    Block::Para(p) => {
                                        let text = inline_md(p);
                                        if !text.trim().is_empty() {
                                            content.push(json!(text.trim()));
                                        }
                                    }
                                    Block::Table(i) if d < MAX_DEPTH => content.push(json!({"table": table(i, d + 1)})),
                                    _ => {}
                                }
                            }
                            let mut cell = json!({"content": content});
                            if c.span() > 1 {
                                cell["span"] = json!(c.span());
                            }
                            if c.props.vmerge == VMerge::Continue {
                                cell["merged"] = json!(true);
                            }
                            cell
                        })
                        .collect(),
                )
            })
            .collect();
        json!({"rows": rows})
    }
    table(t, 1)
}

pub const SYSTEM: &str = "You restructure one table from an old word-processor document into clean document structure for Markdown.
The table is JSON: \"rows\" of cells; a cell's \"content\" lists its paragraphs (strings) and inner tables ({\"table\": …}). \"span\" is how many columns a cell covers; \"merged\": true means the cell continues the one above.
First decide what the table is:
- \"layout\": the table only places text on the page (a form, a letterhead, a cover, boxes side by side). Rewrite it as headings, paragraphs and lists in reading order: left to right, top to bottom, inner tables where they sit.
- \"data\": rows of comparable records under column headers. Keep it as a table: one header row, the same number of cells in every row, no merged cells (repeat a merged value in each row it covers). Inner tables become their own tables or lists right after.
Rules:
- Copy every word exactly as written, in its language and order. Never translate, summarise, correct, reorder within a cell or add words: no new headings, labels or explanations.
- Every word of the table must appear in your answer. Leave out only empty cells.
- Use a heading only for text that already reads as a title.
- Text may carry Markdown emphasis (**bold**, *italic*) and links [text](url): keep them where they are.
Answer with JSON only.";

/// The answer's shape (JSON Schema).
pub fn schema() -> Value {
    let strings = json!({"type": "array", "items": {"type": "string"}});
    json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string", "enum": ["layout", "data"]},
            "blocks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "type": {"type": "string", "enum": ["heading", "paragraph", "list", "table"]},
                        "level": {"type": "integer", "minimum": 1, "maximum": 6},
                        "text": {"type": "string"},
                        "ordered": {"type": "boolean"},
                        "items": strings,
                        "header": strings,
                        "rows": {"type": "array", "items": strings}
                    },
                    "required": ["type"]
                }
            }
        },
        "required": ["kind", "blocks"]
    })
}

/// The request for the table at body `index`: (system prompt, user message, answer schema).
pub fn request(doc: &Document, index: usize) -> Result<(String, String, Value), String> {
    let t = doc.body.get(index).and_then(|b| b.as_table()).ok_or_else(|| format!("no table at block {index}"))?;
    if let Some(why) = blocker(t) {
        return Err(format!("this table stays as it is: {why}"));
    }
    Ok((SYSTEM.to_string(), format!("Table:\n{}", describe(t)), schema()))
}

/// A block of the model's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    Heading(u8, String),
    Paragraph(String),
    List { ordered: bool, items: Vec<String> },
    Table { header: Vec<String>, rows: Vec<Vec<String>> },
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
}

/// The answer's blocks; unknown or empty ones are dropped.
pub fn parse(answer: &Value) -> Result<Vec<Out>, String> {
    let blocks = answer["blocks"].as_array().ok_or("the answer has no blocks")?;
    let mut out = Vec::new();
    for b in blocks {
        let text = b["text"].as_str().unwrap_or("").trim().to_string();
        match b["type"].as_str() {
            Some("heading") if !text.is_empty() => out.push(Out::Heading(b["level"].as_u64().unwrap_or(2).clamp(1, 6) as u8, text)),
            Some("paragraph") if !text.is_empty() => out.push(Out::Paragraph(text)),
            Some("list") => {
                let items: Vec<String> = strings(&b["items"]).into_iter().filter(|s| !s.trim().is_empty()).collect();
                if !items.is_empty() {
                    out.push(Out::List { ordered: b["ordered"].as_bool().unwrap_or(false), items });
                }
            }
            Some("table") => {
                let rows: Vec<Vec<String>> = b["rows"].as_array().into_iter().flatten().map(strings).filter(|r| !r.is_empty()).collect();
                let header = strings(&b["header"]);
                if !header.is_empty() || !rows.is_empty() {
                    out.push(Out::Table { header, rows });
                }
            }
            _ => {}
        }
    }
    if out.is_empty() {
        return Err("the answer is empty".into());
    }
    Ok(out)
}

fn out_text(blocks: &[Out]) -> String {
    let mut s = String::new();
    for b in blocks {
        match b {
            Out::Heading(_, t) | Out::Paragraph(t) => s.push_str(t),
            Out::List { items, .. } => s.push_str(&items.join(" ")),
            Out::Table { header, rows } => {
                s.push_str(&header.join(" "));
                for r in rows {
                    s.push(' ');
                    s.push_str(&r.join(" "));
                }
            }
        }
        s.push('\n');
    }
    s
}

/// What the check found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Check {
    /// Words of the table.
    pub words: usize,
    /// Words of the table the answer lacks (each as often as it's missing).
    pub missing: Vec<String>,
    /// Words of the answer the table doesn't have.
    pub invented: Vec<String>,
}

impl Check {
    pub fn passed(&self) -> bool {
        self.missing.is_empty() && self.invented.is_empty()
    }
    /// One line for the user.
    pub fn summary(&self) -> String {
        let few = |w: &[String]| w.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
        match (self.missing.is_empty(), self.invented.is_empty()) {
            (true, true) => format!("all {} words kept", self.words),
            (false, true) => format!("{} words missing: {}", self.missing.len(), few(&self.missing)),
            (true, false) => format!("{} words added: {}", self.invented.len(), few(&self.invented)),
            (false, false) => {
                format!("{} words missing ({}), {} added ({})", self.missing.len(), few(&self.missing), self.invented.len(), few(&self.invented))
            }
        }
    }
}

/// `text` without link targets: `[a](http://x)` → `[a]`.
fn strip_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("](") {
        out.push_str(rest.get(..=i).unwrap_or(""));
        let after = rest.get(i + 2..).unwrap_or("");
        match after.find(')') {
            Some(j) if !after.get(..j).unwrap_or(" ").contains(char::is_whitespace) => rest = after.get(j + 1..).unwrap_or(""),
            _ => rest = rest.get(i + 1..).unwrap_or(""),
        }
    }
    out.push_str(rest);
    out
}

/// Compare the table's words with the answer's: none lost, none made up. A word may appear more
/// often than in the table (a merged value repeated in each row it covers).
pub fn check(t: &Table, blocks: &[Out]) -> Check {
    let source = word_count(t);
    let mut answer: BTreeMap<String, usize> = BTreeMap::new();
    for w in words(&strip_links(&out_text(blocks))) {
        *answer.entry(w).or_insert(0) += 1;
    }
    let mut missing = Vec::new();
    for (w, n) in &source {
        let got = answer.get(w).copied().unwrap_or(0);
        for _ in got..*n {
            missing.push(w.clone());
        }
    }
    let invented = answer.keys().filter(|w| !source.contains_key(*w)).cloned().collect();
    Check { words: source.values().sum(), missing, invented }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("<br>"),
            c => o.push(c),
        }
    }
    o
}

/// Markdown emphasis and links (as [`inline_md`] writes them) as HTML; everything else escaped.
fn inline_html(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    'scan: while let Some(c) = rest.chars().next() {
        if c == '['
            && let Some(close) = rest.find("](")
            && let Some(end) = rest.get(close + 2..).and_then(|a| a.find(')'))
        {
            let text = rest.get(1..close).unwrap_or("");
            let url = rest.get(close + 2..close + 2 + end).unwrap_or("");
            if !text.is_empty() && !text.contains('[') && !url.is_empty() && !url.contains(char::is_whitespace) {
                out.push_str(&format!("<a href=\"{}\">{}</a>", esc(url), inline_html(text)));
                rest = rest.get(close + 3 + end..).unwrap_or("");
                continue;
            }
        }
        for (mark, open, shut) in [("***", "<b><i>", "</i></b>"), ("**", "<b>", "</b>"), ("*", "<i>", "</i>")] {
            if let Some(after) = rest.strip_prefix(mark)
                && let Some(e) = after.find(mark).filter(|e| *e > 0)
            {
                out.push_str(open);
                out.push_str(&inline_html(after.get(..e).unwrap_or("")));
                out.push_str(shut);
                rest = after.get(e + mark.len()..).unwrap_or("");
                continue 'scan;
            }
        }
        out.push_str(&esc(&c.to_string()));
        rest = rest.get(c.len_utf8()..).unwrap_or("");
    }
    out
}

/// The blocks as HTML, for the HTML importer (styles, list numbering).
pub fn to_html(blocks: &[Out]) -> String {
    let mut h = String::from("<html><body>");
    for b in blocks {
        match b {
            Out::Heading(l, t) => h.push_str(&format!("<h{l}>{}</h{l}>", inline_html(t))),
            Out::Paragraph(t) => h.push_str(&format!("<p>{}</p>", inline_html(t))),
            Out::List { ordered, items } => {
                let tag = if *ordered { "ol" } else { "ul" };
                h.push_str(&format!("<{tag}>"));
                for i in items {
                    h.push_str(&format!("<li>{}</li>", inline_html(i)));
                }
                h.push_str(&format!("</{tag}>"));
            }
            Out::Table { header, rows } => {
                let cols = rows.iter().map(Vec::len).chain([header.len()]).max().unwrap_or(1).clamp(1, wordcraft_doc::table::MAX_COLS);
                let row = |cells: &[String], tag: &str| {
                    let mut r = String::from("<tr>");
                    for i in 0..cols {
                        r.push_str(&format!("<{tag}>{}</{tag}>", inline_html(cells.get(i).map(String::as_str).unwrap_or(""))));
                    }
                    r + "</tr>"
                };
                h.push_str("<table>");
                if !header.is_empty() {
                    h.push_str(&format!("<thead>{}</thead>", row(header, "th")));
                }
                h.push_str("<tbody>");
                for r in rows.iter().take(wordcraft_doc::table::MAX_ROWS) {
                    h.push_str(&row(r, "td"));
                }
                h.push_str("</tbody></table>");
            }
        }
    }
    h + "</body></html>"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use wordcraft_doc::table::Cell;

    /// A form: a title row spanning two columns, then label/value pairs, with an address
    /// table inside a cell.
    fn form() -> Document {
        let mut outer = Table::new(3, 2, 400.0);
        outer.rows[0].cells[0] = Cell::with_text("Application for leave");
        outer.rows[0].cells[0].props.span = 2;
        outer.rows[0].cells.truncate(1);
        outer.rows[1].cells[0] = Cell::with_text("Name:");
        outer.rows[1].cells[1] = Cell::with_text("Jane Doe");
        let mut address = Table::new(1, 2, 200.0);
        address.rows[0].cells[0] = Cell::with_text("12 rue des Lilas");
        address.rows[0].cells[1] = Cell::with_text("Lyon");
        outer.rows[2].cells[0] = Cell::with_text("Address:");
        outer.rows[2].cells[1].blocks.push(Arc::new(Block::Table(address)));
        let mut d = Document::from_text("Before");
        d.body.push(Arc::new(Block::Table(outer)));
        d
    }

    fn good_answer() -> Value {
        json!({"kind": "layout", "blocks": [
            {"type": "heading", "level": 2, "text": "Application for leave"},
            {"type": "list", "items": ["Name: Jane Doe", "Address: 12 rue des Lilas, Lyon"]}
        ]})
    }

    #[test]
    fn finds_nested_and_merged_tables_only() {
        let mut d = form();
        d.body.push(Arc::new(Block::Table(Table::new(2, 2, 300.0))));
        let c = candidates(&d);
        assert_eq!(c.len(), 1, "a plain table is left alone: {c:?}");
        assert_eq!((c[0].index, c[0].depth, c[0].merged, c[0].blocked.as_deref()), (1, 2, true, None));
        let (_, user, schema) = request(&d, 1).unwrap();
        assert!(user.contains("\"span\":2") && user.contains("\"table\"") && user.contains("Lyon"), "{user}");
        assert_eq!(schema["required"], json!(["kind", "blocks"]));
        assert!(request(&d, 0).is_err(), "block 0 is a paragraph");
    }

    #[test]
    fn checks_every_word() {
        let d = form();
        let t = d.body[1].as_table().unwrap();
        let ok = check(t, &parse(&good_answer()).unwrap());
        assert!(ok.passed(), "{}", ok.summary());
        assert_eq!(ok.words, 12);
        let lossy = json!({"kind": "layout", "blocks": [{"type": "paragraph", "text": "Application for leave: Jane Doe, Lyon"}]});
        let c = check(t, &parse(&lossy).unwrap());
        assert_eq!(c.missing, ["12", "address", "des", "lilas", "name", "rue"]);
        let made_up = json!({"kind": "layout", "blocks": [{"type": "heading", "text": "Leave request form"}, {"type": "paragraph", "text": "Application for leave Name: Jane Doe Address: 12 rue des Lilas Lyon"}]});
        let c = check(t, &parse(&made_up).unwrap());
        assert_eq!(c.invented, ["form", "request"]);
        assert!(c.summary().contains("added"));
    }

    #[test]
    fn answers_become_html_the_importer_reads() {
        let blocks = parse(&json!({"kind": "data", "blocks": [
            {"type": "heading", "level": 9, "text": "Prices <2026> & taxes"},
            {"type": "table", "header": ["Item", "Price"], "rows": [["Tea"], ["Coffee", "2", "extra"]]},
            {"type": "list", "ordered": true, "items": ["one", " "]},
            {"type": "mystery", "text": "dropped"}
        ]}))
        .unwrap();
        assert_eq!(blocks.len(), 3);
        let html = to_html(&blocks);
        assert!(html.contains("<h6>Prices &lt;2026&gt; &amp; taxes</h6>"), "{html}");
        let doc = wordcraft_formats::html::import(&html);
        let tables: Vec<&Table> = doc.body.iter().filter_map(|b| b.as_table()).collect();
        assert_eq!(tables.len(), 1);
        assert!(tables[0].rows.iter().all(|r| r.cells.len() == 3), "rows padded to the widest");
        assert!(doc.plain_text(wordcraft_doc::StoryRef::Body).contains("Coffee"));
        assert!(parse(&json!({"blocks": []})).is_err());
    }

    #[test]
    fn bookmarks_and_fields_pass_pictures_dont() {
        let mut d = form();
        let Block::Table(t) = Arc::make_mut(&mut d.body[1]) else { panic!() };
        // A form field whose result is "Lyon" in place of the plain text, and a bookmark.
        let Block::Para(p) = Arc::make_mut(&mut t.rows[1].cells[1].blocks[0]) else { panic!() };
        p.insert_object(0, InlineObject::BookmarkStart { name: "name".into() }, &Default::default()).unwrap();
        let c = candidates(&d);
        assert_eq!(c[0].blocked, None, "a bookmark carries no words");
        let Block::Table(t) = Arc::make_mut(&mut d.body[1]) else { panic!() };
        let Block::Para(p) = Arc::make_mut(&mut t.rows[1].cells[1].blocks[0]) else { panic!() };
        p.insert_object(
            0,
            InlineObject::Field { instr: "FORMTEXT".into(), result: "Smith ".into(), locked: false, dirty: false, code: false, form: String::new() },
            &Default::default(),
        )
        .unwrap();
        let c = candidates(&d);
        assert_eq!(c[0].words, 13, "the field's result counts");
        assert!(request(&d, 1).unwrap().1.contains("Smith"));
        let Block::Table(t) = Arc::make_mut(&mut d.body[1]) else { panic!() };
        let Block::Para(p) = Arc::make_mut(&mut t.rows[1].cells[1].blocks[0]) else { panic!() };
        p.insert_object(
            0,
            InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Footnote, id: 1, custom: String::new() },
            &Default::default(),
        )
        .unwrap();
        assert_eq!(candidates(&d)[0].blocked.as_deref(), Some("notes in its cells"));
        d.body.push(Arc::new(Block::Table(Table::new(2, 2, 100.0))));
        let mut empty = Table::new(2, 2, 100.0);
        empty.rows[0].cells[0].props.span = 2;
        d.body.push(Arc::new(Block::Table(empty)));
        assert_eq!(candidates(&d).last().unwrap().blocked.as_deref(), Some("no text"));
    }

    #[test]
    fn emphasis_and_links_travel_as_markdown() {
        let mut p = Paragraph::with_text("Protocol No.: A5346 see site", Default::default());
        let bold = wordcraft_doc::props::CharProps { bold: Some(true), ..Default::default() };
        let link = wordcraft_doc::props::CharProps { link: Some("https://example.org/a".into()), ..Default::default() };
        p.runs = vec![
            wordcraft_doc::Run { len: 9, props: bold.clone() },
            wordcraft_doc::Run { len: 4, props: bold },
            wordcraft_doc::Run { len: 11, props: Default::default() },
            wordcraft_doc::Run { len: 4, props: link },
        ];
        assert_eq!(inline_md(&p), "**Protocol No.:** A5346 see [site](https://example.org/a)");
        let html = inline_html("**Protocol No.:** A5346 *x* 2*3 [site](https://example.org/a) <b>");
        assert_eq!(html, "<b>Protocol No.:</b> A5346 <i>x</i> 2*3 <a href=\"https://example.org/a\">site</a> &lt;b&gt;");
        let mut t = Table::new(1, 1, 100.0);
        t.rows[0].cells[0].blocks = vec![Arc::new(Block::Para(p))];
        let ok = check(&t, &[Out::Paragraph("**Protocol No.:** A5346 see [site](https://example.org/a)".into())]);
        assert!(ok.passed(), "the link's address isn't a made-up word: {}", ok.summary());
    }

    #[test]
    fn pictures_and_deep_nesting_block_a_table() {
        let mut t = Table::new(1, 1, 100.0);
        for _ in 0..MAX_DEPTH + 2 {
            let mut outer = Table::new(1, 1, 100.0);
            outer.rows[0].cells[0].blocks.push(Arc::new(Block::Table(t)));
            t = outer;
        }
        let mut d = Document::new();
        d.body.push(Arc::new(Block::Table(t)));
        let c = candidates(&d);
        assert!(c[0].blocked.as_deref().is_some_and(|b| b.contains("nested")), "{c:?}");
    }
}
