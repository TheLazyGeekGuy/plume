//! Plume: Markdown blocks from the ribbon. A code block with its language (` ```js `), a
//! container (VitePress `::: tip`, or a GitHub alert `> [!NOTE]` for the kinds only GitHub has),
//! the language of the code block at the caret, and leaving a block for plain text.

use serde_json::{Value, json};
use wordcraft_doc::props::{MdBlock, MdCode, MdContainer};
use wordcraft_doc::{Block, Path, StoryRef};

use super::para::fmt;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Longest info string (language and options) a command sets.
const MAX_INFO: usize = 200;
/// Longest container title a command sets.
const MAX_TITLE: usize = 200;

/// The container kinds the ribbon offers, and whether each is written as a GitHub alert (the
/// kinds VitePress has as `:::` containers are written that way; VitePress reads GitHub's
/// alerts too, so `note`, `important` and `caution` work on both).
pub const KINDS: [(&str, bool); 8] = [
    ("note", true),
    ("tip", false),
    ("important", true),
    ("warning", false),
    ("caution", true),
    ("info", false),
    ("danger", false),
    ("details", false),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("markdown.codeBlock", "Code Block", "Insert › Markdown", code_block)
            .params(r#"{"language"?: string (the info string: js, python, mermaid, js{2}…)} — the selected paragraphs become one code block"#),
        CommandSpec::new("markdown.callout", "Callout", "Insert › Markdown", callout)
            .params(r#"{"kind": "note|tip|important|warning|caution|info|danger|details", "title"?: string} — the selected paragraphs go in it, under its title line"#),
        CommandSpec::new("markdown.codeLanguage", "Code Language", "Insert › Markdown", code_language)
            .params(r#"{"language": string} — the code block at the caret"#),
        CommandSpec::new("markdown.leaveBlock", "Leave Block", "Insert › Markdown", leave_block),
        CommandSpec::new("markdown.block", "Markdown Block", "Insert › Markdown", |s, _| Ok(block_at_caret(s))).pure(),
    ]
}

/// The Markdown block at the caret: `{code?: {language}, callout?: {kind, title, alert}, source?}`.
fn block_at_caret(s: &Session) -> Value {
    let Some(md) = s.doc.para(s.sel.focus.story, &s.sel.focus.path).and_then(|p| p.props.md.as_deref()) else {
        return json!({});
    };
    let mut out = json!({});
    if let Some(c) = &md.code {
        out["code"] = json!({"language": c.info});
    }
    if let Some(c) = &md.container {
        out["callout"] = json!({"kind": c.kind, "alert": c.alert});
    }
    if md.raw.is_some() {
        out["source"] = json!(true);
    }
    out
}

/// Every paragraph's Markdown block in `blocks` (tables included), for ids.
fn max_id(blocks: &wordcraft_doc::Blocks, depth: usize) -> u32 {
    if depth > 8 {
        return 0;
    }
    blocks
        .iter()
        .map(|b| match &**b {
            Block::Para(p) => p
                .props
                .md
                .as_deref()
                .map_or(0, |m| m.code.as_ref().map_or(0, |c| c.id).max(m.container.as_ref().map_or(0, |c| c.id)).max(m.raw.unwrap_or(0))),
            Block::Table(t) => t.rows.iter().flat_map(|r| r.cells.iter()).map(|c| max_id(&c.blocks, depth + 1)).max().unwrap_or(0),
        })
        .max()
        .unwrap_or(0)
}

/// A block id no paragraph of the document uses yet.
fn new_id(s: &Session) -> u32 {
    max_id(&s.doc.body, 0).saturating_add(1)
}

fn text_param(v: &Value, key: &str, max: usize) -> Result<Option<String>, CmdError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(x)) if x.chars().count() <= max => Ok(Some(x.replace(['\n', '\r', '`'], " ").trim().to_string())),
        Some(_) => Err(CmdError::Params(format!("`{key}` must be a string of at most {max} characters"))),
    }
}

fn ensure_code_style(s: &mut Session) {
    if s.doc.styles.get(wordcraft_formats::model::CODE_STYLE).is_none() {
        s.doc.styles.upsert(wordcraft_formats::model::code_style());
    }
}

fn code_block(s: &mut Session, v: &Value) -> CmdResult {
    let info = text_param(v, "language", MAX_INFO)?.unwrap_or_default();
    ensure_code_style(s);
    let code = MdCode { id: new_id(s), info };
    fmt(s, &|p| {
        p.style = Some(wordcraft_formats::model::CODE_STYLE.into());
        p.numbering = None;
        let md = p.md.get_or_insert_with(Default::default);
        md.code = Some(code.clone());
        md.raw = None;
        md.title = false;
    })
}

fn callout(s: &mut Session, v: &Value) -> CmdResult {
    let kind = p::req_str(v, "kind")?.trim().to_ascii_lowercase();
    let Some((_, alert)) = KINDS.iter().find(|(k, _)| *k == kind) else {
        return Err(CmdError::Params(format!("`kind`: one of {}", KINDS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "))));
    };
    let container =
        MdContainer { id: new_id(s), kind: if *alert { kind.to_ascii_uppercase() } else { kind.clone() }, title: String::new(), alert: *alert };
    let title = match text_param(v, "title", MAX_TITLE)? {
        Some(t) if !t.is_empty() => t,
        _ => wordcraft_formats::markdown::default_title(&container).unwrap_or_default(),
    };
    let paths = super::selected_paths(s);
    let story = s.sel.focus.story;
    // Paragraphs already in a container can't go in another (Markdown has no nesting Plume writes).
    if paths.iter().any(|path| s.doc.para(story, path).and_then(|p| p.props.md.as_deref()).is_some_and(|m| m.container.is_some())) {
        return Err(CmdError::Disabled("markdown.callout: the selection is in a callout already".into()));
    }
    let Some(first) = paths.first().cloned() else { return Err(CmdError::Disabled("markdown.callout: no paragraph selected".into())) };
    for path in &paths {
        let para = s.doc.para_mut(story, path)?;
        para.props.md.get_or_insert_with(Default::default).container = Some(container.clone());
        para.touch();
    }
    // Its title line, above the first paragraph.
    let mut head = wordcraft_doc::Paragraph::new();
    let _ = head.insert_text(0, &title, &wordcraft_doc::CharProps { bold: Some(true), ..Default::default() });
    head.mark.bold = Some(true);
    head.props.md = Some(Box::new(MdBlock { container: Some(container), title: true, ..Default::default() }));
    s.doc.insert_block(story, &first, Block::Para(head))?;
    shift_after_insert(s, story, &first);
    Ok(json!({"kind": kind, "alert": alert, "title": title}))
}

/// The selection after a block was inserted at `at`: positions at or after it in the same
/// container move down one block.
fn shift_after_insert(s: &mut Session, story: StoryRef, at: &Path) {
    for pos in [&mut s.sel.anchor, &mut s.sel.focus] {
        if pos.story == story && pos.path.parent() == at.parent() && pos.path.last() >= at.last() {
            pos.path = pos.path.with_last(pos.path.last() + 1);
        }
    }
}

fn code_language(s: &mut Session, v: &Value) -> CmdResult {
    let info = text_param(v, "language", MAX_INFO)?.unwrap_or_default();
    let story = s.sel.focus.story;
    let at = s.sel.focus.path.clone();
    let Some(id) = s.doc.para(story, &at).and_then(|p| p.props.md.as_deref()).and_then(|m| m.code.as_ref()).map(|c| c.id) else {
        return Err(CmdError::Disabled("markdown.codeLanguage: the caret is not in a code block".into()));
    };
    let n = s.doc.container(story, &at).map_or(0, |b| b.len());
    for k in 0..n {
        let path = at.with_last(k);
        let same = s.doc.para(story, &path).and_then(|p| p.props.md.as_deref()).and_then(|m| m.code.as_ref()).is_some_and(|c| c.id == id);
        if same {
            let para = s.doc.para_mut(story, &path)?;
            if let Some(c) = para.props.md.as_mut().and_then(|m| m.code.as_mut()) {
                c.info.clone_from(&info);
            }
            para.touch();
        }
    }
    Ok(json!({"language": info}))
}

/// The selected paragraphs leave their block: plain paragraphs again (a code line keeps its text).
fn leave_block(s: &mut Session, _: &Value) -> CmdResult {
    fmt(s, &|p| {
        if p.md.as_deref().is_some_and(|m| m.code.is_some() || m.raw.is_some()) {
            p.style = None;
        }
        p.md = None;
    })
}
