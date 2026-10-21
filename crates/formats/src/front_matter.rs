//! Plume: a Markdown document's YAML front matter, kept as written. The document's properties
//! (title, author…) are read from it, and a save rewrites only the keys whose value changed, in
//! place and in the form they were written in (a list stays a list, `tags` stays `tags`).
//! Everything else (comments, PDF template settings, VitePress keys, the author's own keys) is
//! left as it is.
//!
//! A document converted from Word gets its Word properties here once ([`adopt_office`]): the
//! creation date, category, revision, company and the custom properties, each under its own key
//! at the top level, where pandoc, PDF templates and GitHub's table read them.

use wordcraft_doc::Document;

use crate::model::FRONT_MATTER_PART;

/// One top-level entry: its key (empty for comments and blank lines between keys) and its lines.
struct Entry {
    key: String,
    lines: Vec<String>,
}

/// A line that continues the entry above it (an indented line or a list item).
fn nested(l: &str) -> bool {
    l.starts_with([' ', '\t', '-'])
}

/// The key a top-level line names (`title: …` → `title`).
fn line_key(l: &str) -> Option<&str> {
    if nested(l) || l.starts_with('#') {
        return None;
    }
    let (k, _) = l.split_once(':')?;
    let k = k.trim();
    (!k.is_empty() && !k.contains(' ')).then_some(k)
}

fn entries(body: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for l in body.lines() {
        match line_key(l) {
            Some(k) => out.push(Entry { key: k.to_string(), lines: vec![l.to_string()] }),
            None if nested(l) && out.last().is_some_and(|e| !e.key.is_empty()) => {
                if let Some(e) = out.last_mut() {
                    e.lines.push(l.to_string());
                }
            }
            None => out.push(Entry { key: String::new(), lines: vec![l.to_string()] }),
        }
    }
    out
}

fn join(entries: &[Entry]) -> String {
    entries.iter().flat_map(|e| e.lines.iter().map(String::as_str)).collect::<Vec<_>>().join("\n")
}

fn find(entries: &[Entry], key: &str) -> Option<usize> {
    entries.iter().position(|e| !e.key.is_empty() && e.key.eq_ignore_ascii_case(key))
}

/// A YAML scalar as Plume writes it (double-quoted), single-quoted, plain, or a flow list `[a, b]`
/// (joined with `, `).
pub fn scalar(v: &str) -> String {
    let v = v.trim();
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
        return inner.split(',').map(scalar).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
    }
    v.to_string()
}

fn entry_value(e: &Entry) -> String {
    let first = e.lines.first().and_then(|l| l.split_once(':')).map(|(_, v)| v.trim()).unwrap_or("");
    if !first.is_empty() {
        return scalar(first);
    }
    // A block list (`key:` then `  - item` lines): its items, joined.
    e.lines.iter().skip(1).filter_map(|l| l.trim().strip_prefix('-')).map(scalar).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ")
}

/// The value of top-level `key` (a list's items joined with `, `), if the front matter has it.
pub fn get(body: &str, key: &str) -> Option<String> {
    let es = entries(body);
    find(&es, key).and_then(|i| es.get(i)).map(entry_value)
}

/// Whether top-level `key` holds a nested map (indented `name: value` lines) rather than a value
/// or a list.
pub fn is_map(body: &str, key: &str) -> bool {
    let es = entries(body);
    find(&es, key).and_then(|i| es.get(i)).is_some_and(|e| e.lines.iter().skip(1).any(|l| !l.trim().is_empty() && !l.trim().starts_with('-')))
}

/// The top-level keys and their values, in order (nested maps give their key and an empty value).
pub fn fields(body: &str) -> Vec<(String, String)> {
    entries(body).iter().filter(|e| !e.key.is_empty()).map(|e| (e.key.clone(), entry_value(e))).collect()
}

/// `v` as a double-quoted YAML string.
fn quoted(v: &str) -> String {
    format!("\"{}\"", v.trim().replace('\\', "\\\\").replace('"', "\\\"").replace(['\n', '\r'], " "))
}

fn entry_lines(key: &str, value: &str, list: bool) -> Vec<String> {
    if list {
        let mut lines = vec![format!("{key}:")];
        lines.extend(value.split(", ").map(str::trim).filter(|x| !x.is_empty()).map(|x| format!("  - {}", quoted(x))));
        lines
    } else {
        vec![format!("{key}: {}", quoted(value))]
    }
}

/// `body` with top-level `key` set to `value` (empty removes it). An existing key keeps its
/// place, its spelling and its form (a block list stays one); a new key goes after the last of
/// the keys `after` names the front matter has, else at the top; with no `after`, at the end.
pub fn set_after(body: &str, key: &str, value: &str, after: Option<&[&str]>) -> String {
    let mut es = entries(body);
    let value = value.trim();
    match find(&es, key) {
        Some(i) if value.is_empty() => {
            es.remove(i);
        }
        Some(i) => {
            if let Some(e) = es.get_mut(i) {
                let list = e.lines.len() > 1 && e.lines.iter().skip(1).all(|l| l.trim().starts_with('-') || l.trim().is_empty());
                let list = list || e.lines.first().is_some_and(|l| l.split_once(':').is_some_and(|(_, v)| v.trim().starts_with('[')));
                e.lines = entry_lines(&e.key.clone(), value, list);
            }
        }
        None if value.is_empty() => {}
        None => {
            let at = match after {
                None => es.len(),
                Some(keys) => keys.iter().filter_map(|k| find(&es, k)).max().map_or(0, |i| i + 1),
            };
            es.insert(at.min(es.len()), Entry { key: key.to_string(), lines: entry_lines(key, value, false) });
        }
    }
    join(&es)
}

/// Whether `key` can be a top-level key Plume writes: 1 to 64 letters, digits, `-` and `_`.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.chars().count() <= 64 && key.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
}

/// [`set_after`] with a new key at the end.
pub fn set(body: &str, key: &str, value: &str) -> String {
    set_after(body, key, value, None)
}

/// Indentation of a line (spaces; a tab counts as two), `None` for a blank or comment line.
fn indent_of(l: &str) -> Option<usize> {
    let t = l.trim_start();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    Some(l.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 2 } else { 1 }).sum())
}

/// The key of a `key: value` line at any indentation (not a list item).
fn nested_key(l: &str) -> Option<&str> {
    let t = l.trim_start();
    if t.starts_with('-') || t.starts_with('#') {
        return None;
    }
    let (k, _) = t.split_once(':')?;
    let k = k.trim().trim_matches(['"', '\'']);
    (!k.is_empty() && !k.contains(' ')).then_some(k)
}

/// Where the map entry `key` is among `lines[from..to]` at indentation `ind`: its line and the
/// end of its nested block.
fn find_at(lines: &[String], from: usize, to: usize, ind: usize, key: &str) -> Option<(usize, usize)> {
    let mut i = from;
    while i < to {
        let l = lines.get(i)?;
        if indent_of(l) == Some(ind) && nested_key(l).is_some_and(|k| k == key) {
            let mut end = i + 1;
            while end < to && lines.get(end).is_some_and(|n| indent_of(n).is_none_or(|d| d > ind)) {
                end += 1;
            }
            // Blank or comment lines at the end belong to what follows.
            while end > i + 1 && lines.get(end - 1).is_some_and(|n| indent_of(n).is_none()) {
                end -= 1;
            }
            return Some((i, end));
        }
        i += 1;
    }
    None
}

/// The value at `path` in nested maps (`["plume", "classification", "level"]`), if set.
pub fn get_path(body: &str, path: &[&str]) -> Option<String> {
    let lines: Vec<String> = body.lines().map(str::to_string).collect();
    let (mut from, mut to, mut ind) = (0, lines.len(), 0);
    for (n, key) in path.iter().enumerate() {
        let (at, end) = find_at(&lines, from, to, ind, key)?;
        if n + 1 == path.len() {
            let (_, v) = lines.get(at)?.split_once(':')?;
            let v = scalar(v.trim());
            return (!v.is_empty()).then_some(v);
        }
        ind = lines.get(at + 1..end)?.iter().find_map(|l| indent_of(l))?;
        (from, to) = (at + 1, end);
    }
    None
}

/// A value as Plume writes it in a nested map: plain when YAML reads it back as the same text
/// (`C2`, `personal`), double-quoted otherwise (`"1.0"`, `"yes"`, text with spaces).
fn nested_value(v: &str) -> String {
    let plain = v.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '+' | '.'))
        && v.chars().next().is_some_and(char::is_alphabetic)
        && !matches!(v.to_ascii_lowercase().as_str(), "yes" | "no" | "true" | "false" | "on" | "off" | "null" | "y" | "n");
    if plain { v.to_string() } else { quoted(v) }
}

/// `body` with the value at `path` set (empty removes it, and maps it leaves empty). Every other
/// line — the map's other keys, unknown ones included, comments, order — stays as it was; a new
/// key goes at the end of its map, indented by two spaces.
pub fn set_path(body: &str, path: &[&str], value: &str) -> String {
    let mut lines: Vec<String> = body.lines().map(str::to_string).collect();
    let value = value.trim().replace(['\n', '\r'], " ");
    set_in(&mut lines, 0, None, 0, path, &value);
    lines.join("\n")
}

/// [`set_path`] within `lines[from..to]` (`to` `None`: to the end) at indentation `ind`; whether
/// the map still has entries.
fn set_in(lines: &mut Vec<String>, from: usize, to: Option<usize>, ind: usize, path: &[&str], value: &str) -> bool {
    let Some((key, rest)) = path.split_first() else { return true };
    let to_end = to.unwrap_or(lines.len());
    match find_at(lines, from, to_end, ind, key) {
        Some((at, end)) if rest.is_empty() => {
            if value.is_empty() {
                lines.drain(at..end);
            } else {
                // A value is written on its key's line, replacing what was nested under it.
                lines.drain(at + 1..end);
                if let Some(l) = lines.get_mut(at) {
                    *l = format!("{}{key}: {}", " ".repeat(ind), nested_value(value));
                }
            }
        }
        Some((at, end)) => {
            let child = lines.get(at + 1..end).and_then(|c| c.iter().find_map(|l| indent_of(l))).unwrap_or(ind + 2);
            let before = lines.len();
            let kept = set_in(lines, at + 1, Some(end), child, rest, value);
            if !kept {
                let end = (end + lines.len()).saturating_sub(before);
                lines.drain(at..end.max(at + 1));
            }
        }
        None if value.is_empty() => {}
        None => {
            // A new entry at the end of this map: its key, then its nested keys.
            let mut at = to_end;
            while at > from && lines.get(at - 1).is_some_and(|l| indent_of(l).is_none()) {
                at -= 1;
            }
            let mut new = Vec::new();
            for (d, k) in path.iter().enumerate() {
                let pad = " ".repeat(ind + 2 * d);
                new.push(if d + 1 == path.len() { format!("{pad}{k}: {}", nested_value(value)) } else { format!("{pad}{k}:") });
            }
            for (i, l) in new.into_iter().enumerate() {
                lines.insert(at + i, l);
            }
        }
    }
    let to_end = to.map_or(lines.len(), |t| t.min(lines.len()));
    (from..to_end).any(|i| lines.get(i).and_then(|l| indent_of(l)).is_some())
}

/// The document's front matter as stored ([`FRONT_MATTER_PART`]).
pub fn stored(doc: &Document) -> String {
    doc.passthrough.get(FRONT_MATTER_PART).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default()
}

/// Store `body` as the document's front matter.
pub fn store(doc: &mut Document, body: &str) {
    if body.trim().is_empty() {
        doc.passthrough.remove(FRONT_MATTER_PART);
    } else {
        doc.passthrough.insert(FRONT_MATTER_PART.to_string(), std::sync::Arc::new(body.as_bytes().to_vec()));
    }
}

/// The Word extended properties a DOCX keeps (`docProps/app.xml`): `name\tvalue` lines, as the
/// DOCX reader stores them (`wordcraft_docx::package::APP_KEPT`).
const APP_KEPT: &str = "wordcraft:app.kept";

/// Custom properties that only mean something to Word or a plug-in in it (citation managers'
/// preferences, sensitivity labels, Mark as Final): not written.
fn technical(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with('_') || n.starts_with("zotero_") || n.starts_with("mendeley") || n.starts_with("msip_label") || n.starts_with("contenttypeid")
}

/// Keys Plume or the front matter itself already uses: a Word custom property of the same name
/// is written as `word-<name>`.
const RESERVED: &[&str] = &[
    "title",
    "author",
    "subject",
    "keywords",
    "tags",
    "description",
    "date",
    "lastupdated",
    "editor",
    "version",
    "category",
    "publisher",
    "manager",
    "status",
    "generator",
    "lang",
    "layout",
    "outline",
];

/// A custom property's name as a front matter key: letters, digits, `-` and `_`.
pub fn key_of(name: &str) -> Option<String> {
    let k: String = name.trim().chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_') { c } else { '-' }).collect();
    let k = k.trim_matches('-').to_string();
    if k.is_empty() {
        return None;
    }
    Some(if RESERVED.contains(&k.to_ascii_lowercase().as_str()) { format!("word-{k}") } else { k })
}

/// The date part of an ISO 8601 time (`2026-10-18T20:47:17Z` → `2026-10-18`).
fn day(iso: &str) -> &str {
    iso.trim().get(..10).filter(|d| d.len() == 10 && d.as_bytes().get(4) == Some(&b'-')).unwrap_or(iso.trim())
}

/// Whether `doc` holds properties only an office suite sets: a category, custom properties or a
/// DOCX's extended properties. (The modification time, last author and revision don't tell:
/// Plume sets them itself when it saves.)
fn from_office(doc: &Document) -> bool {
    !doc.core.category.trim().is_empty() || !doc.custom_props.is_empty() || doc.passthrough.contains_key(APP_KEPT)
}

/// The Word properties of `doc` that the five front matter properties (title, author, subject,
/// keywords, description) don't hold, as front matter keys and values, in order.
pub fn office_properties(doc: &Document) -> Vec<(String, String)> {
    let c = &doc.core;
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |k: &str, v: &str| {
        if !v.trim().is_empty() {
            out.push((k.to_string(), v.trim().to_string()));
        }
    };
    push("date", day(&c.created));
    push("lastUpdated", day(&c.modified));
    push("editor", &c.last_modified_by);
    if c.revision > 0 {
        push("version", &c.revision.to_string());
    }
    push("category", &c.category);
    let app = doc.passthrough.get(APP_KEPT).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    for (name, key) in [("Company", "publisher"), ("Manager", "manager")] {
        if let Some(v) = app.lines().find_map(|l| l.strip_prefix(name).and_then(|r| r.strip_prefix('\t'))) {
            push(key, v);
        }
    }
    for p in doc.custom_props.iter().filter(|p| p.kind != "raw" && !technical(&p.name)) {
        if let Some(k) = key_of(&p.name) {
            push(&k, &p.value);
        }
    }
    out
}

/// Plume: a document converted from Word, with no front matter yet, gets its Word properties in
/// it, once. A document that has front matter, or that shows no sign of an office suite, is left
/// alone ([`adopt_converted`] skips that check, for a document known to come from one).
pub fn adopt_office(doc: &mut Document) {
    if from_office(doc) {
        adopt_converted(doc);
    }
}

/// [`adopt_office`] for a document just converted from an office file: its properties are Word's.
pub fn adopt_converted(doc: &mut Document) {
    if doc.passthrough.contains_key(FRONT_MATTER_PART) {
        return;
    }
    let mut body = String::new();
    for (k, v) in office_properties(doc) {
        body = set(&body, &k, &v);
    }
    store(doc, &body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_values_are_read_and_written_in_place() {
        let body = "title: \"Mon document\"\n# a comment\nplume:\n  classification:\n    reference: plume\n    level: C1\n    owner: x  # unknown key kept\n  context: personal\nother: 1";
        assert_eq!(get_path(body, &["plume", "classification", "level"]).as_deref(), Some("C1"));
        assert_eq!(get_path(body, &["plume", "context"]).as_deref(), Some("personal"));
        assert_eq!(get_path(body, &["plume", "visibility"]), None);
        let b = set_path(body, &["plume", "classification", "level"], "C2");
        assert!(b.contains("    level: C2\n    owner: x  # unknown key kept"), "{b}");
        let b = set_path(&b, &["plume", "visibility"], "private");
        assert!(b.contains("  context: personal\n  visibility: private\nother: 1"), "{b}");
        let b = set_path(&b, &["plume", "classification", "version"], "1.0");
        assert_eq!(get_path(&b, &["plume", "classification", "version"]).as_deref(), Some("1.0"));
        assert!(b.contains("version: \"1.0\""), "a number-looking value is quoted: {b}");
        assert!(b.starts_with("title: \"Mon document\"\n# a comment\n"), "{b}");
        // A new map where there was none; removing its last key removes it.
        let fresh = set_path("title: x", &["plume", "classification", "level"], "C0");
        assert_eq!(fresh, "title: x\nplume:\n  classification:\n    level: C0");
        assert_eq!(set_path(&fresh, &["plume", "classification", "level"], ""), "title: x");
        assert_eq!(set_path("", &["plume", "context"], "other"), "plume:\n  context: other");
    }

    const SRC: &str = "title: Guide\ntags:\n  - a\n  - b\n# PDF settings\ntitlepage: true\nauthor: [Ada, Grace]";

    #[test]
    fn values_are_read_as_written() {
        assert_eq!(get(SRC, "title").as_deref(), Some("Guide"));
        assert_eq!(get(SRC, "tags").as_deref(), Some("a, b"));
        assert_eq!(get(SRC, "author").as_deref(), Some("Ada, Grace"));
        assert_eq!(get(SRC, "missing"), None);
        let keys: Vec<String> = fields(SRC).into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["title", "tags", "titlepage", "author"]);
    }

    #[test]
    fn a_change_rewrites_only_its_key_in_its_form() {
        let out = set(SRC, "tags", "a, b, c");
        assert_eq!(out, "title: Guide\ntags:\n  - \"a\"\n  - \"b\"\n  - \"c\"\n# PDF settings\ntitlepage: true\nauthor: [Ada, Grace]");
        let out = set(SRC, "author", "Ada");
        assert!(out.ends_with("author:\n  - \"Ada\""), "{out}");
        let out = set(SRC, "title", "");
        assert!(out.starts_with("tags:"), "{out}");
        let out = set_after(SRC, "subject", "Bilan", Some(&["title"]));
        assert!(out.starts_with("title: Guide\nsubject: \"Bilan\"\ntags:"), "{out}");
        assert!(set_after(SRC, "description", "D", Some(&[])).starts_with("description: \"D\"\ntitle: Guide"));
    }

    #[test]
    fn word_properties_are_adopted_once() {
        let mut d = Document::new();
        d.core.title = "Rapport".into();
        d.core.category = "Finance".into();
        d.core.created = "2026-10-18T20:47:17Z".into();
        d.core.last_modified_by = "Ada".into();
        d.core.revision = 3;
        d.set_custom_prop("toto", "valeur perso");
        d.set_custom_prop("Client Name", "ACME");
        d.set_custom_prop("ZOTERO_PREF_1", "<data/>");
        d.set_custom_prop("title", "clash");
        adopt_office(&mut d);
        let body = stored(&d);
        assert_eq!(
            body,
            "date: \"2026-10-18\"\neditor: \"Ada\"\nversion: \"3\"\ncategory: \"Finance\"\ntoto: \"valeur perso\"\nClient-Name: \"ACME\"\nword-title: \"clash\""
        );
        d.core.category = "Other".into();
        adopt_office(&mut d);
        assert_eq!(stored(&d), body, "once");
        let mut plain = Document::new();
        plain.core.title = "Notes".into();
        // What Plume sets when it saves is no sign of Word.
        plain.core.last_modified_by = "Plume User".into();
        plain.core.revision = 1;
        plain.core.modified = "2026-10-18T00:00:00Z".into();
        adopt_office(&mut plain);
        assert!(stored(&plain).is_empty(), "never a Word document");
    }
}
