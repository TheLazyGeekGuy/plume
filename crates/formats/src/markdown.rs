//! Markdown: a CommonMark subset plus GitHub tables and strikethrough.
//!
//! Import: ATX and setext headings, paragraphs with soft/hard breaks, block quotes (→ Quote
//! style), fenced and indented code (→ one Code paragraph per line), thematic breaks (→ a
//! bottom-border paragraph), bullet and ordered lists (nested by indentation), GFM pipe tables,
//! and inlines: emphasis/strong (the CommonMark delimiter algorithm), `~~strike~~`, code spans,
//! inline links, autolinks, images (`data:` URIs become pictures, others their alt text),
//! backslash escapes, entities, `<br>` and `<a id>` anchors.
//!
//! Export writes the same constructs; pictures become `![alt](media-name)` placeholders.
//!
//! [`export_native`] is the Markdown-native save: pure CommonMark + GFM with no HTML at all
//! (only HTML *comments*, which every renderer hides), pictures written as files beside the
//! document with stable content-hash names, and a [`Report`] of what Markdown can't hold. Its
//! counterpart [`import_with`] loads pictures named by a relative path through a resolver.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use wordcraft_doc::{Align, Document};

use crate::html::decode_entity;
use crate::model::{self, Cell, FBlock, FTable, Fmt, Img, Inline, Kind, ListInfo, Para, make_img};

/// Nesting limit for block containers (quotes, list items) and inline links.
const MAX_NEST: usize = 32;

// ---------------------------------------------------------------------------------------------
// Import

/// Parse Markdown text into a document.
pub fn import(text: &str) -> Document {
    model::to_doc(&parse(text))
}

/// [`import`] onto `page`: Markdown gives no page size (#1381).
pub fn import_on(text: &str, page: &wordcraft_doc::SectionProps) -> Document {
    model::to_doc_in(&parse(text), page.clone())
}

/// Most distinct picture destinations one document resolves.
const MAX_IMAGE_REFS: usize = 4096;

/// What the image branch of the inline parser does with a destination that isn't a `data:` URI.
enum ImageRefs {
    /// Alt text only (plain [`parse`]).
    Off,
    /// Note the destination (first pass of [`import_with`]).
    Collect(Vec<String>),
    /// Use the bytes resolved for it (second pass).
    Use(BTreeMap<String, Arc<Vec<u8>>>),
}

thread_local! {
    static IMAGE_REFS: RefCell<ImageRefs> = const { RefCell::new(ImageRefs::Off) };
}

/// Restores [`ImageRefs::Off`] when an import ends, however it ends.
struct RefsGuard;

impl Drop for RefsGuard {
    fn drop(&mut self) {
        IMAGE_REFS.with(|r| {
            if let Ok(mut r) = r.try_borrow_mut() {
                *r = ImageRefs::Off;
            }
        });
    }
}

fn set_refs(v: ImageRefs) -> RefsGuard {
    IMAGE_REFS.with(|r| {
        if let Ok(mut r) = r.try_borrow_mut() {
            *r = v;
        }
    });
    RefsGuard
}

/// The bytes resolved for `dest`, noting it on the collecting pass.
fn image_ref(dest: &str) -> Option<Arc<Vec<u8>>> {
    IMAGE_REFS.with(|r| {
        let mut r = r.try_borrow_mut().ok()?;
        match &mut *r {
            ImageRefs::Off => None,
            ImageRefs::Collect(v) => {
                if v.len() < MAX_IMAGE_REFS && !v.iter().any(|d| d == dest) {
                    v.push(dest.to_string());
                }
                None
            }
            ImageRefs::Use(m) => m.get(dest).cloned(),
        }
    })
}

/// [`import_on`] with pictures named by a path (`![alt](assets/img.png)`): `resolve` gives the
/// bytes of each destination (the caller decides what may be read, e.g. only files inside the
/// document's folder); a destination it can't resolve keeps its alt text, as in [`import`].
pub fn import_with(text: &str, page: &wordcraft_doc::SectionProps, resolve: &dyn Fn(&str) -> Option<Arc<Vec<u8>>>) -> Document {
    let dests = {
        let _guard = set_refs(ImageRefs::Collect(Vec::new()));
        let _ = parse(text);
        IMAGE_REFS.with(|r| match r.try_borrow_mut().map(|mut r| std::mem::replace(&mut *r, ImageRefs::Off)) {
            Ok(ImageRefs::Collect(v)) => v,
            _ => Vec::new(),
        })
    };
    if dests.is_empty() {
        return import_on(text, page);
    }
    let resolved: BTreeMap<String, Arc<Vec<u8>>> = dests.into_iter().filter_map(|d| resolve(&d).map(|b| (d, b))).collect();
    let _guard = set_refs(ImageRefs::Use(resolved));
    model::to_doc_in(&parse(text), page.clone())
}

/// Parse Markdown text into the flow model.
pub fn parse(text: &str) -> model::Flow {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut lines: Vec<String> = text.split('\n').map(|l| expand_tabs(l.trim_end_matches('\r'))).collect();
    let meta = take_front_matter(&mut lines).unwrap_or_default();
    let comments = take_comment_block(&mut lines);
    // Footnote definitions are parsed first (a reference inside one stays text), then the body
    // resolves `[^label]` against them.
    let notes: BTreeMap<String, Vec<FBlock>> = take_footnote_defs(&mut lines).into_iter().map(|(l, body)| (l, parse_blocks(&body, 1, 0))).collect();
    let _guard = NotesGuard::set(notes);
    let mut blocks = parse_blocks(&lines, 0, 0);
    if blocks.is_empty() {
        blocks.push(FBlock::Para(Para::default()));
    }
    model::Flow { blocks, meta, comments }
}

/// First line of the comment block [`export_native`] writes.
pub const COMMENTS_OPEN: &str = "<!-- plume:comments";
/// The block's first line in the first prototype (read and dropped: it held no anchors).
const OLD_COMMENTS_OPEN: &str = "<!-- wordcraft:comments";

/// `c12` → (12, false), `/c12` → (12, true): a comment range marker's text.
fn comment_marker(inner: &str) -> Option<(u32, bool)> {
    let (end, num) = match inner.strip_prefix('/') {
        Some(r) => (true, r),
        None => (false, inner),
    };
    let digits = num.strip_prefix('c')?;
    if digits.is_empty() || digits.len() > 9 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((digits.parse().ok()?, end))
}

/// Remove the comment block (outside code blocks) and read its threads:
///
/// ```text
/// <!-- plume:comments
/// c1 | Alice | 2026-10-14T09:12:00Z | resolved
///     The comment's text,
///     one paragraph per line.
/// c1.1 | Bob | 2026-10-14T10:03:00Z
///     A reply.
/// -->
/// ```
fn take_comment_block(lines: &mut Vec<String>) -> Vec<model::FComment> {
    let mut out: Vec<model::FComment> = Vec::new();
    let mut fenced: Option<(char, usize)> = None;
    let mut start = None;
    for (k, l) in lines.iter().enumerate() {
        if indent(l) >= 4 {
            continue;
        }
        let t = l.trim();
        if let Some((c, n)) = fenced {
            if fence(t).is_some_and(|(c2, n2)| c2 == c && n2 >= n) {
                fenced = None;
            }
            continue;
        }
        if let Some(f) = fence(t) {
            fenced = Some(f);
            continue;
        }
        if t.starts_with(COMMENTS_OPEN) || t.starts_with(OLD_COMMENTS_OPEN) {
            start = Some(k);
            break;
        }
    }
    let Some(start) = start else { return out };
    let legacy = lines.get(start).is_some_and(|l| l.trim().starts_with(OLD_COMMENTS_OPEN));
    let end = lines.iter().enumerate().skip(start).find(|(_, l)| l.contains("-->")).map(|(k, _)| k).unwrap_or(lines.len().saturating_sub(1));
    if !legacy {
        for l in lines.get(start + 1..end).unwrap_or(&[]) {
            if indent(l) >= 4 || l.trim().is_empty() {
                if let Some(c) = out.last_mut()
                    && (!c.text.is_empty() || !l.trim().is_empty())
                {
                    if !c.text.is_empty() {
                        c.text.push('\n');
                    }
                    c.text.push_str(strip_indent(l, 4).trim_end());
                }
                continue;
            }
            let mut parts = l.split(" | ");
            let key = parts.next().unwrap_or("").trim();
            let (n, reply) = match key.split_once('.') {
                Some((a, b)) => (comment_marker(a).map(|m| m.0), b.parse::<u32>().ok()),
                None => (comment_marker(key).map(|m| m.0), Some(0)),
            };
            let (Some(n), Some(reply)) = (n, reply) else { continue };
            if out.len() >= 5_000 {
                break;
            }
            let author = parts.next().unwrap_or("").trim().to_string();
            let date = parts.next().unwrap_or("").trim().to_string();
            let resolved = parts.next().is_some_and(|x| x.trim() == "resolved");
            out.push(model::FComment { n, reply, author, date, resolved, text: String::new() });
        }
        for c in &mut out {
            while c.text.ends_with('\n') {
                c.text.pop();
            }
        }
    }
    lines.drain(start..=end.min(lines.len().saturating_sub(1)));
    out
}

/// Longest front matter block read (lines).
const MAX_FRONT_MATTER: usize = 200;
/// Most footnote definitions one document holds.
const MAX_NOTES: usize = 10_000;

/// YAML front matter (`---` … `---`) at the very start: the document properties it gives
/// (`title`, `author`, `subject`, `keywords`, `description`, `date`), its lines removed. Only a
/// block of `key: value` lines counts, so a document opening with a thematic break isn't eaten.
fn take_front_matter(lines: &mut Vec<String>) -> Option<model::Meta> {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return None;
    }
    let close = lines.iter().enumerate().skip(1).take(MAX_FRONT_MATTER).find(|(_, l)| matches!(l.trim_end(), "---" | "...")).map(|(k, _)| k)?;
    let body = lines.get(1..close)?;
    let mut meta = model::Meta::default();
    let mut any = false;
    for l in body {
        if l.trim().is_empty() || l.trim_start().starts_with('#') {
            continue;
        }
        let (key, value) = l.split_once(':')?;
        if key.starts_with([' ', '-']) || key.trim().is_empty() || key.contains(' ') {
            return None;
        }
        let value = yaml_scalar(value.trim());
        any = true;
        match key.trim().to_ascii_lowercase().as_str() {
            "title" => meta.title = value,
            "author" => meta.author = value,
            "subject" => meta.subject = value,
            "keywords" | "tags" => meta.keywords = value,
            "description" => meta.description = value,
            "date" => meta.created = value,
            _ => {}
        }
    }
    if !any {
        return None;
    }
    lines.drain(..=close);
    Some(meta)
}

/// A YAML scalar as written by [`front_matter_md`] (double-quoted with `\"` and `\\`), single-quoted
/// (`''`), plain, or a flow list `[a, b]` (joined with `, `).
fn yaml_scalar(v: &str) -> String {
    if let Some(inner) = v.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push(' '),
                    Some(n) => out.push(n),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    if let Some(inner) = v.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    if let Some(inner) = v.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return inner.split(',').map(|x| yaml_scalar(x.trim())).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
    }
    v.to_string()
}

/// A footnote definition's label at the start of `l` (`[^label]: rest`) and the rest.
fn footnote_def(l: &str) -> Option<(String, String)> {
    if indent(l) > 3 {
        return None;
    }
    let t = l.trim_start().strip_prefix("[^")?;
    let close = t.find("]:")?;
    let label = t.get(..close)?;
    if label.is_empty() || label.len() > 64 || label.contains(char::is_whitespace) || label.contains(['[', ']']) {
        return None;
    }
    let rest = t.get(close + 2..)?;
    Some((label.to_string(), rest.strip_prefix(' ').unwrap_or(rest).to_string()))
}

/// Remove footnote definitions (outside code blocks) from `lines`: label → the definition's
/// lines, continuation lines (indented by four) dedented.
fn take_footnote_defs(lines: &mut Vec<String>) -> Vec<(String, Vec<String>)> {
    let mut defs: Vec<(String, Vec<String>)> = Vec::new();
    if !lines.iter().any(|l| l.contains("[^")) {
        return defs;
    }
    let mut kept: Vec<String> = Vec::with_capacity(lines.len());
    let mut fenced: Option<(char, usize)> = None;
    let mut i = 0;
    while let Some(l) = lines.get(i) {
        if let Some((c, n)) = fenced {
            if indent(l) < 4 && fence(l.trim_start()).is_some_and(|(c2, n2)| c2 == c && n2 >= n) {
                fenced = None;
            }
            kept.push(l.clone());
            i += 1;
            continue;
        }
        if indent(l) < 4
            && let Some(f) = fence(l.trim_start())
        {
            fenced = Some(f);
            kept.push(l.clone());
            i += 1;
            continue;
        }
        match footnote_def(l).filter(|_| defs.len() < MAX_NOTES) {
            Some((label, first)) => {
                let mut body = vec![first];
                i += 1;
                while let Some(next) = lines.get(i) {
                    if is_blank(next) {
                        // A blank line continues the note only if an indented line follows.
                        let more = lines.get(i + 1..).and_then(|r| r.iter().find(|x| !is_blank(x))).is_some_and(|x| indent(x) >= 4);
                        if !more {
                            break;
                        }
                        body.push(String::new());
                    } else if indent(next) >= 4 {
                        body.push(strip_indent(next, 4).to_string());
                    } else {
                        break;
                    }
                    i += 1;
                }
                if !defs.iter().any(|(l, _)| *l == label) {
                    defs.push((label, body));
                }
            }
            None => {
                kept.push(l.clone());
                i += 1;
            }
        }
    }
    *lines = kept;
    defs
}

thread_local! {
    static NOTES: RefCell<BTreeMap<String, Vec<FBlock>>> = const { RefCell::new(BTreeMap::new()) };
}

/// The footnote definitions of the document being parsed; the previous ones come back when it
/// ends, however it ends.
struct NotesGuard(BTreeMap<String, Vec<FBlock>>);

impl NotesGuard {
    fn set(notes: BTreeMap<String, Vec<FBlock>>) -> NotesGuard {
        let prev = NOTES.with(|n| n.try_borrow_mut().map(|mut n| std::mem::replace(&mut *n, notes)).unwrap_or_default());
        NotesGuard(prev)
    }
}

impl Drop for NotesGuard {
    fn drop(&mut self) {
        let prev = std::mem::take(&mut self.0);
        NOTES.with(|n| {
            if let Ok(mut n) = n.try_borrow_mut() {
                *n = prev;
            }
        });
    }
}

/// A footnote reference `[^label]` at byte `i` of `s` whose definition exists: the note's blocks
/// and the byte after the reference.
fn footnote_ref(s: &str, i: usize) -> Option<(Vec<FBlock>, usize)> {
    let rest = s.get(i..)?.strip_prefix("[^")?;
    let close = within(rest, 66).find(']')?;
    let label = rest.get(..close)?;
    if label.is_empty() || label.contains(char::is_whitespace) || rest.get(close + 1..).is_some_and(|r| r.starts_with(':')) {
        return None;
    }
    let blocks = NOTES.with(|n| n.try_borrow().ok().and_then(|n| n.get(label).cloned()))?;
    Some((blocks, i + 2 + close + 1))
}

/// Leading tabs → spaces (tab stops of 4); the rest of the line is kept.
fn expand_tabs(l: &str) -> String {
    if !l.starts_with([' ', '\t']) || !l.contains('\t') {
        return l.to_string();
    }
    let mut out = String::with_capacity(l.len() + 8);
    let mut col = 0usize;
    let mut rest = l;
    while let Some(c) = rest.chars().next() {
        match c {
            ' ' => {
                out.push(' ');
                col += 1;
            }
            '\t' => {
                let n = 4 - col % 4;
                out.push_str(&" ".repeat(n));
                col += n;
            }
            _ => break,
        }
        rest = &rest[c.len_utf8()..];
    }
    out.push_str(rest);
    out
}

fn indent(l: &str) -> usize {
    l.len() - l.trim_start_matches(' ').len()
}

fn is_blank(l: &str) -> bool {
    l.trim().is_empty()
}

/// Strip up to `n` leading spaces.
fn strip_indent(l: &str, n: usize) -> &str {
    let k = indent(l).min(n);
    l.get(k..).unwrap_or("")
}

fn atx_heading(t: &str) -> Option<(u8, &str)> {
    let n = t.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&n) {
        return None;
    }
    let rest = t.get(n..)?;
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let mut body = rest.trim();
    // Optional closing sequence.
    let trimmed = body.trim_end_matches('#');
    if trimmed.len() != body.len() && (trimmed.is_empty() || trimmed.ends_with(' ')) {
        body = trimmed.trim_end();
    }
    Some((n as u8, body))
}

fn thematic_break(t: &str) -> bool {
    let mut ch = None;
    let mut n = 0;
    for c in t.chars() {
        match c {
            ' ' | '\t' => {}
            '*' | '-' | '_' => {
                if ch.is_some_and(|x| x != c) {
                    return false;
                }
                ch = Some(c);
                n += 1;
            }
            _ => return false,
        }
    }
    n >= 3
}

fn fence(t: &str) -> Option<(char, usize)> {
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|x| *x == c).count();
    if n < 3 {
        return None;
    }
    if c == '`' && t.get(n..).is_some_and(|info| info.contains('`')) {
        return None;
    }
    Some((c, n))
}

#[derive(Clone, Copy, PartialEq)]
struct Marker {
    ordered: bool,
    /// Bullet char or ordered delimiter.
    ch: char,
    /// Bytes of the marker itself (after the indent).
    len: usize,
    start: u32,
}

fn list_marker(t: &str) -> Option<Marker> {
    let first = t.chars().next()?;
    let (m, len) = if matches!(first, '-' | '+' | '*') {
        (Marker { ordered: false, ch: first, len: 1, start: 1 }, 1)
    } else {
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if !(1..=9).contains(&digits) {
            return None;
        }
        let d = t.get(digits..)?.chars().next()?;
        if d != '.' && d != ')' {
            return None;
        }
        let start = t.get(..digits)?.parse::<u32>().unwrap_or(1);
        (Marker { ordered: true, ch: d, len: digits + 1, start }, digits + 1)
    };
    let rest = t.get(len..)?;
    if rest.is_empty() || rest.starts_with(' ') { Some(m) } else { None }
}

fn split_row(l: &str) -> Vec<String> {
    let t = l.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = if let Some(before) = t.strip_suffix('|') {
        let num_bs = before.chars().rev().take_while(|&c| c == '\\').count();
        if num_bs % 2 == 0 { before } else { t }
    } else {
        t
    };
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.peek() {
                Some(&'|') => {
                    cur.push('|');
                    chars.next();
                }
                Some(&'\\') => {
                    cur.push('\\');
                    cur.push('\\');
                    chars.next();
                }
                _ => cur.push('\\'),
            },
            '|' => cells.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

fn delimiter_row(l: &str) -> Option<Vec<Option<Align>>> {
    if !l.contains('-') {
        return None;
    }
    let cells = split_row(l);
    let mut out = Vec::new();
    for c in cells {
        let c = c.trim();
        let left = c.starts_with(':');
        let right = c.ends_with(':');
        let core = c.trim_matches(':');
        if core.is_empty() || !core.chars().all(|x| x == '-') {
            return None;
        }
        out.push(match (left, right) {
            (true, true) => Some(Align::Center),
            (false, true) => Some(Align::Right),
            _ => None,
        });
    }
    Some(out)
}

/// Does this line start a block that interrupts a paragraph?
fn interrupts(l: &str) -> bool {
    if indent(l) >= 4 {
        return false;
    }
    let t = l.trim_start();
    atx_heading(t).is_some()
        || thematic_break(t)
        || fence(t).is_some()
        || t.starts_with('>')
        || list_marker(t).is_some_and(|m| (!m.ordered || m.start == 1) && !is_blank(t.get(m.len..).unwrap_or("")))
}

fn flush_para(lines: &mut Vec<String>, out: &mut Vec<FBlock>, depth: usize) {
    if lines.is_empty() {
        return;
    }
    let text = join_lines(lines);
    lines.clear();
    let p = Para { inlines: parse_inlines(&text, &Fmt::default(), depth), ..Default::default() };
    out.push(FBlock::Para(p));
}

/// Paragraph lines → text with `\n` for hard breaks and spaces for soft ones.
fn join_lines(lines: &[String]) -> String {
    let mut s = String::new();
    let n = lines.len();
    for (i, l) in lines.iter().enumerate() {
        let l = l.trim_start();
        if i + 1 == n {
            s.push_str(l.trim_end());
        } else if l.ends_with("  ") {
            s.push_str(l.trim_end());
            s.push('\n');
        } else if l.ends_with('\\') && !l.ends_with("\\\\") {
            s.push_str(&l[..l.len() - 1]);
            s.push('\n');
        } else {
            s.push_str(l.trim_end());
            s.push(' ');
        }
    }
    s
}

fn code_para(line: &str) -> FBlock {
    let mut p = Para::new(Kind::Code);
    p.push_text(line, &Fmt::default());
    FBlock::Para(p)
}

/// Parse block structure. `level` is the list nesting level for lists found here.
fn parse_blocks(lines: &[String], depth: usize, level: u8) -> Vec<FBlock> {
    let mut out = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut i = 0;
    while let Some(line) = lines.get(i) {
        if is_blank(line) {
            flush_para(&mut para, &mut out, depth);
            i += 1;
            continue;
        }
        let ind = indent(line);
        if ind >= 4 {
            if !para.is_empty() {
                para.push(line.clone());
                i += 1;
                continue;
            }
            // Indented code.
            let mut code: Vec<&str> = Vec::new();
            while let Some(l) = lines.get(i) {
                if is_blank(l) {
                    code.push("");
                } else if indent(l) >= 4 {
                    code.push(strip_indent(l, 4));
                } else {
                    break;
                }
                i += 1;
            }
            while code.last().is_some_and(|l| l.is_empty()) {
                code.pop();
            }
            out.extend(code.into_iter().map(code_para));
            continue;
        }
        let t = line.trim_start();
        // An HTML comment on its own (CommonMark HTML block, type 2) isn't content. A range
        // marker followed by text on the same line is part of a paragraph.
        if para.is_empty() && t.starts_with("<!--") && !t.find("-->").and_then(|k| t.get(k + 3..)).is_some_and(|r| !r.trim().is_empty()) {
            while let Some(l) = lines.get(i) {
                i += 1;
                if l.contains("-->") {
                    break;
                }
            }
            continue;
        }
        // Setext heading underline.
        if !para.is_empty() {
            let u = t.trim_end();
            if !u.is_empty() && (u.chars().all(|c| c == '=') || (u.chars().all(|c| c == '-'))) {
                let lvl = if u.starts_with('=') { 1 } else { 2 };
                let text = join_lines(&para);
                para.clear();
                let mut p = Para::new(Kind::Heading(lvl));
                p.inlines = parse_inlines(&text, &Fmt::default(), depth);
                out.push(FBlock::Para(p));
                i += 1;
                continue;
            }
        }
        if let Some((n, body)) = atx_heading(t) {
            flush_para(&mut para, &mut out, depth);
            let mut p = Para::new(Kind::Heading(n));
            p.inlines = parse_inlines(body, &Fmt::default(), depth);
            out.push(FBlock::Para(p));
            i += 1;
            continue;
        }
        if thematic_break(t) {
            flush_para(&mut para, &mut out, depth);
            out.push(FBlock::Para(Para::new(Kind::Rule)));
            i += 1;
            continue;
        }
        if let Some((fc, fl)) = fence(t) {
            flush_para(&mut para, &mut out, depth);
            i += 1;
            while let Some(l) = lines.get(i) {
                i += 1;
                let lt = l.trim_start();
                if indent(l) < 4 && fence(lt).is_some_and(|(c, n)| c == fc && n >= fl) && lt.trim_start_matches(fc).trim().is_empty() {
                    break;
                }
                out.push(code_para(strip_indent(l, ind)));
            }
            continue;
        }
        if t.starts_with('>') {
            flush_para(&mut para, &mut out, depth);
            let mut inner: Vec<String> = Vec::new();
            while let Some(l) = lines.get(i) {
                let lt = l.trim_start();
                if indent(l) < 4 && lt.starts_with('>') {
                    let s = &lt[1..];
                    inner.push(s.strip_prefix(' ').unwrap_or(s).to_string());
                } else if !is_blank(l) && !interrupts(l) && inner.last().is_some_and(|x| !is_blank(x)) {
                    inner.push(l.clone());
                } else {
                    break;
                }
                i += 1;
            }
            if depth >= MAX_NEST {
                let mut p = Para::new(Kind::Quote);
                p.push_text(&inner.join(" "), &Fmt::default());
                out.push(FBlock::Para(p));
            } else {
                for mut b in parse_blocks(&inner, depth + 1, level) {
                    if let FBlock::Para(p) = &mut b
                        && p.kind == Kind::Normal
                    {
                        p.kind = Kind::Quote;
                    }
                    out.push(b);
                }
            }
            continue;
        }
        if para.is_empty()
            && t.contains('|')
            && let Some(aligns) = lines.get(i + 1).and_then(|l| delimiter_row(l))
        {
            let head = split_row(t);
            if head.len() == aligns.len() {
                let mut table = FTable::default();
                let cell = |s: &str, header: bool, a: Option<Align>| {
                    let p = Para { align: a, inlines: parse_inlines(s, &Fmt::default(), depth), ..Default::default() };
                    Cell { blocks: vec![FBlock::Para(p)], header, ..Default::default() }
                };
                table.rows.push(head.iter().zip(&aligns).map(|(s, a)| cell(s, true, *a)).collect());
                i += 2;
                while let Some(l) = lines.get(i) {
                    if is_blank(l) || !l.contains('|') || interrupts(l) {
                        break;
                    }
                    let mut cells = split_row(l);
                    cells.resize(aligns.len(), String::new());
                    table.rows.push(cells.iter().zip(&aligns).map(|(s, a)| cell(s, false, *a)).collect());
                    i += 1;
                }
                out.push(FBlock::Table(table));
                continue;
            }
        }
        if let Some(m) = list_marker(t)
            && (para.is_empty() || ((!m.ordered || m.start == 1) && !is_blank(t.get(m.len..).unwrap_or(""))))
        {
            flush_para(&mut para, &mut out, depth);
            i = parse_list(lines, i, depth, level, &mut out);
            continue;
        }
        para.push(line.clone());
        i += 1;
    }
    flush_para(&mut para, &mut out, depth);
    out
}

/// Parse a list starting at `lines[i]`; returns the index after it.
fn parse_list(lines: &[String], mut i: usize, depth: usize, level: u8, out: &mut Vec<FBlock>) -> usize {
    let Some(first) = lines.get(i).and_then(|l| list_marker(l.trim_start())) else { return i + 1 };
    while let Some(line) = lines.get(i) {
        let ind = indent(line);
        if ind >= 4 {
            break;
        }
        let t = line.trim_start();
        let Some(m) = list_marker(t) else { break };
        if m.ordered != first.ordered || m.ch != first.ch {
            break;
        }
        let after = t.get(m.len..).unwrap_or("");
        let spaces = indent(after);
        let content_indent = if is_blank(after) || spaces > 4 { ind + m.len + 1 } else { ind + m.len + spaces };
        let mut item: Vec<String> =
            vec![if is_blank(after) { String::new() } else { after.get(spaces.min(4).min(after.len())..).unwrap_or("").to_string() }];
        if spaces > 4 {
            item[0] = after.get(1..).unwrap_or("").to_string();
        }
        i += 1;
        while let Some(l) = lines.get(i) {
            if is_blank(l) {
                item.push(String::new());
            } else if indent(l) >= content_indent {
                item.push(strip_indent(l, content_indent).to_string());
            } else if item.last().is_some_and(|x| !is_blank(x)) && !interrupts(l) && list_marker(l.trim_start()).is_none() {
                // Lazy continuation of the item's paragraph.
                item.push(l.trim_start().to_string());
            } else {
                break;
            }
            i += 1;
        }
        // Blank lines at the end of an item belong between items.
        while item.len() > 1 && item.last().is_some_and(|x| x.is_empty()) {
            item.pop();
        }
        let li = ListInfo { ordered: first.ordered, level: level.min(8) };
        let inner = if depth >= MAX_NEST {
            let mut p = Para::default();
            p.push_text(&item.join(" "), &Fmt::default());
            vec![FBlock::Para(p)]
        } else {
            parse_blocks(&item, depth + 1, level.saturating_add(1).min(8))
        };
        let mut placed = false;
        if inner.is_empty() {
            out.push(FBlock::Para(Para { list: Some(li), ..Default::default() }));
            placed = true;
        }
        for b in inner {
            match b {
                FBlock::Para(mut p) if p.list.is_none() => {
                    if !placed {
                        p.list = Some(li);
                        placed = true;
                        out.push(FBlock::Para(p));
                    } else if let Some(FBlock::Para(prev)) = out.last_mut()
                        && prev.list.is_some_and(|x| x.level == li.level)
                    {
                        // A continuation paragraph joins the item with a line break.
                        prev.push_text("\n", &Fmt::default());
                        prev.inlines.extend(p.inlines);
                    } else {
                        p.list = Some(li);
                        out.push(FBlock::Para(p));
                    }
                }
                FBlock::Para(p) => {
                    if !placed {
                        // The item starts with a nested list: give it an empty first line.
                        out.push(FBlock::Para(Para { list: Some(li), ..Default::default() }));
                        placed = true;
                    }
                    out.push(FBlock::Para(p));
                }
                other => {
                    placed = true;
                    out.push(other);
                }
            }
        }
        // Blank lines between items.
        while lines.get(i).is_some_and(|l| is_blank(l)) {
            let next_is_item = lines
                .get(i + 1)
                .is_some_and(|l| indent(l) < 4 && list_marker(l.trim_start()).is_some_and(|m| m.ordered == first.ordered && m.ch == first.ch));
            if !next_is_item {
                return i;
            }
            i += 1;
        }
    }
    i
}

// ---- inlines ----

#[derive(Clone, Copy, PartialEq, Eq)]
enum Emph {
    Em,
    Strong,
    Strike,
}

enum Node {
    Text(String, Fmt),
    Inl(Inline),
    Delim { ch: char, orig: usize, left: usize, open: bool, close: bool, opens: Vec<Emph>, closes: Vec<Emph> },
}

fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation() || (!c.is_alphanumeric() && !c.is_whitespace() && !c.is_control())
}

/// Parse inline Markdown into formatted spans (on top of `base`).
pub fn parse_inlines(s: &str, base: &Fmt, depth: usize) -> Vec<Inline> {
    let nodes = tokenize(s, base, depth);
    emit(nodes)
}

fn push_text(nodes: &mut Vec<Node>, s: &str, f: &Fmt) {
    if s.is_empty() {
        return;
    }
    if let Some(Node::Text(t, pf)) = nodes.last_mut()
        && pf == f
    {
        t.push_str(s);
        return;
    }
    nodes.push(Node::Text(s.to_string(), f.clone()));
}

/// The closing `]` matching the `[` at byte `open` (escapes and code spans respected). Gives up
/// after looking `max` bytes ahead.
fn matching_bracket(s: &str, open: usize, max: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while let Some(&c) = b.get(i) {
        if i - open > max {
            return None;
        }
        match c {
            b'\\' => i += 1,
            b'`' => {
                let n = b.get(i..).map(|r| r.iter().take_while(|x| **x == b'`').count()).unwrap_or(1);
                if let Some(end) = find_backticks(s, i + n, n) {
                    i = end + n - 1;
                } else {
                    i += n - 1;
                }
            }
            b'[' => depth += 1,
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Byte index of a run of exactly `n` backticks at or after `from`.
fn find_backticks(s: &str, from: usize, n: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = from;
    while let Some(&c) = b.get(i) {
        if c == b'`' {
            let k = b.get(i..).map(|r| r.iter().take_while(|x| **x == b'`').count()).unwrap_or(1);
            if k == n {
                return Some(i);
            }
            i += k;
        } else {
            i += 1;
        }
    }
    None
}

/// Parse `(dest "title")` at byte `at` (which holds `(`): (destination, index after `)`).
fn link_dest(s: &str, at: usize) -> Option<(String, usize)> {
    let rest = s.get(at..)?.strip_prefix('(')?;
    let mut i = at + 1;
    let ws = rest.len() - rest.trim_start().len();
    i += ws;
    let r = s.get(i..)?;
    let (dest, used) = if let Some(inner) = r.strip_prefix('<') {
        let end = inner.find(['>', '\n'])?;
        if inner.get(end..)?.starts_with('\n') {
            return None;
        }
        (inner.get(..end)?.to_string(), end + 2)
    } else {
        let mut depth = 0i32;
        let mut end = r.len();
        let mut chars = r.char_indices().peekable();
        while let Some((k, c)) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '(' => depth += 1,
                ')' if depth == 0 => {
                    end = k;
                    break;
                }
                ')' => depth -= 1,
                c if c.is_whitespace() => {
                    end = k;
                    break;
                }
                _ => {}
            }
        }
        (r.get(..end)?.to_string(), end)
    };
    i += used;
    let r = s.get(i..)?;
    let ws = r.len() - r.trim_start().len();
    i += ws;
    let r = s.get(i..)?;
    if let Some(q) = r.chars().next().filter(|c| matches!(c, '"' | '\'' | '(')) {
        let close = if q == '(' { ')' } else { q };
        let end = r.get(1..)?.find(close)?;
        i += end + 2;
        let r = s.get(i..)?;
        i += r.len() - r.trim_start().len();
    }
    if s.get(i..)?.starts_with(')') { Some((unescape(&dest), i + 1)) } else { None }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(n) = chars.peek().copied().filter(|n| n.is_ascii_punctuation())
        {
            out.push(n);
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// The value of attribute `name` in an HTML tag's text.
fn tag_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(k) = lower.get(from..)?.find(name) {
        let at = from + k;
        let before = lower.get(..at)?.chars().next_back();
        let after = lower.get(at + name.len()..)?.trim_start();
        if before.is_some_and(|c| c.is_whitespace()) && after.starts_with('=') {
            let v = after.get(1..)?.trim_start();
            let q = v.chars().next()?;
            return Some(if q == '"' || q == '\'' {
                let inner = v.get(1..)?;
                let off = tag.len() - inner.len();
                let end = inner.find(q)?;
                tag.get(off..off + end)?.to_string()
            } else {
                let end = v.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(v.len());
                let off = tag.len() - v.len();
                tag.get(off..off + end)?.to_string()
            });
        }
        from = at + name.len();
    }
    None
}

fn tokenize(s: &str, base: &Fmt, depth: usize) -> Vec<Node> {
    let mut nodes: Vec<Node> = Vec::new();
    // Bytes the link and image scans below may look ahead in total. Each unmatched `[` or `![(`
    // would otherwise rescan the rest of the text, which is quadratic on hostile input; once the
    // budget is gone, further brackets are plain text.
    let mut scan_budget = s.len().saturating_mul(8).saturating_add(8192);
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut text_start = 0usize;
    macro_rules! flush {
        () => {
            if text_start < i {
                push_text(&mut nodes, s.get(text_start..i).unwrap_or(""), base);
            }
        };
    }
    while let Some(&c) = b.get(i) {
        match c {
            b'\\' => {
                flush!();
                match s.get(i + 1..).and_then(|r| r.chars().next()) {
                    Some(n) if n.is_ascii_punctuation() => {
                        push_text(&mut nodes, &n.to_string(), base);
                        i += 2;
                    }
                    _ => {
                        push_text(&mut nodes, "\\", base);
                        i += 1;
                    }
                }
                text_start = i;
            }
            b'`' => {
                flush!();
                let n = b.get(i..).map(|r| r.iter().take_while(|x| **x == b'`').count()).unwrap_or(1);
                match find_backticks(s, i + n, n) {
                    Some(end) => {
                        let mut code = s.get(i + n..end).unwrap_or("").replace('\n', " ");
                        if code.len() >= 2 && code.starts_with(' ') && code.ends_with(' ') && !code.trim().is_empty() {
                            code = code[1..code.len() - 1].to_string();
                        }
                        let mut f = base.clone();
                        f.code = true;
                        push_text(&mut nodes, &code, &f);
                        i = end + n;
                    }
                    None => {
                        push_text(&mut nodes, s.get(i..i + n).unwrap_or(""), base);
                        i += n;
                    }
                }
                text_start = i;
            }
            b'*' | b'_' | b'~' => {
                flush!();
                let ch = c as char;
                let n = b.get(i..).map(|r| r.iter().take_while(|x| **x == c).count()).unwrap_or(1);
                let before = s.get(..i).and_then(|r| r.chars().next_back()).unwrap_or(' ');
                let after = s.get(i + n..).and_then(|r| r.chars().next()).unwrap_or(' ');
                let lf = !after.is_whitespace() && (!is_punct(after) || before.is_whitespace() || is_punct(before));
                let rf = !before.is_whitespace() && (!is_punct(before) || after.is_whitespace() || is_punct(after));
                let (open, close) = match ch {
                    '_' => (lf && (!rf || is_punct(before)), rf && (!lf || is_punct(after))),
                    '~' if n != 2 => (false, false),
                    _ => (lf, rf),
                };
                if open || close {
                    nodes.push(Node::Delim { ch, orig: n, left: n, open, close, opens: Vec::new(), closes: Vec::new() });
                } else {
                    push_text(&mut nodes, s.get(i..i + n).unwrap_or(""), base);
                }
                i += n;
                text_start = i;
            }
            b'[' if footnote_ref(s, i).is_some() => {
                flush!();
                if let Some((blocks, end)) = footnote_ref(s, i) {
                    nodes.push(Node::Inl(Inline::Note { endnote: false, blocks }));
                    i = end;
                } else {
                    i += 1;
                }
                text_start = i;
            }
            b'!' | b'[' if depth < MAX_NEST => {
                let img = c == b'!';
                let open = if img { i + 1 } else { i };
                let parsed = if b.get(open) == Some(&b'[') && scan_budget > 0 {
                    let found = matching_bracket(s, open, scan_budget);
                    scan_budget = scan_budget.saturating_sub(found.map_or(scan_budget, |close| close - open));
                    found.and_then(|close| {
                        // A destination that does not parse may have been read to the end of the text.
                        scan_budget = scan_budget.saturating_sub(s.len().saturating_sub(close));
                        let (dest, end) = link_dest(s, close + 1)?;
                        Some((s.get(open + 1..close)?.to_string(), dest, end))
                    })
                } else {
                    None
                };
                match parsed {
                    Some((inner, dest, end)) if img => {
                        flush!();
                        let alt: String = parse_inlines(&inner, &Fmt::default(), depth + 1)
                            .iter()
                            .filter_map(|x| if let Inline::Text(t, _) = x { Some(t.as_str()) } else { None })
                            .collect();
                        let data = match model::data_uri(&dest) {
                            Some(d) => Some(Arc::new(d)),
                            None => image_ref(&dest),
                        };
                        match data.and_then(|d| make_img(d, None, None, &alt)) {
                            Some(im) => nodes.push(Node::Inl(Inline::Image(im))),
                            None => push_text(&mut nodes, &alt, base),
                        }
                        i = end;
                        text_start = i;
                    }
                    Some((inner, dest, end)) if base.link.is_none() => {
                        flush!();
                        let mut f = base.clone();
                        f.link = Some(dest);
                        for x in parse_inlines(&inner, &f, depth + 1) {
                            nodes.push(Node::Inl(x));
                        }
                        i = end;
                        text_start = i;
                    }
                    _ => i += 1,
                }
            }
            b'<' if s.get(i..).is_some_and(|r| r.starts_with("<!--")) => {
                let rest = s.get(i + 4..).unwrap_or("");
                match within(rest, MAX_TAG).find("-->") {
                    Some(k) => {
                        flush!();
                        if let Some((id, end)) = rest.get(..k).and_then(|inner| comment_marker(inner.trim())) {
                            nodes.push(Node::Inl(Inline::Comment { id, end }));
                        }
                        i += 4 + k + 3;
                        text_start = i;
                    }
                    None => i += 1,
                }
            }
            b'<' => {
                let rest = s.get(i + 1..).unwrap_or("");
                let end = within(rest, MAX_TAG).find('>');
                let inner = end.and_then(|e| rest.get(..e)).unwrap_or("");
                let lower = inner.to_ascii_lowercase();
                let is_auto = !inner.is_empty()
                    && !inner.contains([' ', '<', '\n'])
                    && (inner.contains("://") || lower.starts_with("mailto:") || (inner.contains('@') && !inner.contains('/')));
                if is_auto {
                    flush!();
                    let mut f = base.clone();
                    f.link = Some(if inner.contains('@') && !inner.contains(':') { format!("mailto:{inner}") } else { inner.to_string() });
                    push_text(&mut nodes, inner, &f);
                    i += inner.len() + 2;
                    text_start = i;
                } else if matches!(lower.trim_end_matches('/').trim(), "br") {
                    flush!();
                    push_text(&mut nodes, "\n", base);
                    i += inner.len() + 2;
                    text_start = i;
                } else if lower.starts_with("a ") && rest.get(inner.len() + 1..).is_some_and(|r| r.to_ascii_lowercase().starts_with("</a>")) {
                    flush!();
                    if let Some(name) = tag_attr(inner, "id").or_else(|| tag_attr(inner, "name")) {
                        nodes.push(Node::Inl(Inline::Anchor(name)));
                    }
                    i += inner.len() + 2 + 4;
                    text_start = i;
                } else {
                    i += 1;
                }
            }
            b'&' => {
                let rest = s.get(i..).unwrap_or("");
                match within(rest, 33).find(';').and_then(|k| decode_entity(rest.get(1..k)?).map(|ch| (ch, k))) {
                    Some((ch, k)) => {
                        flush!();
                        push_text(&mut nodes, &ch, base);
                        i += k + 1;
                        text_start = i;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    flush!();
    nodes
}

/// Longest `<...>` tag or autolink taken as inline HTML.
const MAX_TAG: usize = 4096;

/// The first `n` bytes of `s` (fewer if that would cut a character), so a search for a closing
/// character stays short however long the text is.
fn within(s: &str, n: usize) -> &str {
    let mut end = n.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.get(..end).unwrap_or("")
}

fn emph_index(ch: char) -> usize {
    match ch {
        '*' => 0,
        '_' => 1,
        _ => 2,
    }
}

/// The CommonMark "process emphasis" pass over the delimiter runs, then flattening into spans.
fn emit(mut nodes: Vec<Node>) -> Vec<Inline> {
    // Delimiter stack of node indexes; bottoms per (char, closer length % 3, closer can open).
    let mut stack: Vec<usize> = Vec::new();
    let mut bottom = [[[None::<usize>; 2]; 3]; 3];
    for c in 0..nodes.len() {
        let (ch, corig, copen, cclose) = match nodes.get(c) {
            Some(Node::Delim { ch, orig, open, close, .. }) => (*ch, *orig, *open, *close),
            _ => continue,
        };
        if cclose {
            loop {
                let cleft = match nodes.get(c) {
                    Some(Node::Delim { left, .. }) => *left,
                    _ => 0,
                };
                if cleft == 0 {
                    break;
                }
                let bot = bottom.get(emph_index(ch)).and_then(|x| x.get(corig % 3)).and_then(|x| x.get(copen as usize)).copied().flatten();
                let mut found = None;
                for (k, &o) in stack.iter().enumerate().rev() {
                    if bot.is_some_and(|b| o <= b) {
                        break;
                    }
                    if let Some(Node::Delim { ch: och, orig: oorig, left: oleft, open: oopen, close: oclose, .. }) = nodes.get(o) {
                        if *och != ch || !*oopen || *oleft == 0 {
                            continue;
                        }
                        if ch == '~' && (*oleft < 2 || cleft < 2) {
                            continue;
                        }
                        let odd = (*oclose || copen) && (oorig + corig) % 3 == 0 && !(oorig % 3 == 0 && corig % 3 == 0);
                        if odd {
                            continue;
                        }
                        found = Some((k, o, *oleft));
                        break;
                    }
                }
                let Some((k, o, oleft)) = found else {
                    if let Some(slot) = bottom.get_mut(emph_index(ch)).and_then(|x| x.get_mut(corig % 3)).and_then(|x| x.get_mut(copen as usize)) {
                        *slot = Some(c.saturating_sub(1));
                    }
                    break;
                };
                let (used, kind) = if ch == '~' {
                    (2, Emph::Strike)
                } else if oleft >= 2 && cleft >= 2 {
                    (2, Emph::Strong)
                } else {
                    (1, Emph::Em)
                };
                if let Some(Node::Delim { left, opens, .. }) = nodes.get_mut(o) {
                    *left -= used;
                    opens.push(kind);
                }
                if let Some(Node::Delim { left, closes, .. }) = nodes.get_mut(c) {
                    *left -= used;
                    closes.push(kind);
                }
                // Delimiters between opener and closer can no longer match.
                stack.truncate(k + 1);
                if oleft - used == 0 {
                    stack.truncate(k);
                }
            }
        }
        let left = match nodes.get(c) {
            Some(Node::Delim { left, .. }) => *left,
            _ => 0,
        };
        if copen && left > 0 {
            stack.push(c);
        }
    }
    // Flatten.
    let mut out: Vec<Inline> = Vec::new();
    let (mut em, mut strong, mut strike) = (0usize, 0usize, 0usize);
    let push = |out: &mut Vec<Inline>, t: &str, f: &Fmt, em: usize, strong: usize, strike: usize| {
        if t.is_empty() {
            return;
        }
        let mut f = f.clone();
        f.italic |= em > 0;
        f.bold |= strong > 0;
        f.strike |= strike > 0;
        if let Some(Inline::Text(pt, pf)) = out.last_mut()
            && *pf == f
        {
            pt.push_str(t);
            return;
        }
        out.push(Inline::Text(t.to_string(), f));
    };
    let base_of = |nodes: &[Node], k: usize| -> Fmt {
        // Delimiter literals take the formatting (link, …) of neighbouring text.
        for j in (0..k).rev() {
            if let Some(Node::Text(_, f)) = nodes.get(j) {
                return Fmt { bold: false, italic: false, strike: false, ..f.clone() };
            }
        }
        Fmt::default()
    };
    for (k, n) in nodes.iter().enumerate() {
        match n {
            Node::Text(t, f) => push(&mut out, t, f, em, strong, strike),
            Node::Inl(Inline::Text(t, f)) => push(&mut out, t, f, em, strong, strike),
            Node::Inl(other) => out.push(other.clone()),
            Node::Delim { ch, left, opens, closes, .. } => {
                for e in closes {
                    match e {
                        Emph::Em => em = em.saturating_sub(1),
                        Emph::Strong => strong = strong.saturating_sub(1),
                        Emph::Strike => strike = strike.saturating_sub(1),
                    }
                }
                let lit: String = std::iter::repeat_n(*ch, *left).collect();
                let f = base_of(&nodes, k);
                push(&mut out, &lit, &f, em, strong, strike);
                for e in opens {
                    match e {
                        Emph::Em => em += 1,
                        Emph::Strong => strong += 1,
                        Emph::Strike => strike += 1,
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Export

/// Escape text for Markdown (inline context).
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '~' | '|' | '&' => {
                out.push('\\');
                out.push(c);
            }
            '\u{000C}' | '\u{000E}' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Escape what would start a block at the beginning of a line.
fn escape_line_start(s: String) -> String {
    let s = s.trim_start().to_string();
    let t = s.as_str();
    let lead = s.len() - t.len();
    let needs = t.starts_with('#')
        || t.starts_with('>')
        || t.starts_with("- ")
        || t.starts_with("+ ")
        || t == "-"
        || t == "+"
        || t.starts_with('=')
        || thematic_break(t)
        || {
            let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
            d > 0 && t.get(d..).is_some_and(|r| r.starts_with(". ") || r.starts_with(") ") || r == "." || r == ")")
        };
    if !needs {
        return s;
    }
    if let Some(d) = t.find(['.', ')']).filter(|_| t.starts_with(|c: char| c.is_ascii_digit())) {
        return format!("{}{}\\{}", &s[..lead], &t[..d], &t[d..]);
    }
    format!("{}\\{}", &s[..lead], t)
}

fn code_span(t: &str) -> String {
    let mut longest = 0;
    let mut cur = 0;
    for c in t.chars() {
        if c == '`' {
            cur += 1;
            longest = longest.max(cur);
        } else {
            cur = 0;
        }
    }
    let ticks = "`".repeat(longest + 1);
    let pad = if t.starts_with('`') || t.ends_with('`') || (t.starts_with(' ') && t.ends_with(' ') && !t.trim().is_empty()) { " " } else { "" };
    format!("{ticks}{pad}{t}{pad}{ticks}")
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Mark {
    Bold,
    Italic,
    Strike,
}

/// What Markdown can't hold, found while writing a native copy ([`export_native`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Adaptation {
    /// Underlined text (not a link): written plain.
    Underline,
    /// Superscript or subscript: written on the baseline.
    SupSub,
    /// Text colour or highlight: left to the publication profile.
    Color,
    /// Centred, right or justified paragraph: left to the publication profile.
    Alignment,
    /// A page break before a paragraph.
    PageBreak,
    /// A bookmark that isn't a heading: links to it lose their target.
    Bookmark,
    /// A table cell with several paragraphs or line breaks: joined on one line.
    CellParagraphs,
    /// A table inside a table: its text joined into the cell.
    NestedTable,
    /// A merged cell: written as empty cells.
    MergedCells,
    /// A chart, diagram or drawing: kept as its alt text.
    Figure,
    /// An equation: written as its linear text.
    Equation,
    /// An endnote: written as a footnote.
    Endnote,
    /// A table used for layout (no rules, or one column): its cells written as paragraphs.
    LayoutTable,
    /// A picture in a format browsers don't show (EMF, WMF…) kept as is.
    ImageFormat,
}

impl Adaptation {
    pub fn describe(self) -> &'static str {
        match self {
            Adaptation::Underline => "underlined text written without underline",
            Adaptation::SupSub => "superscript/subscript written on the baseline",
            Adaptation::Color => "text colour or highlight left to the publication profile",
            Adaptation::Alignment => "paragraph alignment left to the publication profile",
            Adaptation::PageBreak => "page breaks left out",
            Adaptation::Bookmark => "bookmarks (not on a heading) left out",
            Adaptation::CellParagraphs => "table cells with several lines joined on one line",
            Adaptation::NestedTable => "nested tables flattened into their cell",
            Adaptation::MergedCells => "merged cells written as empty cells",
            Adaptation::Figure => "charts and drawings kept as their alt text",
            Adaptation::Equation => "equations written as linear text",
            Adaptation::Endnote => "endnotes written as footnotes",
            Adaptation::LayoutTable => "layout tables written as paragraphs",
            Adaptation::ImageFormat => "pictures in formats browsers can't show (EMF, WMF…)",
        }
    }
}

/// How many times each [`Adaptation`] happened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub counts: BTreeMap<Adaptation, usize>,
    /// Pictures written as asset files.
    pub images: usize,
    /// Comments kept as HTML comments.
    pub comments: usize,
}

impl Report {
    fn note(&mut self, a: Adaptation) {
        let n = self.counts.entry(a).or_insert(0);
        *n = n.saturating_add(1);
    }
    /// Nothing was adapted.
    pub fn is_lossless(&self) -> bool {
        self.counts.is_empty()
    }
    /// One line per adaptation: `3 × underlined text written without underline`.
    pub fn lines(&self) -> Vec<String> {
        self.counts.iter().map(|(a, n)| format!("{n} × {}", a.describe())).collect()
    }
}

/// How [`export_native`] writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeOptions {
    /// Folder, relative to the Markdown file, the pictures go in (`assets`). Empty = beside it.
    pub assets_dir: String,
    /// Keep the document's comments, as an HTML comment block at the end.
    pub comments: bool,
}

impl Default for NativeOptions {
    fn default() -> Self {
        NativeOptions { assets_dir: "assets".into(), comments: true }
    }
}

/// A Markdown-native copy: the text, the asset files it references and what was adapted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bundle {
    pub text: String,
    /// (path relative to the Markdown file, bytes), each path once.
    pub assets: Vec<(String, Arc<Vec<u8>>)>,
    pub report: Report,
}

/// Writer state: the legacy copy ([`export`]) or the native profile ([`export_native`]).
#[derive(Default)]
struct Ctx {
    native: bool,
    assets_dir: String,
    assets: Vec<(String, Arc<Vec<u8>>)>,
    report: Report,
    /// Notes met so far, numbered in order: written as definitions after the body.
    notes: Vec<Vec<FBlock>>,
    /// Bookmark name → slug of the heading it sits in (native copies link to headings by slug).
    slugs: BTreeMap<String, String>,
    /// Write comment range markers and the comment block.
    comments: bool,
    /// Document comment id → its thread number (`c1`…), for the threads written.
    comment_keys: BTreeMap<u32, u32>,
}

impl Ctx {
    fn native(opts: &NativeOptions) -> Ctx {
        let dir = opts.assets_dir.trim().trim_matches(['/', '\\']).replace('\\', "/");
        Ctx { native: true, assets_dir: dir, comments: opts.comments, ..Default::default() }
    }

    fn note(&mut self, a: Adaptation) {
        if self.native {
            self.report.note(a);
        }
    }

    /// The asset path of `img`: `assets/img-<hash>.<ext>`, the same for the same bytes, so saving
    /// again doesn't rename files and identical pictures are stored once.
    fn asset(&mut self, img: &Img) -> String {
        let converted = web_image(img);
        let img = converted.as_ref().unwrap_or(img);
        if !matches!(img.ext.as_str(), "png" | "jpeg" | "jpg" | "gif" | "webp" | "bmp" | "svg") {
            self.note(Adaptation::ImageFormat);
        }
        let ext: String = img.ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(5).collect::<String>().to_ascii_lowercase();
        let ext = if ext.is_empty() { "bin".to_string() } else { ext };
        let file = format!("img-{:016x}.{ext}", fnv64(&img.data));
        let path = if self.assets_dir.is_empty() { file } else { format!("{}/{file}", self.assets_dir) };
        if !self.assets.iter().any(|(p, _)| *p == path) {
            self.assets.push((path.clone(), img.data.clone()));
            self.report.images = self.report.images.saturating_add(1);
        }
        path
    }
}

/// `img` as a PNG when browsers can't show its format but the image crate reads it (TIFF…).
fn web_image(img: &Img) -> Option<Img> {
    if matches!(img.ext.as_str(), "png" | "jpeg" | "jpg" | "gif" | "webp" | "bmp" | "svg") {
        return None;
    }
    let decoded = image::load_from_memory(&img.data).ok()?;
    let mut png = Vec::new();
    decoded.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some(Img { data: Arc::new(png), ext: "png".into(), ..img.clone() })
}

/// The anchor GitHub gives a heading: lower case, punctuation dropped, spaces as `-`.
pub fn heading_slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c)
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

/// Bookmarks inside headings → their heading's slug (`-1`, `-2`… for repeated headings).
fn heading_slugs(blocks: &[FBlock]) -> BTreeMap<String, String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for b in blocks {
        let FBlock::Para(p) = b else { continue };
        if !matches!(p.kind, Kind::Heading(_) | Kind::Title) {
            continue;
        }
        let base = heading_slug(&p.text());
        let n = seen.entry(base.clone()).or_insert(0);
        let slug = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
        *n += 1;
        for i in &p.inlines {
            if let Inline::Anchor(a) = i {
                out.insert(a.clone(), slug.clone());
            }
        }
    }
    out
}

/// FNV-1a, 64 bits: a stable content hash for asset names (not for security).
fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Inline spans → Markdown with properly nested markers.
#[cfg(test)]
fn inlines_md(inlines: &[Inline]) -> String {
    inlines_md_impl(inlines, false, &mut Ctx::default())
}

fn inlines_md_impl(inlines: &[Inline], in_table: bool, ctx: &mut Ctx) -> String {
    // Split into (text, fmt) pieces with whitespace moved outside emphasis.
    let mut pieces: Vec<(String, Fmt, Option<String>)> = Vec::new();
    if inlines.iter().any(|i| matches!(i, Inline::Equation { .. })) {
        ctx.note(Adaptation::Equation);
    }
    let inlines = crate::model::equations_as_text(inlines);
    for i in inlines.iter() {
        if ctx.native
            && let Inline::Text(t, f) = i
            && !t.trim().is_empty()
        {
            if f.underline {
                ctx.note(Adaptation::Underline);
            }
            if f.sup || f.sub {
                ctx.note(Adaptation::SupSub);
            }
            // A link's colour is the link style's, not the author's.
            if (f.link.is_none() && f.color.is_some_and(|c| c != wordcraft_doc::Rgb::BLACK)) || f.background.is_some() {
                ctx.note(Adaptation::Color);
            }
        }
        match i {
            Inline::Text(t, f) => {
                let has_emph = f.bold || f.italic || f.strike;
                if has_emph && !f.code {
                    let core = t.trim_matches([' ', '\t', '\n']);
                    let lead = &t[..t.len() - t.trim_start_matches([' ', '\t', '\n']).len()];
                    let trail = &t[t.trim_end_matches([' ', '\t', '\n']).len()..];
                    let plain = Fmt { bold: false, italic: false, strike: false, ..f.clone() };
                    if !lead.is_empty() {
                        pieces.push((lead.to_string(), plain.clone(), None));
                    }
                    if !core.is_empty() {
                        pieces.push((core.to_string(), f.clone(), None));
                    }
                    if !trail.is_empty() && !core.is_empty() {
                        pieces.push((trail.to_string(), plain, None));
                    }
                } else {
                    pieces.push((t.clone(), f.clone(), None));
                }
            }
            Inline::Image(img) => {
                let name = if ctx.native { link_target(&ctx.asset(img)) } else { format!("image.{}", img.ext) };
                pieces.push((String::new(), Fmt::default(), Some(format!("![{}]({})", escape(&img.alt), name))));
            }
            // No HTML in a native copy: headings get their anchors from the renderer's slugs.
            Inline::Anchor(a) if ctx.native => {
                if !ctx.slugs.contains_key(a) {
                    ctx.note(Adaptation::Bookmark);
                }
            }
            Inline::Comment { id, end } => {
                if let Some(n) = ctx.comment_keys.get(id).filter(|_| ctx.native) {
                    let slash = if *end { "/" } else { "" };
                    pieces.push((String::new(), Fmt::default(), Some(format!("<!-- {slash}c{n} -->"))));
                }
            }
            Inline::Note { endnote, blocks } => {
                if *endnote {
                    ctx.note(Adaptation::Endnote);
                }
                ctx.notes.push(blocks.clone());
                pieces.push((String::new(), Fmt::default(), Some(format!("[^{}]", ctx.notes.len()))));
            }
            Inline::Anchor(a) => {
                let safe: String = a.chars().filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')).collect();
                pieces.push((String::new(), Fmt::default(), Some(format!("<a id=\"{safe}\"></a>"))));
            }
            Inline::Figure(alt) if ctx.native => {
                ctx.note(Adaptation::Figure);
                pieces.push((format!("[{}]", alt.trim()), Fmt { italic: true, ..Fmt::default() }, None));
            }
            Inline::Figure(alt) => pieces.push((String::new(), Fmt::default(), Some(format!("![{}]()", escape(alt))))),
            Inline::Equation { .. } => {}
        }
    }
    let mut out = String::new();
    let mut stack: Vec<Mark> = Vec::new();
    let mut link: Option<String> = None;
    let close_all = |out: &mut String, stack: &mut Vec<Mark>| {
        while let Some(m) = stack.pop() {
            out.push_str(marker(m));
        }
    };
    for (text, f, raw) in pieces {
        // Links are the outermost construct; a script or other non-web target is plain text.
        let f_link = f.link.clone().filter(|l| wordcraft_doc::link::is_safe_href(l)).map(|l| match l.strip_prefix('#') {
            Some(name) if ctx.native => ctx.slugs.get(name).map(|slug| format!("#{slug}")).unwrap_or(l),
            _ => l,
        });
        if f_link != link || raw.is_some() {
            close_all(&mut out, &mut stack);
            if let Some(l) = &link {
                out.push_str(&format!("]({})", link_target(l)));
            }
            link = None;
            if let Some(r) = raw {
                out.push_str(&r);
                continue;
            }
            if let Some(l) = f_link {
                out.push('[');
                link = Some(l);
            }
        }
        let want: Vec<Mark> =
            [(f.bold, Mark::Bold), (f.italic, Mark::Italic), (f.strike, Mark::Strike)].iter().filter(|x| x.0).map(|x| x.1).collect();
        // Close until the stack is a prefix of what we want.
        while let Some(pos) = stack.iter().position(|m| !want.contains(m)) {
            while stack.len() > pos {
                if let Some(m) = stack.pop() {
                    out.push_str(marker(m));
                }
            }
        }
        for m in want {
            if !stack.contains(&m) {
                out.push_str(marker(m));
                stack.push(m);
            }
        }
        if f.code {
            if in_table {
                let t = text.replace('|', "\\|");
                out.push_str(&code_span(&t));
            } else {
                out.push_str(&code_span(&text));
            }
        } else {
            out.push_str(&escape(&text).replace('\n', "\\\n").replace('\t', "    "));
        }
    }
    close_all(&mut out, &mut stack);
    if let Some(l) = &link {
        out.push_str(&format!("]({})", link_target(l)));
    }
    out
}

fn link_target(l: &str) -> String {
    if l.contains([' ', '(', ')', '<', '>']) { format!("<{}>", l.replace(['<', '>', '\n'], "")) } else { l.to_string() }
}

fn marker(m: Mark) -> &'static str {
    match m {
        Mark::Bold => "**",
        Mark::Italic => "*",
        Mark::Strike => "~~",
    }
}

fn table_md(t: &FTable, out: &mut Vec<String>, ctx: &mut Ctx) {
    let cols = t.cols().max(1);
    // A native copy has no `<br>`: the lines of a cell are joined with a space.
    let cell_md = |c: &Cell, ctx: &mut Ctx| -> String {
        let br = if ctx.native { " " } else { "<br>" };
        let mut parts = Vec::new();
        for b in &c.blocks {
            match b {
                FBlock::Para(p) => {
                    let md = inlines_md_impl(&p.inlines, true, ctx);
                    if md.contains("\\\n") {
                        ctx.note(Adaptation::CellParagraphs);
                    }
                    parts.push(md.replace("\\\n", br));
                }
                FBlock::Table(inner) => {
                    ctx.note(Adaptation::NestedTable);
                    parts.push(escape(&inner.rows.iter().flat_map(|r| r.iter().map(Cell::text)).collect::<Vec<_>>().join(" ")))
                }
            }
        }
        if parts.iter().filter(|p| !p.is_empty()).count() > 1 {
            ctx.note(Adaptation::CellParagraphs);
        }
        let parts: Vec<String> = if ctx.native { parts.into_iter().filter(|p| !p.is_empty()).collect() } else { parts };
        parts.join(br).replace('\n', " ")
    };
    let align_of = |c: &Cell| c.blocks.iter().find_map(|b| if let FBlock::Para(p) = b { Some(p.align) } else { None }).flatten();
    let row_md = |r: &Vec<Cell>, ctx: &mut Ctx| -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for c in r {
            v.push(if c.covered { String::new() } else { cell_md(c, ctx) });
            if c.covered || c.colspan > 1 || c.rowspan > 1 {
                ctx.note(Adaptation::MergedCells);
            }
            for _ in 1..c.colspan.clamp(1, 63) {
                v.push(String::new());
            }
        }
        v.resize(cols, String::new());
        v
    };
    let Some(head) = t.rows.first() else { return };
    out.push(format!("| {} |", row_md(head, ctx).join(" | ")));
    let mut aligns: Vec<Option<Align>> = Vec::new();
    for c in head {
        aligns.push(align_of(c));
        for _ in 1..c.colspan.clamp(1, 63) {
            aligns.push(None);
        }
    }
    aligns.resize(cols, None);
    let delim: Vec<&str> = aligns
        .iter()
        .map(|a| match a {
            Some(Align::Center) => ":---:",
            Some(Align::Right) => "---:",
            _ => "---",
        })
        .collect();
    out.push(format!("| {} |", delim.join(" | ")));
    for r in t.rows.iter().skip(1) {
        out.push(format!("| {} |", row_md(r, ctx).join(" | ")));
    }
}

/// Markdown text of a document.
pub fn export(doc: &Document) -> String {
    write_doc(doc, &mut Ctx::default())
}

fn write_doc(doc: &Document, ctx: &mut Ctx) -> String {
    let flow = model::from_doc_markdown(doc);
    let mut out: Vec<String> = Vec::new();
    if ctx.native {
        ctx.slugs = heading_slugs(&flow.blocks);
        if ctx.comments {
            ctx.comment_keys = comment_keys(doc, &flow.blocks);
        }
        front_matter_md(&flow.meta, &mut out);
    }
    write_blocks(&flow.blocks, &mut out, ctx);
    // Footnote definitions, numbered in reference order; a note's later lines are indented.
    let mut k = 0;
    while let Some(blocks) = ctx.notes.get(k).cloned() {
        k += 1;
        let mut body: Vec<String> = Vec::new();
        write_blocks(&blocks, &mut body, ctx);
        while body.last().is_some_and(|l| l.is_empty()) {
            body.pop();
        }
        let mut lines = body.into_iter();
        out.push(format!("[^{k}]: {}", lines.next().unwrap_or_default()).trim_end().to_string());
        for l in lines {
            out.push(if l.is_empty() { l } else { format!("    {l}") });
        }
        out.push(String::new());
    }
    let mut s = out.join("\n");
    while s.ends_with("\n\n") {
        s.pop();
    }
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// The Markdown-native copy of `doc`: CommonMark + GFM (tables, strikethrough) with no HTML
/// except HTML comments, its pictures as asset files, and a report of what was adapted. Writing
/// it, reading it back with [`import_with`] and writing again gives the same text.
pub fn export_native(doc: &Document, opts: &NativeOptions) -> Bundle {
    let mut ctx = Ctx::native(opts);
    let mut text = write_doc(doc, &mut ctx);
    if !ctx.comment_keys.is_empty() {
        ctx.report.comments = ctx.comment_keys.len();
        text.push('\n');
        text.push_str(&comments_block(doc, &ctx.comment_keys));
    }
    Bundle { text, assets: ctx.assets, report: ctx.report }
}

/// How a Markdown document looks while it is edited: the look of the site it will be read on.
/// Only the editing view changes; the Markdown file never carries any of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    /// As GitHub, GitLab and Forgejo show Markdown: a sans-serif text, rules under the first two
    /// heading levels, quotes with a bar, shaded code.
    #[default]
    Gfm,
    /// As a VitePress documentation site shows it: a rule above second-level headings, roomier
    /// lines.
    VitePress,
}

impl Theme {
    pub fn from_name(name: &str) -> Option<Theme> {
        match name.trim().to_ascii_lowercase().as_str() {
            "gfm" | "github" | "gitlab" | "forgejo" => Some(Theme::Gfm),
            "vitepress" => Some(Theme::VitePress),
            _ => None,
        }
    }
}

/// Give `doc` (read from Markdown) the look of `theme`: body text, headings, quotes and code.
pub fn apply_theme(doc: &mut Document, theme: Theme) {
    use wordcraft_doc::props::{Border, BorderStyle, Borders, LineSpacing, TextColor};
    let text = wordcraft_doc::Rgb(0x1F, 0x23, 0x28);
    let muted = wordcraft_doc::Rgb(0x59, 0x63, 0x6E);
    let rule = Border { style: BorderStyle::Single, width: 0.75, color: Some(wordcraft_doc::Rgb(0xD1, 0xD9, 0xE0)), space: 3.0 };
    let bar = Border { style: BorderStyle::Single, width: 3.0, color: Some(wordcraft_doc::Rgb(0xD1, 0xD9, 0xE0)), space: 8.0 };
    let (font, line) = match theme {
        Theme::Gfm => (wordcraft_doc::styles::sans_font(), 1.5),
        Theme::VitePress => (wordcraft_doc::styles::sans_font(), 1.7),
    };
    let st = &mut doc.styles;
    st.default_chr.font = Some(font.into());
    st.default_chr.size = Some(12.0);
    st.default_chr.color = Some(TextColor::Rgb(text));
    st.default_para.space_before = Some(0.0);
    st.default_para.space_after = Some(12.0);
    st.default_para.line_spacing = Some(LineSpacing::Multiple(line));
    let sizes = [24.0, 18.0, 15.0, 12.0, 10.5, 10.0];
    for (i, size) in sizes.iter().enumerate() {
        if let Some(h) = st.get_mut(&format!("Heading{}", i + 1)) {
            h.chr.font = Some(font.into());
            h.chr.size = Some(*size);
            h.chr.bold = Some(true);
            h.chr.italic = Some(false);
            h.chr.color = Some(TextColor::Rgb(if i == 5 { muted } else { text }));
            h.para.space_before = Some(18.0);
            h.para.space_after = Some(12.0);
            h.para.line_spacing = Some(LineSpacing::Multiple(1.25));
            h.para.borders = match (theme, i) {
                (Theme::Gfm, 0 | 1) => Some(Borders { bottom: Some(rule), ..Default::default() }),
                (Theme::VitePress, 1) => Some(Borders { top: Some(Border { space: 18.0, ..rule }), ..Default::default() }),
                _ => None,
            };
        }
    }
    if let Some(t) = st.get_mut("Title") {
        t.chr.font = Some(font.into());
        t.chr.size = Some(28.0);
        t.chr.bold = Some(true);
        t.chr.color = Some(TextColor::Rgb(text));
    }
    for id in ["Quote", "IntenseQuote"] {
        if let Some(q) = st.get_mut(id) {
            q.para.align = Some(Align::Left);
            q.para.indent_left = Some(12.0);
            q.para.indent_right = Some(0.0);
            q.para.borders = Some(Borders { left: Some(bar), ..Default::default() });
            q.chr.italic = Some(false);
            q.chr.color = Some(TextColor::Rgb(muted));
        }
    }
    if let Some(c) = st.get_mut(model::CODE_STYLE) {
        c.para.shading = Some(wordcraft_doc::Rgb(0xF6, 0xF8, 0xFA));
        c.para.indent_left = Some(8.0);
        c.para.indent_right = Some(8.0);
        c.chr.size = Some(10.5);
    }
    if let Some(h) = st.get_mut("Hyperlink") {
        h.chr.color = Some(TextColor::Rgb(wordcraft_doc::Rgb(0x09, 0x69, 0xDA)));
        h.chr.underline = Some(wordcraft_doc::props::Underline::None);
    }
    theme_blocks(doc);
}

/// The look of the blocks themselves (none of it is written back to Markdown): links in the
/// link style, tables with light rules and a shaded header, room after tables and code blocks.
fn theme_blocks(doc: &mut Document) {
    use wordcraft_doc::Block;
    use wordcraft_doc::props::{Border, BorderStyle, Borders};
    let grid = Border { style: BorderStyle::Single, width: 0.75, color: Some(wordcraft_doc::Rgb(0xD1, 0xD9, 0xE0)), space: 0.0 };
    let code = |b: &Block| matches!(b, Block::Para(p) if p.props.style.as_deref() == Some(model::CODE_STYLE));
    let kinds: Vec<(bool, bool)> = doc.body.iter().map(|b| (matches!(&**b, Block::Table(_)), code(b))).collect();
    for (i, b) in doc.body.iter_mut().enumerate() {
        let (prev_table, prev_code) = i.checked_sub(1).and_then(|k| kinds.get(k)).copied().unwrap_or((false, false));
        let next_code = kinds.get(i + 1).is_some_and(|k| k.1);
        match Arc::make_mut(b) {
            Block::Para(p) => {
                let mut changed = false;
                for r in &mut p.runs {
                    if r.props.link.is_some() && r.props.style.is_none() {
                        r.props.style = Some("Hyperlink".into());
                        changed = true;
                    }
                }
                let is_code = p.props.style.as_deref() == Some(model::CODE_STYLE);
                if prev_table || (is_code && !prev_code) {
                    p.props.space_before = Some(if is_code { 6.0 } else { 12.0 });
                    changed = true;
                }
                if is_code && !next_code {
                    p.props.space_after = Some(12.0);
                    changed = true;
                }
                if changed {
                    p.touch();
                }
            }
            Block::Table(t) => {
                t.props.borders = Some(Borders {
                    top: Some(grid),
                    left: Some(grid),
                    bottom: Some(grid),
                    right: Some(grid),
                    between: Some(grid),
                    inside_v: Some(grid),
                });
                if let Some(first) = t.rows.first_mut() {
                    for c in &mut first.cells {
                        c.props.shading = Some(wordcraft_doc::Rgb(0xF6, 0xF8, 0xFA));
                    }
                }
            }
        }
    }
}

/// A zip of a native copy: the Markdown as `md_name` and each asset at its path.
pub fn bundle_zip(md_name: &str, bundle: &Bundle) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let err = |e: zip::result::ZipError| format!("zip: {e}");
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file(md_name, opts).map_err(err)?;
    z.write_all(bundle.text.as_bytes()).map_err(|e| e.to_string())?;
    // Pictures are compressed already.
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (path, bytes) in &bundle.assets {
        z.start_file(path.as_str(), stored).map_err(err)?;
        z.write_all(bytes).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(err)?.into_inner())
}

/// YAML front matter for the document properties that are set (none: nothing written).
fn front_matter_md(m: &model::Meta, out: &mut Vec<String>) {
    let fields = [
        ("title", &m.title),
        ("author", &m.author),
        ("subject", &m.subject),
        ("keywords", &m.keywords),
        ("description", &m.description),
        ("date", &m.created),
    ];
    if fields.iter().all(|(_, v)| v.trim().is_empty()) {
        return;
    }
    out.push("---".into());
    for (k, v) in fields {
        let v = v.trim();
        if !v.is_empty() {
            let q = v.replace('\\', "\\\\").replace('"', "\\\"").replace(['\n', '\r'], " ");
            out.push(format!("{k}: \"{q}\""));
        }
    }
    out.push("---".into());
    out.push(String::new());
}

/// The thread a comment belongs to (its top comment).
fn comment_root(doc: &Document, id: u32) -> u32 {
    let mut cur = id;
    for _ in 0..64 {
        match doc.comments.get(&cur).and_then(|c| c.parent) {
            Some(p) if p != cur && doc.comments.contains_key(&p) => cur = p,
            _ => break,
        }
    }
    cur
}

/// Thread numbers: commented ranges in document order, then threads with no range.
fn comment_keys(doc: &Document, blocks: &[FBlock]) -> BTreeMap<u32, u32> {
    fn visit(doc: &Document, blocks: &[FBlock], keys: &mut BTreeMap<u32, u32>, depth: usize) {
        if depth > model::MAX_DEPTH + 2 {
            return;
        }
        for b in blocks {
            match b {
                FBlock::Para(p) => {
                    for i in &p.inlines {
                        match i {
                            Inline::Comment { id, end: false } if doc.comments.contains_key(id) && comment_root(doc, *id) == *id => {
                                let n = keys.len() as u32 + 1;
                                keys.entry(*id).or_insert(n);
                            }
                            Inline::Note { blocks, .. } => visit(doc, blocks, keys, depth + 1),
                            _ => {}
                        }
                    }
                }
                FBlock::Table(t) => {
                    for c in t.rows.iter().flatten() {
                        visit(doc, &c.blocks, keys, depth + 1);
                    }
                }
            }
        }
    }
    let mut keys = BTreeMap::new();
    visit(doc, blocks, &mut keys, 0);
    for (id, c) in &doc.comments {
        if c.parent.is_none() && !keys.contains_key(id) && keys.len() < 5_000 {
            let n = keys.len() as u32 + 1;
            keys.insert(*id, n);
        }
    }
    keys
}

/// The comment threads as one HTML comment: invisible on every renderer, kept by every Markdown
/// tool, readable in a text editor. See [`take_comment_block`] for the format.
fn comments_block(doc: &Document, keys: &BTreeMap<u32, u32>) -> String {
    // `--` can't appear inside an HTML comment, and `|` separates the header fields.
    let clean = |s: &str| s.replace("--", "‑‑").replace('|', "/").replace(['\n', '\r'], " ");
    let mut threads: Vec<(u32, u32)> = keys.iter().map(|(id, n)| (*n, *id)).collect();
    threads.sort();
    let entry = |s: &mut String, key: String, c: &wordcraft_doc::Comment| {
        let author = if c.author.trim().is_empty() { "Author".to_string() } else { clean(c.author.trim()) };
        s.push_str(&format!("{key} | {author} | {}", clean(c.date.trim())));
        if c.resolved {
            s.push_str(" | resolved");
        }
        s.push('\n');
        for line in doc.plain_text(wordcraft_doc::StoryRef::Part(c.part)).lines() {
            let line = clean(line).trim_end().to_string();
            s.push_str(&if line.is_empty() { String::new() } else { format!("    {line}") });
            s.push('\n');
        }
    };
    let mut s = format!("{COMMENTS_OPEN}\n");
    for (n, id) in threads {
        let Some(c) = doc.comments.get(&id) else { continue };
        entry(&mut s, format!("c{n}"), c);
        let replies = doc.comments.iter().filter(|(rid, _)| **rid != id && comment_root(doc, **rid) == id).take(200);
        for (k, (_, r)) in replies.enumerate() {
            entry(&mut s, format!("c{n}.{}", k + 1), r);
        }
    }
    s.push_str("-->\n");
    s
}

/// A paragraph line must not start with a comment marker: CommonMark would read the line as an
/// HTML block. Leading markers move after the first word (or to the end of a line whose first
/// word is code).
fn shift_leading_markers(line: &str) -> String {
    let mut markers = String::new();
    let mut rest = line;
    while let Some(r) = rest.strip_prefix("<!-- ") {
        let Some(k) = r.find(" -->") else { break };
        if r.get(..k).and_then(comment_marker).is_none() {
            break;
        }
        markers.push_str(rest.get(..5 + k + 4).unwrap_or(""));
        rest = r.get(k + 4..).unwrap_or("");
    }
    if markers.is_empty() || rest.is_empty() {
        return line.to_string();
    }
    let cut = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let word = rest.get(..cut).unwrap_or("");
    if word.contains('`') {
        return format!("{rest}{markers}");
    }
    format!("{word}{markers}{}", rest.get(cut..).unwrap_or(""))
}

/// Lines (1-based) of `md` that hold HTML other than comments: what a native copy must never
/// contain. Code blocks and code spans are skipped, and so are autolinks (`<https://…>`).
pub fn html_lines(md: &str) -> Vec<usize> {
    let mut bad = Vec::new();
    let mut open_fence: Option<(char, usize)> = None;
    let mut in_comment = false;
    for (n, line) in md.lines().enumerate() {
        let t = line.trim_start();
        if let Some((c, len)) = open_fence {
            if fence(t).is_some_and(|(c2, l2)| c2 == c && l2 >= len) {
                open_fence = None;
            }
            continue;
        }
        if !in_comment && let Some(f) = fence(t) {
            open_fence = Some(f);
            continue;
        }
        let mut rest = line;
        let mut found = false;
        loop {
            if in_comment {
                match rest.find("-->") {
                    Some(k) => {
                        in_comment = false;
                        rest = rest.get(k + 3..).unwrap_or("");
                    }
                    None => break,
                }
            }
            let Some(k) = rest.find('<') else { break };
            let before = rest.get(..k).unwrap_or("");
            let after = rest.get(k..).unwrap_or("");
            // Escaped `\<`, or inside a code span (an odd number of backticks before it).
            if before.ends_with('\\') || before.matches('`').count() % 2 == 1 {
                rest = after.get(1..).unwrap_or("");
                continue;
            }
            if let Some(r) = after.strip_prefix("<!--") {
                in_comment = true;
                rest = r;
                continue;
            }
            let inner = after.get(1..).and_then(|a| a.split_once('>')).map(|(i, _)| i).unwrap_or("");
            let autolink = !inner.is_empty() && !inner.contains([' ', '<']) && (inner.contains(':') || inner.contains('@'));
            // A link destination in angle brackets: `](<path with spaces>)`.
            let destination = before.ends_with("](");
            let tagish = after.get(1..).and_then(|a| a.chars().next()).is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!');
            if tagish && !autolink && !destination {
                found = true;
                break;
            }
            rest = after.get(1..).unwrap_or("");
        }
        if found {
            bad.push(n + 1);
        }
    }
    bad
}

fn write_blocks(blocks: &[FBlock], out: &mut Vec<String>, ctx: &mut Ctx) {
    let mut i = 0;
    let mut counters = [0u32; 9];
    let mut prev_list = false;
    let mut prev_top_ordered = false;
    while let Some(b) = blocks.get(i) {
        if let FBlock::Para(p) = b {
            if p.page_break {
                ctx.note(Adaptation::PageBreak);
            }
            if p.list.is_none() && matches!(p.align, Some(Align::Center | Align::Right)) && !p.is_empty() {
                ctx.note(Adaptation::Alignment);
            }
        }
        match b {
            // A table with no rules, or a single column, lays things out rather than holding
            // data: its cells' contents follow one another as paragraphs.
            FBlock::Table(t) if ctx.native && (t.borderless || t.cols() == 1) => {
                ctx.note(Adaptation::LayoutTable);
                let cells: Vec<FBlock> = t.rows.iter().flatten().filter(|c| !c.covered).flat_map(|c| c.blocks.iter().cloned()).collect();
                write_blocks(&cells, out, ctx);
                prev_list = false;
            }
            FBlock::Table(t) => {
                table_md(t, out, ctx);
                out.push(String::new());
                prev_list = false;
            }
            FBlock::Para(p) if p.kind == Kind::Code && p.list.is_none() => {
                let mut lines = Vec::new();
                while let Some(FBlock::Para(q)) = blocks.get(i) {
                    if q.kind != Kind::Code || q.list.is_some() {
                        break;
                    }
                    lines.push(q.text());
                    i += 1;
                }
                let longest = lines.iter().map(|l| l.chars().filter(|c| *c == '`').count()).max().unwrap_or(0);
                let fence = "`".repeat(3.max(longest + 1).min(64));
                out.push(fence.clone());
                out.extend(lines);
                out.push(fence);
                out.push(String::new());
                prev_list = false;
                continue;
            }
            FBlock::Para(p) => {
                let body = inlines_md_impl(&p.inlines, false, ctx);
                match p.list {
                    Some(li) => {
                        let lv = li.level.min(8) as usize;
                        if !prev_list || (lv == 0 && prev_top_ordered != li.ordered) {
                            counters = [0; 9];
                        }
                        if lv == 0 {
                            prev_top_ordered = li.ordered;
                        }
                        let n = counters.get_mut(lv).map(|c| {
                            *c += 1;
                            *c
                        });
                        for d in counters.iter_mut().skip(lv + 1) {
                            *d = 0;
                        }
                        let marker = if li.ordered { format!("{}.", n.unwrap_or(1)) } else { "-".to_string() };
                        let pad = " ".repeat(4 * lv);
                        let cont = " ".repeat(4 * lv + marker.len() + 1);
                        let body = body.replace("\\\n", &format!("\\\n{cont}"));
                        let body = if p.kind == Kind::Code {
                            code_span(&p.text())
                        } else {
                            let body = escape_line_start(body);
                            // The bullet marker can turn a short run of hyphens into a rule.
                            if thematic_break(&format!("{marker} {body}")) { format!("\\{body}") } else { body }
                        };
                        out.push(format!("{pad}{marker} {body}").trim_end().to_string());
                        prev_list = true;
                        let next_is_list = matches!(blocks.get(i + 1), Some(FBlock::Para(q)) if q.list.is_some());
                        if !next_is_list {
                            out.push(String::new());
                        }
                    }
                    None => {
                        prev_list = false;
                        let line = match p.kind {
                            Kind::Heading(n) => format!("{} {}", "#".repeat(n.clamp(1, 6) as usize), body.replace("\\\n", " ")),
                            Kind::Title => format!("# {}", body.replace("\\\n", " ")),
                            Kind::Rule => "---".to_string(),
                            Kind::Quote => {
                                let b = escape_line_start(body);
                                format!("> {}", b.replace("\\\n", "\\\n> "))
                            }
                            _ => {
                                if body.is_empty() {
                                    i += 1;
                                    continue;
                                }
                                escape_line_start(body).split('\n').map(shift_leading_markers).collect::<Vec<_>>().join("\n")
                            }
                        };
                        out.push(line);
                        out.push(String::new());
                    }
                }
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inl(s: &str) -> Vec<Inline> {
        parse_inlines(s, &Fmt::default(), 0)
    }

    fn spans(s: &str) -> Vec<(String, bool, bool)> {
        inl(s).into_iter().filter_map(|i| if let Inline::Text(t, f) = i { Some((t, f.bold, f.italic)) } else { None }).collect()
    }

    #[test]
    fn emphasis() {
        assert_eq!(spans("a *b* c"), vec![("a ".into(), false, false), ("b".into(), false, true), (" c".into(), false, false)]);
        assert_eq!(spans("**b**"), vec![("b".into(), true, false)]);
        assert_eq!(spans("***x***"), vec![("x".into(), true, true)]);
        assert_eq!(spans("*a **b** c*"), vec![("a ".into(), false, true), ("b".into(), true, true), (" c".into(), false, true)]);
        assert_eq!(spans("snake_case_name"), vec![("snake_case_name".into(), false, false)]);
        assert_eq!(spans("2 * 3 * 4"), vec![("2 * 3 * 4".into(), false, false)]);
        assert_eq!(spans("**unclosed"), vec![("**unclosed".into(), false, false)]);
        let s = inl("~~gone~~");
        assert!(matches!(&s[0], Inline::Text(t, f) if t == "gone" && f.strike));
    }

    #[test]
    fn links_code_escapes() {
        let s = inl("see [the *site*](http://x.org \"T\") and `a*b*` \\*lit\\* &amp; <https://y.com>");
        let link: Vec<_> = s
            .iter()
            .filter_map(|i| if let Inline::Text(t, f) = i { f.link.as_ref().map(|l| (t.clone(), l.clone(), f.italic)) } else { None })
            .collect();
        assert_eq!(link[0], ("the ".into(), "http://x.org".into(), false));
        assert_eq!(link[1], ("site".into(), "http://x.org".into(), true));
        assert!(link.iter().any(|l| l.1 == "https://y.com"));
        assert!(s.iter().any(|i| matches!(i, Inline::Text(t, f) if t == "a*b*" && f.code)));
        let text: String = s.iter().filter_map(|i| if let Inline::Text(t, _) = i { Some(t.as_str()) } else { None }).collect();
        assert!(text.contains("*lit* & "), "{text}");
    }

    #[test]
    fn blocks() {
        let f = parse(
            "# Title\n\nPara one\ncontinues.\n\n> quoted\n\n- a\n- b\n    1. nested\n\n```\ncode *x*\n```\n\n---\n\n| A | B |\n|---|:-:|\n| 1 | 2 |\n",
        );
        let kinds: Vec<String> = f
            .blocks
            .iter()
            .map(|b| match b {
                FBlock::Para(p) => format!("{:?}{:?}:{}", p.kind, p.list.map(|l| (l.ordered, l.level)), p.text()),
                FBlock::Table(t) => format!("table{}", t.rows.len()),
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "Heading(1)None:Title",
                "NormalNone:Para one continues.",
                "QuoteNone:quoted",
                "NormalSome((false, 0)):a",
                "NormalSome((false, 0)):b",
                "NormalSome((true, 1)):nested",
                "CodeNone:code *x*",
                "RuleNone:",
                "table2",
            ]
        );
    }

    #[test]
    fn export_escapes_round_trip() {
        let mut p = Para::default();
        p.push_text("1. not a list *really*", &Fmt::default());
        let md = escape_line_start(inlines_md(&p.inlines));
        let back = parse(&md);
        let FBlock::Para(q) = &back.blocks[0] else { panic!() };
        assert_eq!(q.text(), "1. not a list *really*");
        assert!(q.list.is_none());
    }

    #[test]
    fn single_backtick_code_span_escaped_pipe() {
        let md = "| Command | Status |\n| --- | --- |\n| `cmd \\| grep` | pass |\n";
        let flow = parse(md);
        let FBlock::Table(t) = &flow.blocks[0] else { panic!("expected table") };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].len(), 2);
        let FBlock::Para(p0) = &t.rows[1][0].blocks[0] else { panic!() };
        assert_eq!(p0.inlines.len(), 1);
        assert!(matches!(&p0.inlines[0], Inline::Text(t, f) if t == "cmd | grep" && f.code));
        let FBlock::Para(p1) = &t.rows[1][1].blocks[0] else { panic!() };
        assert_eq!(p1.text(), "pass");

        let mut out = Vec::new();
        table_md(t, &mut out, &mut Ctx::default());
        let exported = out.join("\n");
        assert!(exported.contains("`cmd \\| grep`"), "Exported: {exported}");
    }

    #[test]
    fn double_backtick_code_span_escaped_pipe() {
        let md = "| Syntax | Note |\n| --- | --- |\n| ``a \\| b`` | second |\n";
        let flow = parse(md);
        let FBlock::Table(t) = &flow.blocks[0] else { panic!("expected table") };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].len(), 2);
        let FBlock::Para(p0) = &t.rows[1][0].blocks[0] else { panic!() };
        assert_eq!(p0.inlines.len(), 1);
        assert!(matches!(&p0.inlines[0], Inline::Text(t, f) if t == "a | b" && f.code));
        let FBlock::Para(p1) = &t.rows[1][1].blocks[0] else { panic!() };
        assert_eq!(p1.text(), "second");
    }

    #[test]
    fn code_span_literal_backtick_and_escaped_pipe() {
        let md = "| Expression | Detail |\n| --- | --- |\n| ``a`b\\|c`` | distinct |\n";
        let flow = parse(md);
        let FBlock::Table(t) = &flow.blocks[0] else { panic!("expected table") };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].len(), 2);
        let FBlock::Para(p0) = &t.rows[1][0].blocks[0] else { panic!() };
        assert_eq!(p0.inlines.len(), 1);
        assert!(matches!(&p0.inlines[0], Inline::Text(t, f) if t == "a`b|c" && f.code));
        let FBlock::Para(p1) = &t.rows[1][1].blocks[0] else { panic!() };
        assert_eq!(p1.text(), "distinct");
    }

    #[test]
    fn three_column_table_with_distinctive_text() {
        let md = "| Col1 | Col2 | Col3 |\n| --- | --- | --- |\n| `p1\\|q1` | Distinctive Two | Distinctive Three |\n";
        let flow = parse(md);
        let FBlock::Table(t) = &flow.blocks[0] else { panic!("expected table") };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].len(), 3);
        assert_eq!(t.rows[1][0].text(), "p1|q1");
        assert_eq!(t.rows[1][1].text(), "Distinctive Two");
        assert_eq!(t.rows[1][2].text(), "Distinctive Three");
        let FBlock::Para(p0) = &t.rows[1][0].blocks[0] else { panic!() };
        assert!(matches!(&p0.inlines[0], Inline::Text(_, f) if f.code));
        let FBlock::Para(p1) = &t.rows[1][1].blocks[0] else { panic!() };
        assert!(matches!(&p1.inlines[0], Inline::Text(_, f) if !f.code));
        let FBlock::Para(p2) = &t.rows[1][2].blocks[0] else { panic!() };
        assert!(matches!(&p2.inlines[0], Inline::Text(_, f) if !f.code));
    }

    #[test]
    fn regular_table_cells_containing_escaped_pipes() {
        let md = "| Option | Description |\n| --- | --- |\n| Regular \\| Pipe | Normal text cell |\n";
        let flow = parse(md);
        let FBlock::Table(t) = &flow.blocks[0] else { panic!("expected table") };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].len(), 2);
        assert_eq!(t.rows[1][0].text(), "Regular | Pipe");
        assert_eq!(t.rows[1][1].text(), "Normal text cell");
        let FBlock::Para(p0) = &t.rows[1][0].blocks[0] else { panic!() };
        assert!(matches!(&p0.inlines[0], Inline::Text(_, f) if !f.code));

        let mut out = Vec::new();
        table_md(t, &mut out, &mut Ctx::default());
        let exported = out.join("\n");
        assert!(exported.contains("Regular \\| Pipe"), "Exported: {exported}");
        assert!(!exported.contains("Regular \\\\| Pipe"), "Exported: {exported}");
    }

    #[test]
    fn unescaped_pipes_behave_as_gfm_column_separators() {
        let md = "| Header 1 | Header 2 |\n| --- | --- |\n| `raw|pipe` | Extra Column |\n";
        let flow = parse(md);
        let FBlock::Table(t) = &flow.blocks[0] else { panic!("expected table") };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].len(), 2);
        assert_eq!(t.rows[1][0].text(), "`raw");
        assert_eq!(t.rows[1][1].text(), "pipe`");
    }

    #[test]
    fn ordinary_tables_and_standalone_code_spans_unchanged() {
        let md = "| Alpha | Beta |\n| --- | --- |\n| 1 | 2 |\n";
        let flow = parse(md);
        let FBlock::Table(t) = &flow.blocks[0] else { panic!() };
        let mut out = Vec::new();
        table_md(t, &mut out, &mut Ctx::default());
        assert_eq!(out.join("\n"), "| Alpha | Beta |\n| --- | --- |\n| 1 | 2 |");

        let mut p = Para::default();
        let f = Fmt { code: true, ..Default::default() };
        p.push_text("cmd | grep", &f);
        let exported = inlines_md(&p.inlines);
        assert_eq!(exported, "`cmd | grep`");
    }

    #[test]
    fn table_code_pipe_export_import_round_trip() {
        let mut table = FTable::default();
        let mut cell_a = Cell::default();
        let mut para_a = Para::default();
        let fmt_a = Fmt { code: true, ..Default::default() };
        para_a.push_text("a`b|c", &fmt_a);
        cell_a.blocks.push(FBlock::Para(para_a));

        let mut cell_b = Cell::default();
        let mut para_b = Para::default();
        para_b.push_text("CRITICAL_DISTINCTIVE_INFO", &Fmt::default());
        cell_b.blocks.push(FBlock::Para(para_b));

        let mut head_a = Cell::default();
        let mut head_para_a = Para::default();
        head_para_a.push_text("Col A", &Fmt::default());
        head_a.blocks.push(FBlock::Para(head_para_a));

        let mut head_b = Cell::default();
        let mut head_para_b = Para::default();
        head_para_b.push_text("Col B", &Fmt::default());
        head_b.blocks.push(FBlock::Para(head_para_b));

        table.rows.push(vec![head_a, head_b]);
        table.rows.push(vec![cell_a, cell_b]);

        let mut out = Vec::new();
        table_md(&table, &mut out, &mut Ctx::default());
        let exported_md = out.join("\n");

        assert!(exported_md.contains("``a`b\\|c``"), "Exported: {exported_md}");
        assert!(exported_md.contains("CRITICAL\\_DISTINCTIVE\\_INFO"), "Exported: {exported_md}");

        let back_flow = parse(&exported_md);
        let FBlock::Table(back_table) = &back_flow.blocks[0] else { panic!("expected table") };

        assert_eq!(back_table.rows.len(), 2, "Both rows must exist");
        let row1 = &back_table.rows[1];
        assert_eq!(row1.len(), 2, "Both cells must exist in row 1");

        let back_cell_a = &row1[0];
        let back_cell_b = &row1[1];
        assert_eq!(back_cell_a.text(), "a`b|c", "Cell A text must be preserved");
        assert_eq!(back_cell_b.text(), "CRITICAL_DISTINCTIVE_INFO", "Cell B text must be preserved");

        let FBlock::Para(back_p_a) = &back_cell_a.blocks[0] else { panic!() };
        assert_eq!(back_p_a.inlines.len(), 1);
        assert!(
            matches!(&back_p_a.inlines[0], Inline::Text(t, f) if t == "a`b|c" && f.code),
            "Code formatting must remain intact on Cell A: {:?}",
            back_p_a.inlines
        );

        let FBlock::Para(back_p_b) = &back_cell_b.blocks[0] else { panic!() };
        assert_eq!(back_p_b.inlines.len(), 1);
        assert!(
            matches!(&back_p_b.inlines[0], Inline::Text(t, f) if t == "CRITICAL_DISTINCTIVE_INFO" && !f.code),
            "Cell B formatting must remain plain text: {:?}",
            back_p_b.inlines
        );

        assert_eq!(back_cell_a.text(), "a`b|c");
        assert_eq!(back_cell_b.text(), "CRITICAL_DISTINCTIVE_INFO");

        assert!(!back_cell_a.text().contains("CRITICAL_DISTINCTIVE_INFO"));
        assert!(!back_cell_b.text().contains("a`b|c"));

        let doc = import(&exported_md);
        let re_exported = export(&doc);
        assert!(re_exported.contains("``a`b\\|c``"), "Re-exported: {re_exported}");
        assert!(re_exported.contains("CRITICAL\\_DISTINCTIVE\\_INFO"), "Re-exported: {re_exported}");
        let doc2 = import(&re_exported);
        let back_flow2 = model::from_doc(&doc2);
        let FBlock::Table(back_table2) = &back_flow2.blocks[0] else { panic!() };
        assert_eq!(back_table2.rows[1][0].text(), "a`b|c");
        assert_eq!(back_table2.rows[1][1].text(), "CRITICAL_DISTINCTIVE_INFO");
    }
}
