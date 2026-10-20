//! Plume: the table diagrams in Mermaid: what each holds ([`SheetSpec`]), how it is written, and
//! how it is read back from the parsed diagram (`mermaid_svg::ast`).

use mermaid_svg::ast;

use crate::form::{FieldSpec, choice, flag, get, ident, is, num, number, one_line, quoted, reference, set, text};
use crate::sheet::{GridSpec, Sheet, SheetSpec, TableSpec, cell, level};

fn opt(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("")
}

/// `title …` when the sheet has a title.
fn title_line(s: &Sheet, out: &mut Vec<String>) {
    let t = one_line(get(&s.fields, "title"));
    if !t.is_empty() {
        out.push(format!("    title {t}"));
    }
}

fn parse_num(s: &str) -> f64 {
    s.trim().replace(',', ".").parse::<f64>().ok().filter(|f| f.is_finite()).unwrap_or(0.0)
}

const TITLE: FieldSpec = text("title", "Title");

// ---- sequence ---------------------------------------------------------------

const PARTICIPANT_KINDS: &[(&str, &str)] = &[
    ("participant", "Participant"),
    ("actor", "Actor"),
    ("boundary", "Boundary"),
    ("control", "Control"),
    ("entity", "Entity"),
    ("database", "Database"),
];

const STEPS: &[(&str, &str)] = &[
    ("message", "Message"),
    ("note", "Note"),
    ("activate", "Activate"),
    ("deactivate", "Deactivate"),
    ("loop", "Loop"),
    ("alt", "Alternative"),
    ("else", "Else"),
    ("opt", "Optional"),
    ("par", "Parallel"),
    ("and", "And"),
    ("critical", "Critical"),
    ("option", "Option"),
    ("break", "Break"),
    ("rect", "Highlight"),
    ("end", "End"),
    ("create", "Create"),
    ("destroy", "Destroy"),
    ("autonumber", "Number the Messages"),
];

/// Arrows: id, label, Mermaid, AST kind.
const ARROWS: &[(&str, &str, &str, ast::ArrowKind)] = &[
    ("call", "Call", "->>", ast::ArrowKind::SolidArrow),
    ("reply", "Reply", "-->>", ast::ArrowKind::DashedArrow),
    ("line", "Line", "->", ast::ArrowKind::Solid),
    ("dashed", "Dashed Line", "-->", ast::ArrowKind::Dashed),
    ("async", "Asynchronous", "-)", ast::ArrowKind::Open),
    ("asyncReply", "Asynchronous Reply", "--)", ast::ArrowKind::DashedOpen),
    ("lost", "Lost (Cross)", "-x", ast::ArrowKind::Cross),
    ("lostReply", "Lost Reply (Cross)", "--x", ast::ArrowKind::DashedCross),
    ("both", "Both Ways", "<<->>", ast::ArrowKind::BiSolidArrow),
    ("bothDashed", "Both Ways, Dashed", "<<-->>", ast::ArrowKind::BiDashedArrow),
    ("halfTop", "Half Arrow (Top)", "-|\\", ast::ArrowKind::HalfArrowTop),
    ("halfBottom", "Half Arrow (Bottom)", "-|/", ast::ArrowKind::HalfArrowBottom),
    ("halfTopDashed", "Half Arrow, Dashed (Top)", "--|\\", ast::ArrowKind::DashedHalfArrowTop),
    ("halfBottomDashed", "Half Arrow, Dashed (Bottom)", "--|/", ast::ArrowKind::DashedHalfArrowBottom),
    ("halfStartTop", "Half Arrow at the Start (Top)", "\\|-", ast::ArrowKind::HalfArrowStartTop),
    ("halfStartBottom", "Half Arrow at the Start (Bottom)", "/|-", ast::ArrowKind::HalfArrowStartBottom),
    ("halfStartTopDashed", "Half Arrow at the Start, Dashed (Top)", "\\|--", ast::ArrowKind::DashedHalfArrowStartTop),
    ("halfStartBottomDashed", "Half Arrow at the Start, Dashed (Bottom)", "/|--", ast::ArrowKind::DashedHalfArrowStartBottom),
];

const ARROW_CHOICES: &[(&str, &str)] = &[
    ("call", "Call"),
    ("reply", "Reply"),
    ("line", "Line"),
    ("dashed", "Dashed Line"),
    ("async", "Asynchronous"),
    ("asyncReply", "Asynchronous Reply"),
    ("lost", "Lost (Cross)"),
    ("lostReply", "Lost Reply (Cross)"),
    ("both", "Both Ways"),
    ("bothDashed", "Both Ways, Dashed"),
    ("halfTop", "Half Arrow (Top)"),
    ("halfBottom", "Half Arrow (Bottom)"),
    ("halfTopDashed", "Half Arrow, Dashed (Top)"),
    ("halfBottomDashed", "Half Arrow, Dashed (Bottom)"),
    ("halfStartTop", "Half Arrow at the Start (Top)"),
    ("halfStartBottom", "Half Arrow at the Start (Bottom)"),
    ("halfStartTopDashed", "Half Arrow at the Start, Dashed (Top)"),
    ("halfStartBottomDashed", "Half Arrow at the Start, Dashed (Bottom)"),
];

const NOTE_PLACES: &[(&str, &str)] = &[("right", "Right of"), ("left", "Left of"), ("over", "Over")];

/// Steps: which cells apply.
fn step_hidden(row: &[String], col: usize) -> bool {
    let step = cell(row, 0);
    match col {
        // From
        1 => matches!(step, "loop" | "alt" | "else" | "opt" | "par" | "and" | "critical" | "option" | "break" | "rect" | "end" | "autonumber"),
        // To
        2 => step != "message" && step != "note",
        // Text
        3 => matches!(step, "activate" | "deactivate" | "end" | "create" | "destroy"),
        // Arrow
        4 => step != "message",
        // Note place
        5 => step != "note",
        _ => false,
    }
}

pub const SEQUENCE: SheetSpec = SheetSpec {
    fields: &[TITLE],
    tables: &[
        TableSpec::new("Participants", &[text("id", "Id"), text("name", "Name"), choice("type", "Type", PARTICIPANT_KINDS)]),
        TableSpec::new(
            "Steps",
            &[
                choice("step", "Step", STEPS),
                reference("from", "From", 0),
                reference("to", "To", 0),
                text("text", "Text"),
                choice("arrow", "Arrow", ARROW_CHOICES),
                choice("place", "Note", NOTE_PLACES),
            ],
        )
        .hidden(step_hidden),
    ],
    grid: None,
};

fn seq_items(items: &[ast::SequenceItem], s: &mut Sheet, depth: usize) {
    if depth > 32 {
        return;
    }
    let spec = &SEQUENCE;
    let push = |s: &mut Sheet, cells: &[&str]| s.push(spec, 1, cells);
    for it in items {
        match it {
            ast::SequenceItem::Message(m) => {
                let arrow = ARROWS.iter().find(|a| a.3 == m.arrow).map_or("call", |a| a.0);
                push(s, &["message", &m.from, &m.to, &m.text, arrow, ""]);
            }
            ast::SequenceItem::Note(n) => {
                let place = match n.position {
                    ast::NotePosition::LeftOf => "left",
                    ast::NotePosition::Over => "over",
                    _ => "right",
                };
                let from = n.participants.first().map_or("", String::as_str);
                let to = n.participants.get(1).map_or("", String::as_str);
                push(s, &["note", from, to, &n.text, "", place]);
            }
            ast::SequenceItem::Activate(p) => push(s, &["activate", p, "", "", "", ""]),
            ast::SequenceItem::Deactivate(p) => push(s, &["deactivate", p, "", "", "", ""]),
            ast::SequenceItem::Create(p) => push(s, &["create", p, "", "", "", ""]),
            ast::SequenceItem::Destroy(p) => push(s, &["destroy", p, "", "", "", ""]),
            ast::SequenceItem::AutoNumber(c) => {
                let t = c.map(|c| format!("{} {}", num(c.start), num(c.step))).filter(|t| t != "1 1").unwrap_or_default();
                push(s, &["autonumber", "", "", &t, "", ""]);
            }
            ast::SequenceItem::Loop(b) | ast::SequenceItem::Opt(b) | ast::SequenceItem::Break(b) => {
                let kw = match it {
                    ast::SequenceItem::Loop(_) => "loop",
                    ast::SequenceItem::Opt(_) => "opt",
                    _ => "break",
                };
                push(s, &[kw, "", "", &b.label, "", ""]);
                seq_items(&b.items, s, depth + 1);
                push(s, &["end", "", "", "", "", ""]);
            }
            ast::SequenceItem::Alt(br) | ast::SequenceItem::Par(br) | ast::SequenceItem::Critical(br) => {
                let (first, next) = match it {
                    ast::SequenceItem::Alt(_) => ("alt", "else"),
                    ast::SequenceItem::Par(_) => ("par", "and"),
                    _ => ("critical", "option"),
                };
                for (k, b) in br.iter().enumerate() {
                    push(s, &[if k == 0 { first } else { next }, "", "", &b.label, "", ""]);
                    seq_items(&b.items, s, depth + 1);
                }
                push(s, &["end", "", "", "", "", ""]);
            }
            ast::SequenceItem::Rect(r) => {
                push(s, &["rect", "", "", opt(&r.color), "", ""]);
                seq_items(&r.items, s, depth + 1);
                push(s, &["end", "", "", "", "", ""]);
            }
            _ => {}
        }
    }
}

pub fn sequence_read(d: &ast::SequenceDiagram) -> Sheet {
    let mut s = Sheet::new(&SEQUENCE);
    set(&mut s.fields, "title", opt(&d.title));
    for p in &d.participants {
        let kind = match p.kind {
            ast::ParticipantKind::Actor => "actor",
            ast::ParticipantKind::Boundary => "boundary",
            ast::ParticipantKind::Control => "control",
            ast::ParticipantKind::Entity => "entity",
            ast::ParticipantKind::Database => "database",
            _ => "participant",
        };
        s.push(&SEQUENCE, 0, &[&p.id, &p.display, kind]);
    }
    seq_items(&d.items, &mut s, 0);
    s
}

/// A participant's id as written: its id, or one made from its name.
fn participant_id(row: &[String]) -> String {
    let id = cell(row, 0).trim();
    if !id.is_empty() && id.chars().all(|c| c.is_alphanumeric() || c == '_') { id.to_string() } else { ident(cell(row, 1), "p") }
}

/// Text after `:` in a message or note (`;` and `#` would end or escape it).
fn msg(s: &str) -> String {
    one_line(s).replace(';', ",")
}

pub fn sequence_write(s: &Sheet) -> String {
    let mut out = vec!["sequenceDiagram".to_string()];
    title_line(s, &mut out);
    let ids: Vec<String> = s.rows(0).iter().map(|r| participant_id(r)).collect();
    // Participants created later are declared where they are created.
    let created: Vec<&str> = s.rows(1).iter().filter(|r| cell(r, 0) == "create").map(|r| cell(r, 1)).collect();
    let decl = |r: &[String], id: &str| {
        let name = one_line(cell(r, 1));
        let kind = cell(r, 2);
        let head = if kind == "actor" { "actor" } else { "participant" };
        let meta =
            if matches!(kind, "boundary" | "control" | "entity" | "database") { format!("@{{ \"type\" : \"{kind}\" }}") } else { String::new() };
        if name.is_empty() || name == id { format!("{head} {id}{meta}") } else { format!("{head} {id}{meta} as {name}") }
    };
    for (r, id) in s.rows(0).iter().zip(&ids) {
        if !created.contains(&cell(r, 0)) {
            out.push(format!("    {}", decl(r, id)));
        }
    }
    let who = |name: &str| -> String {
        s.rows(0).iter().zip(&ids).find(|(r, _)| cell(r, 0) == name || cell(r, 1) == name).map_or_else(|| ident(name, "p"), |(_, id)| id.clone())
    };
    let mut depth = 1usize;
    for r in s.rows(1) {
        let pad = "    ".repeat(depth);
        let (from, to, text) = (cell(r, 1), cell(r, 2), cell(r, 3));
        match cell(r, 0) {
            "message" => {
                let op = ARROWS.iter().find(|a| a.0 == cell(r, 4)).map_or("->>", |a| a.2);
                out.push(format!("{pad}{}{op}{}: {}", who(from), who(to), msg(text)));
            }
            "note" => {
                let place = match cell(r, 5) {
                    "left" => "left of",
                    "over" => "over",
                    _ => "right of",
                };
                let whom = if to.is_empty() || cell(r, 5) != "over" { who(from) } else { format!("{},{}", who(from), who(to)) };
                out.push(format!("{pad}Note {place} {whom}: {}", msg(text)));
            }
            k @ ("activate" | "deactivate" | "destroy") => out.push(format!("{pad}{k} {}", who(from))),
            "create" => {
                if let Some((row, id)) = s.rows(0).iter().zip(&ids).find(|(row, _)| cell(row, 0) == from || cell(row, 1) == from) {
                    out.push(format!("{pad}create {}", decl(row, id)));
                }
            }
            "autonumber" => {
                out.push(format!("{pad}autonumber{}", if text.trim().is_empty() { String::new() } else { format!(" {}", one_line(text)) }))
            }
            k @ ("loop" | "alt" | "opt" | "par" | "critical" | "break" | "rect") => {
                out.push(format!("{pad}{k}{}", if text.trim().is_empty() { String::new() } else { format!(" {}", one_line(text)) }));
                depth = (depth + 1).min(32);
            }
            k @ ("else" | "and" | "option") => {
                let pad = "    ".repeat(depth.saturating_sub(1).max(1));
                out.push(format!("{pad}{k}{}", if text.trim().is_empty() { String::new() } else { format!(" {}", one_line(text)) }));
            }
            "end" => {
                depth = depth.saturating_sub(1).max(1);
                out.push(format!("{}end", "    ".repeat(depth)));
            }
            _ => {}
        }
    }
    // Blocks left open are closed.
    while depth > 1 {
        depth -= 1;
        out.push(format!("{}end", "    ".repeat(depth)));
    }
    out.join("\n")
}

/// ZenUML: the same table, written in ZenUML's notation (messages `A->B: text`, `if`/`else`,
/// `while`, `opt`, `par`, `try`/`catch`).
pub fn zenuml_write(s: &Sheet) -> String {
    let mut out = vec!["zenuml".to_string()];
    title_line(s, &mut out);
    let ids: Vec<String> = s.rows(0).iter().map(|r| participant_id(r)).collect();
    for (r, id) in s.rows(0).iter().zip(&ids) {
        let name = one_line(cell(r, 1));
        match cell(r, 2) {
            k @ ("actor" | "boundary" | "control" | "entity" | "database") => {
                let mut kw: Vec<char> = k.chars().collect();
                if let Some(c) = kw.first_mut() {
                    *c = c.to_ascii_uppercase();
                }
                out.push(format!("    @{} {id}", kw.into_iter().collect::<String>()));
            }
            _ if !name.is_empty() && name != *id => out.push(format!("    {id} as {name}")),
            _ => out.push(format!("    {id}")),
        }
    }
    let who = |name: &str| -> String {
        s.rows(0).iter().zip(&ids).find(|(r, _)| cell(r, 0) == name || cell(r, 1) == name).map_or_else(|| ident(name, "p"), |(_, id)| id.clone())
    };
    let mut depth = 1usize;
    for r in s.rows(1) {
        let pad = "    ".repeat(depth);
        let text = one_line(cell(r, 3));
        match cell(r, 0) {
            "message" => out.push(format!("{pad}{}->{}: {}", who(cell(r, 1)), who(cell(r, 2)), text.replace(['{', '}'], ""))),
            "loop" => {
                out.push(format!("{pad}while({text}) {{"));
                depth += 1;
            }
            "alt" => {
                out.push(format!("{pad}if({text}) {{"));
                depth += 1;
            }
            "opt" => {
                out.push(format!("{pad}opt {{"));
                depth += 1;
            }
            "par" => {
                out.push(format!("{pad}par {{"));
                depth += 1;
            }
            "critical" => {
                out.push(format!("{pad}try {{"));
                depth += 1;
            }
            "else" => out.push(format!("{}}} else {{", "    ".repeat(depth.saturating_sub(1).max(1)))),
            "option" => out.push(format!("{}}} catch({text}) {{", "    ".repeat(depth.saturating_sub(1).max(1)))),
            "end" => {
                depth = depth.saturating_sub(1).max(1);
                out.push(format!("{}}}", "    ".repeat(depth)));
            }
            _ => {}
        }
    }
    while depth > 1 {
        depth -= 1;
        out.push(format!("{}}}", "    ".repeat(depth)));
    }
    out.join("\n")
}

// ---- gantt ------------------------------------------------------------------

const STATUS: &[(&str, &str)] = &[("", "Planned"), ("active", "In Progress"), ("done", "Done")];

pub const GANTT: SheetSpec = SheetSpec {
    fields: &[
        TITLE,
        text("dateFormat", "Date Format"),
        text("axisFormat", "Axis Format"),
        text("tickInterval", "Tick Interval"),
        text("excludes", "Excluded Days"),
        text("todayMarker", "Today Marker"),
    ],
    tables: &[TableSpec::new(
        "Tasks",
        &[
            text("section", "Section"),
            text("task", "Task"),
            text("id", "Id"),
            text("start", "Start (date, or after id)"),
            text("end", "End (date, or duration: 5d, 1w)"),
            choice("status", "Status", STATUS),
            flag("crit", "Critical"),
            flag("milestone", "Milestone"),
        ],
    )],
    grid: None,
};

/// An automatic task id (`task1`), not written back.
fn auto_id(id: &str) -> bool {
    id.strip_prefix("task").is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

pub fn gantt_read(d: &ast::GanttDiagram) -> Sheet {
    let mut s = Sheet::new(&GANTT);
    set(&mut s.fields, "title", opt(&d.title));
    set(&mut s.fields, "dateFormat", opt(&d.date_format));
    set(&mut s.fields, "axisFormat", opt(&d.axis_format));
    set(&mut s.fields, "tickInterval", opt(&d.tick_interval));
    set(&mut s.fields, "excludes", d.excludes.join(", "));
    set(&mut s.fields, "todayMarker", opt(&d.today_marker));
    for sec in &d.sections {
        for t in &sec.tasks {
            let id = t.id.as_deref().filter(|i| !auto_id(i)).unwrap_or("");
            let start = match &t.start {
                ast::TaskStart::Date(d) => d.clone(),
                ast::TaskStart::AfterId(ids) => format!("after {}", ids.join(" ")),
                _ => String::new(),
            };
            let end = match &t.end {
                ast::TaskEnd::Duration(d) if d.fract() == 0.0 => format!("{}d", num(*d)),
                ast::TaskEnd::Duration(d) => format!("{}h", num((d * 24.0 * 1000.0).round() / 1000.0)),
                ast::TaskEnd::Date(d) => d.clone(),
                ast::TaskEnd::UntilId(i) => format!("until {i}"),
                _ => String::new(),
            };
            let status = match t.status {
                ast::TaskStatus::Active => "active",
                ast::TaskStatus::Done => "done",
                _ => "",
            };
            let b = |v: bool| if v { "true" } else { "" };
            s.push(&GANTT, 0, &[&sec.name, &t.name, id, &start, &end, status, b(t.crit), b(t.milestone)]);
        }
    }
    s
}

pub fn gantt_write(s: &Sheet) -> String {
    let mut out = vec!["gantt".to_string()];
    title_line(s, &mut out);
    for (k, kw) in [
        ("dateFormat", "dateFormat"),
        ("axisFormat", "axisFormat"),
        ("tickInterval", "tickInterval"),
        ("excludes", "excludes"),
        ("todayMarker", "todayMarker"),
    ] {
        let v = one_line(get(&s.fields, k));
        if !v.is_empty() {
            out.push(format!("    {kw} {v}"));
        }
    }
    let mut section: Option<String> = None;
    for r in s.rows(0) {
        let sec = one_line(cell(r, 0));
        if section.as_deref() != Some(sec.as_str()) && (!sec.is_empty() || section.is_some()) {
            out.push(format!("    section {}", if sec.is_empty() { "Tasks".to_string() } else { sec.clone() }));
            section = Some(sec);
        }
        let mut parts: Vec<String> = Vec::new();
        if !cell(r, 5).is_empty() {
            parts.push(cell(r, 5).to_string());
        }
        if cell(r, 6) == "true" {
            parts.push("crit".into());
        }
        if cell(r, 7) == "true" {
            parts.push("milestone".into());
        }
        let clean = |v: &str| one_line(v).replace([',', ':', '#'], " ");
        let (id, start, end) = (clean(cell(r, 2)), clean(cell(r, 3)), clean(cell(r, 4)));
        let end = if end.trim().is_empty() { "1d".to_string() } else { end.trim().to_string() };
        match (id.trim().is_empty(), start.trim().is_empty()) {
            (true, true) => parts.push(end),
            (true, false) => parts.extend([start.trim().to_string(), end]),
            (false, true) => parts.extend([id.trim().to_string(), end]),
            (false, false) => parts.extend([id.trim().to_string(), start.trim().to_string(), end]),
        }
        let name = one_line(cell(r, 1)).replace([':', '#', ';'], " ");
        out.push(format!("    {} :{}", if name.trim().is_empty() { "Task".to_string() } else { name }, parts.join(", ")));
    }
    out.join("\n")
}

// ---- journey ----------------------------------------------------------------

pub const JOURNEY: SheetSpec = SheetSpec {
    fields: &[TITLE],
    tables: &[TableSpec::new(
        "Steps",
        &[text("section", "Section"), text("task", "Task"), number("score", "Score (1 to 5)"), text("actors", "People (comma separated)")],
    )],
    grid: None,
};

pub fn journey_read(d: &ast::JourneyDiagram) -> Sheet {
    let mut s = Sheet::new(&JOURNEY);
    set(&mut s.fields, "title", opt(&d.title));
    for sec in &d.sections {
        for t in &sec.tasks {
            s.push(&JOURNEY, 0, &[&sec.name, &t.name, &t.score.to_string(), &t.actors.join(", ")]);
        }
    }
    s
}

pub fn journey_write(s: &Sheet) -> String {
    let mut out = vec!["journey".to_string()];
    title_line(s, &mut out);
    let mut section: Option<String> = None;
    for r in s.rows(0) {
        let sec = one_line(cell(r, 0));
        if section.as_deref() != Some(sec.as_str()) {
            out.push(format!("    section {}", if sec.is_empty() { "Steps".to_string() } else { sec.clone() }));
            section = Some(sec);
        }
        let score = cell(r, 2).trim().parse::<i64>().unwrap_or(3).clamp(0, 9);
        let actors: Vec<String> = cell(r, 3).split(',').map(|a| one_line(a).replace(':', " ")).filter(|a| !a.trim().is_empty()).collect();
        out.push(format!(
            "      {}: {score}{}",
            one_line(cell(r, 1)).replace(':', " "),
            if actors.is_empty() { String::new() } else { format!(": {}", actors.join(", ")) }
        ));
    }
    out.join("\n")
}

// ---- timeline ----------------------------------------------------------------

pub const TIMELINE: SheetSpec = SheetSpec {
    fields: &[TITLE],
    tables: &[TableSpec::new("Events", &[text("section", "Section"), text("period", "Period"), text("event", "Event")])],
    grid: None,
};

pub fn timeline_read(d: &ast::TimelineDiagram) -> Sheet {
    let mut s = Sheet::new(&TIMELINE);
    set(&mut s.fields, "title", opt(&d.title));
    for sec in &d.sections {
        let name = opt(&sec.name);
        for p in &sec.periods {
            if p.events.is_empty() {
                s.push(&TIMELINE, 0, &[name, &p.label, ""]);
            }
            for e in &p.events {
                s.push(&TIMELINE, 0, &[name, &p.label, e]);
            }
        }
    }
    s
}

pub fn timeline_write(s: &Sheet) -> String {
    let mut out = vec!["timeline".to_string()];
    title_line(s, &mut out);
    let mut section: Option<String> = None;
    let mut period: Option<String> = None;
    for r in s.rows(0) {
        let sec = one_line(cell(r, 0));
        let per = one_line(cell(r, 1)).replace(':', " ");
        let ev = one_line(cell(r, 2)).replace(':', " ");
        if section.as_deref() != Some(sec.as_str()) && !(section.is_none() && sec.is_empty()) {
            out.push(format!("    section {}", if sec.is_empty() { "-".to_string() } else { sec.clone() }));
            period = None;
        }
        section = Some(sec);
        if period.as_deref() == Some(per.as_str()) && !ev.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&format!(" : {ev}"));
            }
            continue;
        }
        out.push(if ev.is_empty() { format!("        {per}") } else { format!("        {per} : {ev}") });
        period = Some(per);
    }
    out.join("\n")
}

// ---- sankey ------------------------------------------------------------------

pub const SANKEY: SheetSpec = SheetSpec {
    fields: &[],
    tables: &[TableSpec::new("Flows", &[text("source", "From"), text("target", "To"), number("value", "Amount")])],
    grid: None,
};

pub fn sankey_read(d: &ast::SankeyDiagram) -> Sheet {
    let mut s = Sheet::new(&SANKEY);
    for l in &d.links {
        s.push(&SANKEY, 0, &[&l.source, &l.target, &num(l.value)]);
    }
    s
}

fn csv(s: &str) -> String {
    let s = one_line(s);
    if s.contains([',', '"']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
}

pub fn sankey_write(s: &Sheet) -> String {
    let mut out = vec!["sankey-beta".to_string(), String::new()];
    for r in s.rows(0) {
        out.push(format!("{},{},{}", csv(cell(r, 0)), csv(cell(r, 1)), num(parse_num(cell(r, 2)))));
    }
    out.join("\n")
}

// ---- quadrant ----------------------------------------------------------------

pub const QUADRANT: SheetSpec = SheetSpec {
    fields: &[
        TITLE,
        text("xLeft", "X Axis, Left"),
        text("xRight", "X Axis, Right"),
        text("yBottom", "Y Axis, Bottom"),
        text("yTop", "Y Axis, Top"),
        text("q1", "Top Right Quadrant"),
        text("q2", "Top Left Quadrant"),
        text("q3", "Bottom Left Quadrant"),
        text("q4", "Bottom Right Quadrant"),
    ],
    tables: &[TableSpec::new("Points", &[text("point", "Point"), number("x", "X (0 to 1)"), number("y", "Y (0 to 1)")])],
    grid: None,
};

pub fn quadrant_read(d: &ast::QuadrantDiagram) -> Sheet {
    let mut s = Sheet::new(&QUADRANT);
    for (k, v) in [
        ("title", &d.title),
        ("xLeft", &d.x_axis_left),
        ("xRight", &d.x_axis_right),
        ("yBottom", &d.y_axis_bottom),
        ("yTop", &d.y_axis_top),
        ("q1", &d.q1),
        ("q2", &d.q2),
        ("q3", &d.q3),
        ("q4", &d.q4),
    ] {
        set(&mut s.fields, k, opt(v));
    }
    for p in &d.points {
        s.push(&QUADRANT, 0, &[&p.label, &num(p.x), &num(p.y)]);
    }
    s
}

pub fn quadrant_write(s: &Sheet) -> String {
    let mut out = vec!["quadrantChart".to_string()];
    title_line(s, &mut out);
    let f = |k: &str| one_line(get(&s.fields, k)).replace("-->", "→");
    for (axis, a, b) in [("x-axis", "xLeft", "xRight"), ("y-axis", "yBottom", "yTop")] {
        match (f(a).is_empty(), f(b).is_empty()) {
            (true, true) => {}
            (false, true) => out.push(format!("    {axis} {}", f(a))),
            _ => out.push(format!("    {axis} {} --> {}", f(a), f(b))),
        }
    }
    for q in ["q1", "q2", "q3", "q4"] {
        if !f(q).is_empty() {
            out.push(format!("    quadrant-{} {}", &q[1..], f(q)));
        }
    }
    for r in s.rows(0) {
        let label = one_line(cell(r, 0)).replace(':', " ");
        out.push(format!(
            "    {}: [{}, {}]",
            if label.trim().is_empty() { "Point".to_string() } else { label },
            num(parse_num(cell(r, 1)).clamp(0.0, 1.0)),
            num(parse_num(cell(r, 2)).clamp(0.0, 1.0))
        ));
    }
    out.join("\n")
}

// ---- packet ----------------------------------------------------------------

pub const PACKET: SheetSpec = SheetSpec {
    fields: &[TITLE],
    tables: &[TableSpec::new("Fields", &[number("start", "First Bit"), number("end", "Last Bit"), text("label", "Field")])],
    grid: None,
};

pub fn packet_read(d: &ast::PacketDiagram) -> Sheet {
    let mut s = Sheet::new(&PACKET);
    set(&mut s.fields, "title", opt(&d.title));
    for f in &d.fields {
        s.push(&PACKET, 0, &[&f.start.to_string(), &f.end.to_string(), &f.label]);
    }
    s
}

pub fn packet_write(s: &Sheet) -> String {
    let mut out = vec!["packet-beta".to_string()];
    title_line(s, &mut out);
    for r in s.rows(0) {
        let a = cell(r, 0).trim().parse::<u32>().unwrap_or(0).min(1_000_000);
        let b = cell(r, 1).trim().parse::<u32>().unwrap_or(a).clamp(a, 1_000_000);
        let range = if a == b { a.to_string() } else { format!("{a}-{b}") };
        out.push(format!("    {range}: {}", quoted(cell(r, 2))));
    }
    out.join("\n")
}

// ---- git graph ---------------------------------------------------------------

const GIT_ACTIONS: &[(&str, &str)] =
    &[("commit", "Commit"), ("branch", "New Branch"), ("checkout", "Switch to Branch"), ("merge", "Merge"), ("cherry-pick", "Cherry-pick")];
const COMMIT_TYPES: &[(&str, &str)] = &[("", "Normal"), ("HIGHLIGHT", "Highlighted"), ("REVERSE", "Reverted")];
const GIT_DIRS: &[(&str, &str)] = &[("LR", "Left to Right"), ("TB", "Top to Bottom"), ("BT", "Bottom to Top")];

fn git_hidden(row: &[String], col: usize) -> bool {
    let a = cell(row, 0);
    match col {
        // Branch
        1 => matches!(a, "commit" | "cherry-pick"),
        // Commit id
        2 => matches!(a, "branch" | "checkout"),
        // Tag
        3 => matches!(a, "branch" | "checkout"),
        // Type
        4 => !matches!(a, "commit" | "merge"),
        // Order or parent
        5 => !matches!(a, "branch" | "cherry-pick"),
        _ => false,
    }
}

pub const GITGRAPH: SheetSpec = SheetSpec {
    fields: &[TITLE, choice("dir", "Direction", GIT_DIRS)],
    tables: &[TableSpec::new(
        "History",
        &[
            choice("action", "Action", GIT_ACTIONS),
            text("branch", "Branch"),
            text("id", "Commit Id"),
            text("tag", "Tag"),
            choice("type", "Type", COMMIT_TYPES),
            text("more", "Order or Parent"),
        ],
    )
    .hidden(git_hidden)],
    grid: None,
};

fn commit_type(k: ast::CommitKind) -> &'static str {
    match k {
        ast::CommitKind::Highlight => "HIGHLIGHT",
        ast::CommitKind::Reverse => "REVERSE",
        _ => "",
    }
}

pub fn gitgraph_read(d: &ast::GitGraphDiagram) -> Sheet {
    let mut s = Sheet::new(&GITGRAPH);
    set(&mut s.fields, "title", opt(&d.title));
    let dir = match d.direction {
        ast::GitDirection::TopDown => "TB",
        ast::GitDirection::BottomTop => "BT",
        _ => "LR",
    };
    set(&mut s.fields, "dir", dir);
    for e in &d.events {
        match e {
            ast::GitEvent::Commit { id, tags, kind } => s.push(&GITGRAPH, 0, &["commit", "", opt(id), &tags.join(", "), commit_type(*kind), ""]),
            ast::GitEvent::Branch { name, order } => {
                s.push(&GITGRAPH, 0, &["branch", name, "", "", "", &order.map(|o| o.to_string()).unwrap_or_default()]);
            }
            ast::GitEvent::Checkout { name } => s.push(&GITGRAPH, 0, &["checkout", name, "", "", "", ""]),
            ast::GitEvent::Merge { from, id, tags, kind } => {
                s.push(&GITGRAPH, 0, &["merge", from, opt(id), &tags.join(", "), commit_type(*kind), ""])
            }
            ast::GitEvent::CherryPick { commit_id, parent, tag } => s.push(&GITGRAPH, 0, &["cherry-pick", "", commit_id, opt(tag), "", opt(parent)]),
            _ => {}
        }
    }
    s
}

pub fn gitgraph_write(s: &Sheet) -> String {
    let dir = get(&s.fields, "dir");
    let mut out = vec![if matches!(dir, "TB" | "BT") { format!("gitGraph {dir}:") } else { "gitGraph".to_string() }];
    title_line(s, &mut out);
    let q = |v: &str| quoted(v);
    for r in s.rows(0) {
        let (branch, id, tag, ty, more) = (ident(cell(r, 1), "b"), one_line(cell(r, 2)), cell(r, 3), cell(r, 4), cell(r, 5).trim());
        let tags = |line: &mut String| {
            for t in tag.split(',').map(one_line).filter(|t| !t.trim().is_empty()) {
                line.push_str(&format!(" tag: {}", q(t.trim())));
            }
        };
        match cell(r, 0) {
            "branch" => {
                out.push(format!("    branch {branch}{}", if more.parse::<usize>().is_ok() { format!(" order: {more}") } else { String::new() }))
            }
            "checkout" => out.push(format!("    checkout {branch}")),
            a @ ("merge" | "commit") => {
                let mut line = if a == "merge" { format!("    merge {branch}") } else { "    commit".to_string() };
                if !id.trim().is_empty() {
                    line.push_str(&format!(" id: {}", q(&id)));
                }
                tags(&mut line);
                if matches!(ty, "HIGHLIGHT" | "REVERSE") {
                    line.push_str(&format!(" type: {ty}"));
                }
                out.push(line);
            }
            "cherry-pick" => {
                let mut line = format!("    cherry-pick id: {}", q(&id));
                if !more.is_empty() {
                    line.push_str(&format!(" parent: {}", q(more)));
                }
                tags(&mut line);
                out.push(line);
            }
            _ => {}
        }
    }
    out.join("\n")
}

// ---- kanban ---------------------------------------------------------------

const PRIORITIES: &[(&str, &str)] = &[("", "None"), ("Very High", "Very High"), ("High", "High"), ("Low", "Low"), ("Very Low", "Very Low")];

pub const KANBAN: SheetSpec = SheetSpec {
    fields: &[],
    tables: &[TableSpec::new(
        "Cards",
        &[
            text("column", "Column"),
            text("task", "Card (empty: an empty column)"),
            text("assigned", "Assigned to"),
            choice("priority", "Priority", PRIORITIES),
            text("ticket", "Ticket"),
            text("_column", "Column Id"),
            text("_id", "Card Id"),
        ],
    )],
    grid: None,
};

pub fn kanban_read(d: &ast::KanbanDiagram) -> Sheet {
    let mut s = Sheet::new(&KANBAN);
    for c in &d.columns {
        if c.tasks.is_empty() {
            s.push(&KANBAN, 0, &[&c.label, "", "", "", "", &c.id, ""]);
        }
        for t in &c.tasks {
            s.push(&KANBAN, 0, &[&c.label, &t.text, opt(&t.assigned), opt(&t.priority), opt(&t.ticket), &c.id, &t.id]);
        }
    }
    s
}

fn bracket(s: &str) -> String {
    one_line(s).replace(['[', ']'], "")
}

pub fn kanban_write(s: &Sheet) -> String {
    let mut out = vec!["kanban".to_string()];
    let mut column: Option<String> = None;
    let mut used: Vec<String> = Vec::new();
    let fresh = |base: &str, used: &mut Vec<String>| {
        let id = crate::form::unique(&ident(base, "k"), &|c| used.iter().any(|u| u == c));
        used.push(id.clone());
        id
    };
    for r in s.rows(0) {
        let col = one_line(cell(r, 0));
        if column.as_deref() != Some(col.as_str()) {
            let id = if cell(r, 5).trim().is_empty() { fresh(&col, &mut used) } else { fresh(cell(r, 5), &mut used) };
            out.push(format!("  {id}[{}]", bracket(&col)));
            column = Some(col);
        }
        let task = one_line(cell(r, 1));
        if task.is_empty() {
            continue;
        }
        let id = if cell(r, 6).trim().is_empty() { fresh(&task, &mut used) } else { fresh(cell(r, 6), &mut used) };
        let mut meta: Vec<String> = Vec::new();
        for (k, i) in [("assigned", 2), ("priority", 3), ("ticket", 4)] {
            let v = one_line(cell(r, i)).replace(['\'', '}'], "");
            if !v.trim().is_empty() {
                meta.push(format!("{k}: '{v}'"));
            }
        }
        let meta = if meta.is_empty() { String::new() } else { format!("@{{ {} }}", meta.join(", ")) };
        out.push(format!("    {id}[{}]{meta}", bracket(&task)));
    }
    out.join("\n")
}

// ---- mind map ---------------------------------------------------------------

const MIND_SHAPES: &[(&str, &str)] = &[
    ("", "Plain"),
    ("square", "Square"),
    ("rounded", "Rounded"),
    ("circle", "Circle"),
    ("bang", "Bang"),
    ("cloud", "Cloud"),
    ("hexagon", "Hexagon"),
];

pub const MINDMAP: SheetSpec = SheetSpec {
    fields: &[],
    tables: &[TableSpec::new(
        "Ideas",
        &[number("level", "Level"), text("text", "Idea"), choice("shape", "Shape", MIND_SHAPES), text("icon", "Icon")],
    )
    .tree()],
    grid: None,
};

fn mind_read(n: &ast::MindmapNode, lvl: usize, s: &mut Sheet) {
    if lvl > crate::sheet::MAX_LEVEL {
        return;
    }
    let shape = match n.shape {
        ast::MindmapShape::Square => "square",
        ast::MindmapShape::Rounded => "rounded",
        ast::MindmapShape::Circle => "circle",
        ast::MindmapShape::Bang => "bang",
        ast::MindmapShape::Cloud => "cloud",
        ast::MindmapShape::Hexagon => "hexagon",
        _ => "",
    };
    s.push(&MINDMAP, 0, &[&lvl.to_string(), &n.text, shape, opt(&n.icon)]);
    for c in &n.children {
        mind_read(c, lvl + 1, s);
    }
}

pub fn mindmap_read(d: &ast::MindmapDiagram) -> Sheet {
    let mut s = Sheet::new(&MINDMAP);
    if let Some(r) = &d.root {
        mind_read(r, 0, &mut s);
    }
    s
}

pub fn mindmap_write(s: &Sheet) -> String {
    let mut out = vec!["mindmap".to_string()];
    let mut prev = 0usize;
    for (k, r) in s.rows(0).iter().enumerate() {
        // One root; each idea at most one level under the one before.
        let lvl = if k == 0 { 0 } else { level(r).clamp(1, prev + 1) };
        prev = lvl;
        let pad = "  ".repeat(lvl + 1);
        let t = one_line(cell(r, 1));
        let clean = t.replace(['(', ')', '[', ']', '{', '}'], " ");
        let id = format!("n{}", k + 1);
        let node = match cell(r, 2) {
            "square" => format!("{id}[{}]", quoted(&t)),
            "rounded" => format!("{id}({})", quoted(&t)),
            "circle" => format!("{id}(({}))", quoted(&t)),
            "bang" => format!("{id})){}((", quoted(&t)),
            "cloud" => format!("{id}){}(", quoted(&t)),
            "hexagon" => format!("{id}{{{{{}}}}}", quoted(&t)),
            _ if clean.trim().is_empty() => "Idea".to_string(),
            _ => clean.trim().to_string(),
        };
        out.push(format!("{pad}{node}"));
        let icon = one_line(cell(r, 3)).replace([')', '('], "");
        if !icon.trim().is_empty() {
            out.push(format!("{pad}  ::icon({})", icon.trim()));
        }
    }
    out.join("\n")
}

// ---- treemap ----------------------------------------------------------------

pub const TREEMAP: SheetSpec = SheetSpec {
    fields: &[TITLE],
    tables: &[TableSpec::new("Items", &[number("level", "Level"), text("label", "Item"), number("value", "Value (leaves)")]).tree()],
    grid: None,
};

fn tree_read(n: &ast::TreemapNode, lvl: usize, s: &mut Sheet) {
    if lvl > crate::sheet::MAX_LEVEL {
        return;
    }
    s.push(&TREEMAP, 0, &[&lvl.to_string(), &n.label, &n.value.map(num).unwrap_or_default()]);
    for c in &n.children {
        tree_read(c, lvl + 1, s);
    }
}

pub fn treemap_read(d: &ast::TreemapDiagram) -> Sheet {
    let mut s = Sheet::new(&TREEMAP);
    set(&mut s.fields, "title", opt(&d.title));
    for n in &d.root {
        tree_read(n, 0, &mut s);
    }
    s
}

pub fn treemap_write(s: &Sheet) -> String {
    let mut out = vec!["treemap-beta".to_string()];
    title_line(s, &mut out);
    let mut prev: Option<usize> = None;
    for r in s.rows(0) {
        let lvl = level(r).min(prev.map_or(0, |p| p + 1));
        prev = Some(lvl);
        let v = cell(r, 2).trim();
        let value = if v.is_empty() { String::new() } else { format!(": {}", num(parse_num(v))) };
        out.push(format!("{}{}{value}", "    ".repeat(lvl), quoted(cell(r, 1))));
    }
    out.join("\n")
}

// ---- pie ---------------------------------------------------------------------

pub const PIE: SheetSpec = SheetSpec {
    fields: &[TITLE, flag("showData", "Show the Values")],
    tables: &[TableSpec::new("Slices", &[text("label", "Slice"), number("value", "Value")])],
    grid: None,
};

pub fn pie_read(d: &ast::PieDiagram) -> Sheet {
    let mut s = Sheet::new(&PIE);
    set(&mut s.fields, "title", opt(&d.title));
    if d.show_data {
        set(&mut s.fields, "showData", "true");
    }
    for e in &d.entries {
        s.push(&PIE, 0, &[&e.label, &num(e.value)]);
    }
    s
}

pub fn pie_write(s: &Sheet) -> String {
    let t = one_line(get(&s.fields, "title"));
    let mut head = "pie".to_string();
    if is(&s.fields, "showData") {
        head.push_str(" showData");
    }
    if !t.is_empty() {
        head.push_str(&format!(" title {t}"));
    }
    let mut out = vec![head];
    for r in s.rows(0) {
        out.push(format!("    {} : {}", quoted(cell(r, 0)), num(parse_num(cell(r, 1)).max(0.0))));
    }
    out.join("\n")
}

// ---- xy chart --------------------------------------------------------------

const SERIES_KINDS: &[(&str, &str)] = &[("bar", "Bars"), ("line", "Line")];

pub const XYCHART: SheetSpec = SheetSpec {
    fields: &[
        TITLE,
        flag("horizontal", "Horizontal Bars"),
        text("xTitle", "Category Axis Title"),
        text("yTitle", "Value Axis Title"),
        number("yMin", "Value Axis Minimum"),
        number("yMax", "Value Axis Maximum"),
    ],
    tables: &[],
    grid: Some(GridSpec { row_label: "Category", col_label: "Series", col_kinds: Some(SERIES_KINDS) }),
};

pub fn xychart_read(d: &ast::XyChartDiagram) -> Sheet {
    let mut s = Sheet::new(&XYCHART);
    set(&mut s.fields, "title", opt(&d.title));
    if d.horizontal {
        set(&mut s.fields, "horizontal", "true");
    }
    let mut cats: Vec<String> = Vec::new();
    if let Some(x) = &d.x_axis {
        set(&mut s.fields, "xTitle", opt(&x.title));
        if let ast::XyAxisKind::Categories(c) = &x.kind {
            cats = c.clone();
        }
    }
    if let Some(y) = &d.y_axis {
        set(&mut s.fields, "yTitle", opt(&y.title));
        if let ast::XyAxisKind::Range { min, max } = y.kind {
            set(&mut s.fields, "yMin", num(min));
            set(&mut s.fields, "yMax", num(max));
        }
    }
    let n = d.series.iter().map(|x| x.values.len()).max().unwrap_or(0).max(cats.len());
    cats.resize(n, String::new());
    for (k, x) in d.series.iter().enumerate() {
        let kind = if x.kind == ast::XySeriesKind::Line { "line" } else { "bar" };
        s.grid.cols.push((x.title.clone().unwrap_or_default(), kind.into()));
        let _ = k;
    }
    for (i, c) in cats.iter().enumerate() {
        s.grid.rows.push((c.clone(), d.series.iter().map(|x| x.values.get(i).map(|v| num(*v)).unwrap_or_default()).collect()));
    }
    s
}

pub fn xychart_write(s: &Sheet) -> String {
    let mut out = vec![if is(&s.fields, "horizontal") { "xychart-beta horizontal".to_string() } else { "xychart-beta".to_string() }];
    let t = one_line(get(&s.fields, "title"));
    if !t.is_empty() {
        out.push(format!("    title {}", quoted(&t)));
    }
    let cats: Vec<String> = s.grid.rows.iter().map(|(c, _)| quoted(c)).collect();
    let xt = one_line(get(&s.fields, "xTitle"));
    out.push(format!("    x-axis{} [{}]", if xt.is_empty() { String::new() } else { format!(" {}", quoted(&xt)) }, cats.join(", ")));
    let yt = one_line(get(&s.fields, "yTitle"));
    let (lo, hi) = (get(&s.fields, "yMin").trim(), get(&s.fields, "yMax").trim());
    let range = if !lo.is_empty() && !hi.is_empty() { format!(" {} --> {}", num(parse_num(lo)), num(parse_num(hi))) } else { String::new() };
    if !yt.is_empty() || !range.is_empty() {
        out.push(format!("    y-axis{}{range}", if yt.is_empty() { String::new() } else { format!(" {}", quoted(&yt)) }));
    }
    for (k, (name, kind)) in s.grid.cols.iter().enumerate() {
        let vals: Vec<String> = s.grid.rows.iter().map(|(_, v)| num(parse_num(v.get(k).map_or("", String::as_str)))).collect();
        let mark = if kind == "line" { "line" } else { "bar" };
        let name = one_line(name);
        out.push(format!("    {mark}{} [{}]", if name.is_empty() { String::new() } else { format!(" {}", quoted(&name)) }, vals.join(", ")));
    }
    out.join("\n")
}

// ---- radar ---------------------------------------------------------------

const GRATICULES: &[(&str, &str)] = &[("", "Circles"), ("polygon", "Polygons")];

pub const RADAR: SheetSpec = SheetSpec {
    fields: &[
        TITLE,
        number("min", "Minimum"),
        number("max", "Maximum"),
        number("ticks", "Rings"),
        choice("graticule", "Grid", GRATICULES),
        flag("hideLegend", "Hide the Legend"),
    ],
    tables: &[],
    grid: Some(GridSpec { row_label: "Axis", col_label: "Curve", col_kinds: None }),
};

pub fn radar_read(d: &ast::RadarDiagram) -> Sheet {
    let mut s = Sheet::new(&RADAR);
    set(&mut s.fields, "title", opt(&d.title));
    set(&mut s.fields, "min", d.min.map(num).unwrap_or_default());
    set(&mut s.fields, "max", d.max.map(num).unwrap_or_default());
    set(&mut s.fields, "ticks", d.ticks.map(|t| t.to_string()).unwrap_or_default());
    if d.graticule == ast::RadarGraticule::Polygon {
        set(&mut s.fields, "graticule", "polygon");
    }
    if d.show_legend == Some(false) {
        set(&mut s.fields, "hideLegend", "true");
    }
    for c in &d.curves {
        s.grid.cols.push((c.label.clone(), String::new()));
        s.grid.col_ids.push(c.id.clone());
    }
    for (i, a) in d.axes.iter().enumerate() {
        s.grid.rows.push((a.label.clone(), d.curves.iter().map(|c| c.values.get(i).map(|v| num(*v)).unwrap_or_default()).collect()));
        s.grid.row_ids.push(a.id.clone());
    }
    s
}

/// `id["label"]` (just `id` when they are the same).
fn labelled(id: &str, label: &str) -> String {
    if id == label { id.to_string() } else { format!("{id}[{}]", quoted(label)) }
}

pub fn radar_write(s: &Sheet) -> String {
    let mut out = vec!["radar-beta".to_string()];
    title_line(s, &mut out);
    let id_of = |ids: &[String], k: usize, label: &str| ids.get(k).filter(|i| !i.is_empty()).cloned().unwrap_or_else(|| ident(label, "a"));
    let axes: Vec<String> = s.grid.rows.iter().enumerate().map(|(k, (l, _))| labelled(&id_of(&s.grid.row_ids, k, l), &one_line(l))).collect();
    if !axes.is_empty() {
        out.push(format!("    axis {}", axes.join(", ")));
    }
    for (k, (name, _)) in s.grid.cols.iter().enumerate() {
        let vals: Vec<String> = s.grid.rows.iter().map(|(_, v)| num(parse_num(v.get(k).map_or("", String::as_str)))).collect();
        out.push(format!("    curve {}{{{}}}", labelled(&id_of(&s.grid.col_ids, k, name), &one_line(name)), vals.join(", ")));
    }
    for k in ["max", "min", "ticks"] {
        let v = get(&s.fields, k).trim();
        if !v.is_empty() {
            out.push(format!("    {k} {}", num(parse_num(v))));
        }
    }
    if get(&s.fields, "graticule") == "polygon" {
        out.push("    graticule polygon".into());
    }
    if is(&s.fields, "hideLegend") {
        out.push("    showLegend false".into());
    }
    out.join("\n")
}
