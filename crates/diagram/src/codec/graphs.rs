//! Plume: the canvas diagrams in Mermaid: what each offers ([`GraphSpec`]), how it is written,
//! and how it is read back from the parsed diagram (`mermaid_svg::ast`). Each writer emits the
//! diagram's header and statements; the shapes' places follow as `%% plume` comments.

use mermaid_svg::ast;

use crate::form::{FieldSpec, FieldType::Choice, choice, flag, get, ident, is, one_line, quoted, set, text};
use crate::graph::{EdgeKind, Graph, GraphSpec, Head, Look, Node, NodeKind};
use crate::sketch::{LineStyle, NodeShape, Sketch};

/// The direction written: the drawing's, else `TB`.
fn dir_of(g: &Graph) -> &'static str {
    g.direction().unwrap_or("TB")
}

fn flow_dir(d: ast::FlowDirection) -> bool {
    // Left to right or right to left: ranks run across.
    matches!(d, ast::FlowDirection::LeftRight | ast::FlowDirection::RightLeft)
}

// ---- flowchart ---------------------------------------------------------------

const FLOW_NODES: &[NodeKind] = &[
    NodeKind::new("rectangle", "Rectangle", Look::Rect),
    NodeKind::new("rounded", "Rounded Rectangle", Look::Rounded),
    NodeKind::new("stadium", "Terminator", Look::Stadium),
    NodeKind::new("diamond", "Decision", Look::Diamond),
    NodeKind::new("parallelogram", "Data", Look::Parallelogram),
    NodeKind::new("parallelogramAlt", "Data (Left)", Look::ParallelogramAlt),
    NodeKind::new("subroutine", "Predefined Process", Look::Subroutine),
    NodeKind::new("hexagon", "Preparation", Look::Hexagon),
    NodeKind::new("trapezoid", "Manual Operation", Look::Trapezoid),
    NodeKind::new("trapezoidAlt", "Trapezoid (Inverted)", Look::TrapezoidAlt),
    NodeKind::new("database", "Database", Look::Cylinder),
    NodeKind::new("circle", "Circle", Look::Circle),
    NodeKind::new("doubleCircle", "Double Circle", Look::DoubleCircle),
    NodeKind::new("flag", "Flag", Look::Flag),
];

const LINKS: &[EdgeKind] = &[
    EdgeKind::new("arrow", "Arrow", Head::None, Head::Arrow),
    EdgeKind::new("line", "Line", Head::None, Head::None),
    EdgeKind::new("dottedArrow", "Dotted Arrow", Head::None, Head::Arrow).dashed(),
    EdgeKind::new("dotted", "Dotted Line", Head::None, Head::None).dashed(),
    EdgeKind::new("thickArrow", "Thick Arrow", Head::None, Head::Arrow).thick(),
    EdgeKind::new("thick", "Thick Line", Head::None, Head::None).thick(),
];

pub const FLOWCHART: GraphSpec = GraphSpec { fields: &[], nodes: FLOW_NODES, edges: LINKS, edge_label: true };

fn link_kind(style: LineStyle, arrow: bool) -> &'static str {
    match (style, arrow) {
        (LineStyle::Solid, true) => "arrow",
        (LineStyle::Solid, false) => "line",
        (LineStyle::Dotted, true) => "dottedArrow",
        (LineStyle::Dotted, false) => "dotted",
        (LineStyle::Thick, true) => "thickArrow",
        (LineStyle::Thick, false) => "thick",
    }
}

fn link_style(kind: &str) -> (LineStyle, bool) {
    match kind {
        "line" => (LineStyle::Solid, false),
        "dottedArrow" => (LineStyle::Dotted, true),
        "dotted" => (LineStyle::Dotted, false),
        "thickArrow" => (LineStyle::Thick, true),
        "thick" => (LineStyle::Thick, false),
        _ => (LineStyle::Solid, true),
    }
}

/// A flowchart (Plume's own reader, [`Sketch`], which also keeps style lines) as a graph.
pub fn flowchart_read(src: &str) -> Option<Graph> {
    let sk = Sketch::parse(src)?;
    let nodes = sk
        .nodes
        .iter()
        .map(|n| Node { id: n.id.clone(), kind: n.shape.id().into(), text: n.text.clone(), x: n.x, y: n.y, ..Default::default() })
        .collect();
    let edges = sk
        .edges
        .iter()
        .map(|e| crate::graph::Edge { from: e.from, to: e.to, kind: link_kind(e.line, e.arrow).into(), label: e.label.clone(), ..Default::default() })
        .collect();
    let mut fields = std::collections::BTreeMap::new();
    set(&mut fields, "dir", sk.dir.name());
    Some(Graph { nodes, edges, fields, extra: sk.extra })
}

pub fn flowchart_write(g: &Graph) -> String {
    let mut sk = Sketch { extra: g.extra.clone(), ..Default::default() };
    sk.dir = crate::sketch::Dir::parse(get(&g.fields, "dir")).unwrap_or_default();
    for n in &g.nodes {
        sk.nodes.push(crate::sketch::Node { id: n.id.clone(), text: n.text.clone(), shape: NodeShape::from_id(&n.kind), x: n.x, y: n.y });
    }
    for e in &g.edges {
        let (line, arrow) = link_style(&e.kind);
        sk.edges.push(crate::sketch::Edge { from: e.from, to: e.to, label: e.label.clone(), arrow, line });
    }
    sk.to_mermaid()
}

// ---- state ---------------------------------------------------------------

const STATE_NODES: &[NodeKind] = &[
    NodeKind::new("state", "State", Look::Rounded),
    NodeKind::new("start", "Start", Look::Dot).bare(),
    NodeKind::new("end", "End", Look::Bullseye).bare(),
    NodeKind::new("choice", "Choice", Look::Diamond).bare(),
    NodeKind::new("fork", "Fork", Look::Bar).bare(),
    NodeKind::new("join", "Join", Look::Bar).bare(),
];

pub const STATE: GraphSpec =
    GraphSpec { fields: &[], nodes: STATE_NODES, edges: &[EdgeKind::new("transition", "Transition", Head::None, Head::Arrow)], edge_label: true };

pub fn state_read(d: &ast::StateDiagram, src: &str) -> Graph {
    let mut g = Graph::default();
    for s in &d.states {
        let kind = match s.kind {
            ast::StateKind::Start => "start",
            ast::StateKind::End => "end",
            ast::StateKind::Choice => "choice",
            ast::StateKind::Fork => "fork",
            ast::StateKind::Join => "join",
            _ => "state",
        };
        g.nodes.push(Node { id: s.id.clone(), kind: kind.into(), text: s.label.clone(), ..Default::default() });
    }
    for t in &d.transitions {
        if let (Some(from), Some(to)) = (g.index(&t.from), g.index(&t.to)) {
            g.edges.push(crate::graph::Edge {
                from,
                to,
                kind: "transition".into(),
                label: t.label.clone().unwrap_or_default(),
                ..Default::default()
            });
        }
    }
    g.restore_positions(src, flow_dir(d.direction));
    g
}

pub fn state_write(g: &Graph) -> String {
    let mut out = vec!["stateDiagram-v2".to_string()];
    let dir = dir_of(g);
    if dir != "TB" {
        out.push(format!("    direction {dir}"));
    }
    let name = |n: &Node| if n.id.starts_with("__") || n.id.is_empty() { ident(&n.text, "s") } else { n.id.clone() };
    for n in &g.nodes {
        match n.kind.as_str() {
            "start" | "end" => {}
            "choice" | "fork" | "join" => out.push(format!("    state {} <<{}>>", name(n), n.kind)),
            _ if n.text.trim().is_empty() || n.text == name(n) => out.push(format!("    {}", name(n))),
            _ => out.push(format!("    state {} as {}", quoted(&n.text), name(n))),
        }
    }
    for e in &g.edges {
        let (Some(a), Some(b)) = (g.nodes.get(e.from), g.nodes.get(e.to)) else { continue };
        let end = |n: &Node| if matches!(n.kind.as_str(), "start" | "end") { "[*]".to_string() } else { name(n) };
        let label = one_line(&e.label);
        if label.is_empty() {
            out.push(format!("    {} --> {}", end(a), end(b)));
        } else {
            out.push(format!("    {} --> {} : {}", end(a), end(b), label));
        }
    }
    out.join("\n")
}

// ---- class ---------------------------------------------------------------

const CLASS_FIELDS: &[FieldSpec] = &[text("label", "Display Name"), text("stereotype", "Stereotype")];
const CARDS: &[FieldSpec] = &[text("fromCard", "Start Multiplicity"), text("toCard", "End Multiplicity")];

const CLASS_EDGES: &[EdgeKind] = &[
    EdgeKind::new("--|>", "Inheritance", Head::None, Head::Triangle).fields(CARDS),
    EdgeKind::new("..|>", "Realization", Head::None, Head::Triangle).dashed().fields(CARDS),
    EdgeKind::new("--*", "Composition", Head::None, Head::DiamondFilled).fields(CARDS),
    EdgeKind::new("--o", "Aggregation", Head::None, Head::DiamondHollow).fields(CARDS),
    EdgeKind::new("-->", "Association", Head::None, Head::Arrow).fields(CARDS),
    EdgeKind::new("..>", "Dependency", Head::None, Head::Arrow).dashed().fields(CARDS),
    EdgeKind::new("--", "Link", Head::None, Head::None).fields(CARDS),
    EdgeKind::new("..", "Dashed Link", Head::None, Head::None).dashed().fields(CARDS),
    EdgeKind::new("<|--", "Inheritance (from the end)", Head::Triangle, Head::None).fields(CARDS),
    EdgeKind::new("<|..", "Realization (from the end)", Head::Triangle, Head::None).dashed().fields(CARDS),
    EdgeKind::new("*--", "Composition (at the start)", Head::DiamondFilled, Head::None).fields(CARDS),
    EdgeKind::new("o--", "Aggregation (at the start)", Head::DiamondHollow, Head::None).fields(CARDS),
    EdgeKind::new("<--", "Association (to the start)", Head::Arrow, Head::None).fields(CARDS),
    EdgeKind::new("<..", "Dependency (to the start)", Head::Arrow, Head::None).dashed().fields(CARDS),
];

pub const CLASS: GraphSpec = GraphSpec {
    fields: &[],
    nodes: &[NodeKind::new("class", "Class", Look::Table).fields(CLASS_FIELDS).lines("Members (+public, -private, #protected; methods end with ())")],
    edges: CLASS_EDGES,
    edge_label: true,
};

fn class_head(k: ast::ClassRelationKind, left: bool) -> &'static str {
    use ast::ClassRelationKind as K;
    match (k, left) {
        (K::Inheritance | K::Realization, true) => "<|",
        (K::Inheritance | K::Realization, false) => "|>",
        (K::Composition, _) => "*",
        (K::Aggregation, _) => "o",
        (K::Association | K::Dependency, true) => "<",
        (K::Association | K::Dependency, false) => ">",
        _ => "",
    }
}

/// A relation's arrow as written (`<|--`, `*--o`, `()--`…).
fn class_op(r: &ast::ClassRelation) -> String {
    use ast::ClassRelationKind as K;
    let dashed = matches!(r.kind, K::LinkDashed | K::Dependency | K::Realization);
    let left = if r.reversed { class_head(r.kind, true) } else { "" };
    let right = match (r.to_kind, r.reversed) {
        (Some(k), _) => class_head(k, false),
        (None, false) => class_head(r.kind, false),
        (None, true) => "",
    };
    format!("{}{left}{}{right}{}", if r.lollipop_from { "()" } else { "" }, if dashed { ".." } else { "--" }, if r.lollipop_to { "()" } else { "" })
}

const VIS: &[(ast::Visibility, char)] =
    &[(ast::Visibility::Public, '+'), (ast::Visibility::Private, '-'), (ast::Visibility::Protected, '#'), (ast::Visibility::Package, '~')];

pub fn class_read(d: &ast::ClassDiagram, src: &str) -> Graph {
    let mut g = Graph::default();
    for c in &d.classes {
        let mut n = Node { id: c.name.clone(), kind: "class".into(), text: c.name.clone(), ..Default::default() };
        set(&mut n.fields, "label", c.label.clone().unwrap_or_default());
        set(&mut n.fields, "stereotype", c.stereotype.clone().unwrap_or_default());
        n.lines = c
            .members
            .iter()
            .map(|m| {
                let v = VIS.iter().find(|(v, _)| *v == m.visibility).map(|(_, c)| c.to_string()).unwrap_or_default();
                format!("{v}{}", m.text)
            })
            .collect();
        g.nodes.push(n);
    }
    for r in &d.relations {
        if let (Some(from), Some(to)) = (g.index(&r.from), g.index(&r.to)) {
            let mut e = crate::graph::Edge { from, to, kind: class_op(r), label: r.label.clone().unwrap_or_default(), ..Default::default() };
            set(&mut e.fields, "fromCard", r.from_card.clone().unwrap_or_default());
            set(&mut e.fields, "toCard", r.to_card.clone().unwrap_or_default());
            g.edges.push(e);
        }
    }
    g.restore_positions(src, flow_dir(d.direction));
    g
}

/// A class name as written: the text when it is a name Mermaid reads (letters, digits, `_`,
/// `~` for generics), else made into one.
fn class_name(n: &Node) -> String {
    let t = n.text.trim();
    if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '~') { t.to_string() } else { ident(t, "C") }
}

pub fn class_write(g: &Graph) -> String {
    let mut out = vec!["classDiagram".to_string()];
    let dir = dir_of(g);
    if dir != "TB" {
        out.push(format!("    direction {dir}"));
    }
    for n in &g.nodes {
        let label = get(&n.fields, "label");
        let head = if label.is_empty() { format!("    class {}", class_name(n)) } else { format!("    class {}[{}]", class_name(n), quoted(label)) };
        let st = get(&n.fields, "stereotype");
        let lines: Vec<String> = n.lines.iter().map(|l| one_line(l)).filter(|l| !l.is_empty()).collect();
        if st.is_empty() && lines.is_empty() {
            out.push(head);
            continue;
        }
        out.push(format!("{head} {{"));
        if !st.is_empty() {
            out.push(format!("        <<{}>>", one_line(st)));
        }
        for l in lines {
            out.push(format!("        {l}"));
        }
        out.push("    }".into());
    }
    for e in &g.edges {
        let (Some(a), Some(b)) = (g.nodes.get(e.from), g.nodes.get(e.to)) else { continue };
        let card = |k: &str| {
            let c = get(&e.fields, k);
            if c.is_empty() { String::new() } else { format!(" {}", quoted(c)) }
        };
        let label = one_line(&e.label);
        let op = if e.kind.is_empty() { "-->" } else { e.kind.as_str() };
        out.push(format!(
            "    {}{} {op}{} {}{}",
            class_name(a),
            card("fromCard"),
            card("toCard"),
            class_name(b),
            if label.is_empty() { String::new() } else { format!(" : {label}") }
        ));
    }
    out.join("\n")
}

// ---- entity relationship ---------------------------------------------------

const CARD_CHOICES: &[(&str, &str)] = &[("one", "Exactly one"), ("zeroOne", "Zero or one"), ("many", "One or more"), ("zeroMany", "Zero or more")];
const ER_FIELDS: &[FieldSpec] = &[choice("fromCard", "Start Cardinality", CARD_CHOICES), choice("toCard", "End Cardinality", CARD_CHOICES)];

pub const ER: GraphSpec = GraphSpec {
    fields: &[],
    nodes: &[NodeKind::new("entity", "Entity", Look::Table).lines("Attributes (type name PK \"comment\")")],
    edges: &[
        EdgeKind::new("identifying", "Identifying Relationship", Head::None, Head::None).fields(ER_FIELDS),
        EdgeKind::new("nonIdentifying", "Non-identifying Relationship", Head::None, Head::None).dashed().fields(ER_FIELDS),
    ],
    edge_label: true,
};

fn card_id(c: ast::Cardinality) -> &'static str {
    match c {
        ast::Cardinality::ZeroOrOne => "zeroOne",
        ast::Cardinality::OneOrMore => "many",
        ast::Cardinality::ZeroOrMore => "zeroMany",
        _ => "one",
    }
}

pub fn er_read(d: &ast::ErDiagram, src: &str) -> Graph {
    let mut g = Graph::default();
    for en in &d.entities {
        let lines = en
            .attributes
            .iter()
            .map(|a| {
                let mut l = format!("{} {}", a.type_, a.name);
                if let Some(k) = &a.key {
                    l.push(' ');
                    l.push_str(&k.replace(' ', ""));
                }
                if let Some(c) = &a.comment {
                    l.push_str(&format!(" \"{c}\""));
                }
                l
            })
            .collect();
        g.nodes.push(Node { id: en.name.clone(), kind: "entity".into(), text: en.label.clone(), lines, ..Default::default() });
    }
    for r in &d.relations {
        if let (Some(from), Some(to)) = (g.index(&r.left), g.index(&r.right)) {
            let kind = if r.identifying { "identifying" } else { "nonIdentifying" };
            let mut e = crate::graph::Edge { from, to, kind: kind.into(), label: r.label.clone(), ..Default::default() };
            set(&mut e.fields, "fromCard", card_id(r.left_card));
            set(&mut e.fields, "toCard", card_id(r.right_card));
            g.edges.push(e);
        }
    }
    g.restore_positions(src, flow_dir(d.direction));
    g
}

fn er_name(n: &Node) -> String {
    if n.id.is_empty() { ident(&n.text, "E") } else { n.id.clone() }
}

pub fn er_write(g: &Graph) -> String {
    let mut out = vec!["erDiagram".to_string()];
    let dir = dir_of(g);
    if dir != "TB" {
        out.push(format!("    direction {dir}"));
    }
    for n in &g.nodes {
        let name = er_name(n);
        let head = if n.text.is_empty() || n.text == name { name.clone() } else { format!("{name}[{}]", quoted(&n.text)) };
        let lines: Vec<String> = n.lines.iter().map(|l| one_line(l)).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            out.push(format!("    {head}"));
        } else {
            out.push(format!("    {head} {{"));
            for l in lines {
                out.push(format!("        {l}"));
            }
            out.push("    }".into());
        }
    }
    for e in &g.edges {
        let (Some(a), Some(b)) = (g.nodes.get(e.from), g.nodes.get(e.to)) else { continue };
        let left = match get(&e.fields, "fromCard") {
            "zeroOne" => "|o",
            "many" => "}|",
            "zeroMany" => "}o",
            _ => "||",
        };
        let right = match get(&e.fields, "toCard") {
            "zeroOne" => "o|",
            "many" => "|{",
            "zeroMany" => "o{",
            _ => "||",
        };
        let line = if e.kind == "nonIdentifying" { ".." } else { "--" };
        out.push(format!("    {} {left}{line}{right} {} : {}", er_name(a), er_name(b), quoted(&e.label)));
    }
    out.join("\n")
}

// ---- requirement -----------------------------------------------------------

const RISKS: &[(&str, &str)] = &[("", "None"), ("low", "Low"), ("medium", "Medium"), ("high", "High")];
const VERIFY: &[(&str, &str)] =
    &[("", "None"), ("analysis", "Analysis"), ("inspection", "Inspection"), ("test", "Test"), ("demonstration", "Demonstration")];
const REQ_FIELDS: &[FieldSpec] =
    &[text("id", "Id"), text("text", "Text"), choice("risk", "Risk", RISKS), choice("verifymethod", "Verification", VERIFY)];
const ELEMENT_FIELDS: &[FieldSpec] = &[text("type", "Type"), text("docref", "Document")];

const REQ_KINDS: &[(ast::RequirementKind, &str)] = &[
    (ast::RequirementKind::Requirement, "requirement"),
    (ast::RequirementKind::Functional, "functionalRequirement"),
    (ast::RequirementKind::Interface, "interfaceRequirement"),
    (ast::RequirementKind::Performance, "performanceRequirement"),
    (ast::RequirementKind::Physical, "physicalRequirement"),
    (ast::RequirementKind::DesignConstraint, "designConstraint"),
];

const REL_KINDS: &[(ast::ReqRelationKind, &str)] = &[
    (ast::ReqRelationKind::Contains, "contains"),
    (ast::ReqRelationKind::Copies, "copies"),
    (ast::ReqRelationKind::Derives, "derives"),
    (ast::ReqRelationKind::Satisfies, "satisfies"),
    (ast::ReqRelationKind::Verifies, "verifies"),
    (ast::ReqRelationKind::Refines, "refines"),
    (ast::ReqRelationKind::Traces, "traces"),
];

pub const REQUIREMENT: GraphSpec = GraphSpec {
    fields: &[],
    nodes: &[
        NodeKind::new("requirement", "Requirement", Look::Table).fields(REQ_FIELDS),
        NodeKind::new("functionalRequirement", "Functional Requirement", Look::Table).fields(REQ_FIELDS),
        NodeKind::new("interfaceRequirement", "Interface Requirement", Look::Table).fields(REQ_FIELDS),
        NodeKind::new("performanceRequirement", "Performance Requirement", Look::Table).fields(REQ_FIELDS),
        NodeKind::new("physicalRequirement", "Physical Requirement", Look::Table).fields(REQ_FIELDS),
        NodeKind::new("designConstraint", "Design Constraint", Look::Table).fields(REQ_FIELDS),
        NodeKind::new("element", "Element", Look::Rect).fields(ELEMENT_FIELDS),
    ],
    edges: &[
        EdgeKind::new("contains", "Contains", Head::None, Head::Circle),
        EdgeKind::new("copies", "Copies", Head::None, Head::Arrow).dashed(),
        EdgeKind::new("derives", "Derives", Head::None, Head::Arrow).dashed(),
        EdgeKind::new("satisfies", "Satisfies", Head::None, Head::Arrow).dashed(),
        EdgeKind::new("verifies", "Verifies", Head::None, Head::Arrow).dashed(),
        EdgeKind::new("refines", "Refines", Head::None, Head::Arrow).dashed(),
        EdgeKind::new("traces", "Traces", Head::None, Head::Arrow).dashed(),
    ],
    edge_label: false,
};

pub fn requirement_read(d: &ast::RequirementDiagram, src: &str) -> Graph {
    let mut g = Graph::default();
    for r in &d.requirements {
        let kind = REQ_KINDS.iter().find(|(k, _)| *k == r.kind).map_or("requirement", |(_, id)| id);
        let mut n = Node { id: r.name.clone(), kind: kind.into(), text: r.name.clone(), ..Default::default() };
        set(&mut n.fields, "id", r.id.clone().unwrap_or_default());
        set(&mut n.fields, "text", r.text.clone().unwrap_or_default());
        set(&mut n.fields, "risk", r.risk.clone().unwrap_or_default());
        set(&mut n.fields, "verifymethod", r.verifymethod.clone().unwrap_or_default());
        g.nodes.push(n);
    }
    for el in &d.elements {
        let mut n = Node { id: el.name.clone(), kind: "element".into(), text: el.name.clone(), ..Default::default() };
        set(&mut n.fields, "type", el.type_.clone().unwrap_or_default());
        set(&mut n.fields, "docref", el.docref.clone().unwrap_or_default());
        g.nodes.push(n);
    }
    for r in &d.relations {
        if let (Some(from), Some(to)) = (g.index(&r.from), g.index(&r.to)) {
            let kind = REL_KINDS.iter().find(|(k, _)| *k == r.kind).map_or("traces", |(_, id)| id);
            g.edges.push(crate::graph::Edge { from, to, kind: kind.into(), ..Default::default() });
        }
    }
    // Positions are keyed by the requirement's name.
    for n in &mut g.nodes {
        n.id = ident(&n.text, "r");
    }
    g.restore_positions(src, flow_dir(d.direction));
    g
}

/// A requirement's name as written: bare when it is an identifier, else quoted.
fn req_name(n: &Node) -> String {
    let t = one_line(&n.text);
    if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_') { t } else { quoted(&t) }
}

pub fn requirement_write(g: &Graph) -> String {
    let mut out = vec!["requirementDiagram".to_string()];
    let dir = dir_of(g);
    if dir != "TB" {
        out.push(format!("    direction {dir}"));
    }
    for n in &g.nodes {
        let keys: &[&str] = if n.kind == "element" { &["type", "docref"] } else { &["id", "text", "risk", "verifymethod"] };
        out.push(format!("    {} {} {{", if n.kind.is_empty() { "requirement" } else { n.kind.as_str() }, req_name(n)));
        for k in keys {
            let v = one_line(get(&n.fields, k));
            if !v.is_empty() {
                out.push(format!("        {k}: {v}"));
            }
        }
        out.push("    }".into());
    }
    for e in &g.edges {
        let (Some(a), Some(b)) = (g.nodes.get(e.from), g.nodes.get(e.to)) else { continue };
        out.push(format!("    {} - {} -> {}", req_name(a), if e.kind.is_empty() { "traces" } else { &e.kind }, req_name(b)));
    }
    out.join("\n")
}

/// The positions of a requirement diagram's shapes are keyed by their name made an id.
pub fn requirement_positions(g: &Graph) -> Vec<String> {
    let mut g = g.clone();
    for n in &mut g.nodes {
        n.id = ident(&n.text, "r");
    }
    g.positions()
}

// ---- C4 ---------------------------------------------------------------------

const C4_KINDS: &[(&str, &str)] = &[
    ("C4Context", "System Context"),
    ("C4Container", "Container"),
    ("C4Component", "Component"),
    ("C4Dynamic", "Dynamic"),
    ("C4Deployment", "Deployment"),
];
const C4_DIAGRAM: &[FieldSpec] = &[choice("kind", "Diagram Level", C4_KINDS), text("title", "Title")];
const C4_PERSON: &[FieldSpec] = &[text("descr", "Description"), flag("external", "External")];
const C4_TECH: &[FieldSpec] = &[text("technology", "Technology"), text("descr", "Description"), flag("external", "External")];
const C4_NODE: &[FieldSpec] = &[text("technology", "Type"), text("descr", "Description")];
const C4_DIRS: &[(&str, &str)] = &[("", "Automatic"), ("U", "Up"), ("D", "Down"), ("L", "Left"), ("R", "Right")];
const C4_REL: &[FieldSpec] = &[text("technology", "Technology"), choice("direction", "Direction", C4_DIRS), flag("both", "Both Ways")];

/// Element kinds: canvas id, Mermaid macro, AST kind.
const C4_ELEMENTS: &[(&str, &str, ast::C4ElementKind)] = &[
    ("person", "Person", ast::C4ElementKind::Person),
    ("system", "System", ast::C4ElementKind::System),
    ("systemDb", "SystemDb", ast::C4ElementKind::SystemDb),
    ("systemQueue", "SystemQueue", ast::C4ElementKind::SystemQueue),
    ("container", "Container", ast::C4ElementKind::Container),
    ("containerDb", "ContainerDb", ast::C4ElementKind::ContainerDb),
    ("containerQueue", "ContainerQueue", ast::C4ElementKind::ContainerQueue),
    ("component", "Component", ast::C4ElementKind::Component),
    ("componentDb", "ComponentDb", ast::C4ElementKind::ComponentDb),
    ("componentQueue", "ComponentQueue", ast::C4ElementKind::ComponentQueue),
];

const C4_BOUNDARIES: &[(&str, &str, ast::C4BoundaryKind)] = &[
    ("boundary", "Boundary", ast::C4BoundaryKind::Generic),
    ("systemBoundary", "System_Boundary", ast::C4BoundaryKind::System),
    ("containerBoundary", "Container_Boundary", ast::C4BoundaryKind::Container),
    ("enterpriseBoundary", "Enterprise_Boundary", ast::C4BoundaryKind::Enterprise),
    ("node", "Deployment_Node", ast::C4BoundaryKind::Deployment),
];

pub const C4: GraphSpec = GraphSpec {
    fields: C4_DIAGRAM,
    nodes: &[
        NodeKind::new("person", "Person", Look::Person).fields(C4_PERSON),
        NodeKind::new("system", "Software System", Look::Rounded).fields(C4_PERSON),
        NodeKind::new("systemDb", "System Database", Look::Cylinder).fields(C4_PERSON),
        NodeKind::new("systemQueue", "System Queue", Look::Stadium).fields(C4_PERSON),
        NodeKind::new("container", "Container", Look::Rounded).fields(C4_TECH),
        NodeKind::new("containerDb", "Container Database", Look::Cylinder).fields(C4_TECH),
        NodeKind::new("containerQueue", "Container Queue", Look::Stadium).fields(C4_TECH),
        NodeKind::new("component", "Component", Look::Rect).fields(C4_TECH),
        NodeKind::new("componentDb", "Component Database", Look::Cylinder).fields(C4_TECH),
        NodeKind::new("componentQueue", "Component Queue", Look::Stadium).fields(C4_TECH),
        NodeKind::new("boundary", "Boundary", Look::Group).group(),
        NodeKind::new("systemBoundary", "System Boundary", Look::Group).group(),
        NodeKind::new("containerBoundary", "Container Boundary", Look::Group).group(),
        NodeKind::new("enterpriseBoundary", "Enterprise Boundary", Look::Group).group(),
        NodeKind::new("node", "Deployment Node", Look::Group).fields(C4_NODE).group(),
    ],
    edges: &[EdgeKind::new("rel", "Relationship", Head::None, Head::Arrow).dashed().fields(C4_REL)],
    edge_label: true,
};

fn c4_read_elements(els: &[ast::C4Element], parent: &str, g: &mut Graph, depth: usize) {
    if depth > 16 {
        return;
    }
    for e in els {
        let kind = match e.boundary_kind {
            Some(b) => C4_BOUNDARIES.iter().find(|(_, _, k)| *k == b).map_or("boundary", |(id, _, _)| id),
            None => C4_ELEMENTS.iter().find(|(_, _, k)| *k == e.kind).map_or("system", |(id, _, _)| id),
        };
        let mut n = Node { id: e.alias.clone(), kind: kind.into(), text: e.label.clone(), parent: parent.to_string(), ..Default::default() };
        set(&mut n.fields, "descr", e.descr.clone().unwrap_or_default());
        set(&mut n.fields, "technology", e.technology.clone().unwrap_or_default());
        if e.external {
            set(&mut n.fields, "external", "true");
        }
        g.nodes.push(n);
        c4_read_elements(&e.members, &e.alias, g, depth + 1);
    }
}

pub fn c4_read(d: &ast::C4Diagram, src: &str) -> Graph {
    let mut g = Graph::default();
    let kind = match d.kind {
        ast::C4Kind::Container => "C4Container",
        ast::C4Kind::Component => "C4Component",
        ast::C4Kind::Dynamic => "C4Dynamic",
        ast::C4Kind::Deployment => "C4Deployment",
        _ => "C4Context",
    };
    set(&mut g.fields, "kind", kind);
    set(&mut g.fields, "title", d.title.clone().unwrap_or_default());
    c4_read_elements(&d.elements, "", &mut g, 0);
    for r in &d.relations {
        if let (Some(from), Some(to)) = (g.index(&r.from), g.index(&r.to)) {
            let mut e = crate::graph::Edge { from, to, kind: "rel".into(), label: r.label.clone(), ..Default::default() };
            set(&mut e.fields, "technology", r.technology.clone().unwrap_or_default());
            let dir = match r.direction {
                ast::C4RelDirection::Up => "U",
                ast::C4RelDirection::Down => "D",
                ast::C4RelDirection::Left => "L",
                ast::C4RelDirection::Right => "R",
                _ => "",
            };
            set(&mut e.fields, "direction", dir);
            if r.bidirectional {
                set(&mut e.fields, "both", "true");
            }
            g.edges.push(e);
        }
    }
    g.restore_positions(src, true);
    g
}

/// `"text"` as a C4 macro argument.
fn arg(s: &str) -> String {
    format!("\"{}\"", one_line(s).replace('"', "'"))
}

fn c4_write_level(g: &Graph, parent: &str, indent: usize, out: &mut Vec<String>, depth: usize) {
    if depth > 16 {
        return;
    }
    let pad = " ".repeat(indent);
    for n in g.nodes.iter().filter(|n| n.parent == parent) {
        let alias = if n.id.is_empty() { ident(&n.text, "e") } else { n.id.clone() };
        if let Some((_, mac, _)) = C4_BOUNDARIES.iter().find(|(id, _, _)| *id == n.kind) {
            let mut args = vec![alias.clone(), arg(&n.text)];
            if n.kind == "node" {
                for k in ["technology", "descr"] {
                    let v = get(&n.fields, k);
                    if !v.is_empty() {
                        args.push(arg(v));
                    }
                }
            }
            out.push(format!("{pad}{mac}({}) {{", args.join(", ")));
            c4_write_level(g, &n.id, indent + 4, out, depth + 1);
            out.push(format!("{pad}}}"));
            continue;
        }
        let (_, mac, _) = C4_ELEMENTS.iter().find(|(id, _, _)| *id == n.kind).copied().unwrap_or(("system", "System", ast::C4ElementKind::System));
        let ext = if is(&n.fields, "external") { "_Ext" } else { "" };
        let mut args = vec![alias, arg(&n.text)];
        let tech = get(&n.fields, "technology");
        let descr = get(&n.fields, "descr");
        let has_tech = !matches!(mac, "Person" | "System" | "SystemDb" | "SystemQueue");
        if has_tech && (!tech.is_empty() || !descr.is_empty()) {
            args.push(arg(tech));
        }
        if !descr.is_empty() {
            args.push(arg(descr));
        }
        out.push(format!("{pad}{mac}{ext}({})", args.join(", ")));
    }
}

pub fn c4_write(g: &Graph) -> String {
    let kind = match get(&g.fields, "kind") {
        k @ ("C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment") => k,
        _ => "C4Context",
    };
    let mut out = vec![kind.to_string()];
    let title = one_line(get(&g.fields, "title"));
    if !title.is_empty() {
        out.push(format!("    title {title}"));
    }
    c4_write_level(g, "", 4, &mut out, 0);
    for e in &g.edges {
        let (Some(a), Some(b)) = (g.nodes.get(e.from), g.nodes.get(e.to)) else { continue };
        let mac = match (is(&e.fields, "both"), get(&e.fields, "direction")) {
            (true, _) => "BiRel".to_string(),
            (false, d @ ("U" | "D" | "L" | "R")) => format!("Rel_{d}"),
            _ => "Rel".to_string(),
        };
        let mut args = vec![a.id.clone(), b.id.clone(), arg(&e.label)];
        let tech = get(&e.fields, "technology");
        if !tech.is_empty() {
            args.push(arg(tech));
        }
        out.push(format!("    {mac}({})", args.join(", ")));
    }
    out.join("\n")
}

// ---- architecture ------------------------------------------------------------

const ICONS: &[(&str, &str)] =
    &[("", "None"), ("cloud", "Cloud"), ("database", "Database"), ("disk", "Disk"), ("internet", "Internet"), ("server", "Server")];
const ARCH_ICON: &[FieldSpec] = &[FieldSpec { key: "icon", label: "Icon", ty: Choice(ICONS) }];
const SIDES: &[(&str, &str)] = &[("", "Automatic"), ("L", "Left"), ("R", "Right"), ("T", "Top"), ("B", "Bottom")];
const ARCH_EDGE: &[FieldSpec] = &[
    choice("fromSide", "Start Side", SIDES),
    choice("toSide", "End Side", SIDES),
    flag("fromArrow", "Arrow at the Start"),
    flag("toArrow", "Arrow at the End"),
    flag("fromGroup", "From the Start's Group"),
    flag("toGroup", "To the End's Group"),
];

pub const ARCHITECTURE: GraphSpec = GraphSpec {
    fields: &[],
    nodes: &[
        NodeKind::new("service", "Service", Look::Rect).fields(ARCH_ICON),
        NodeKind::new("group", "Group", Look::Group).fields(ARCH_ICON).group(),
        NodeKind::new("junction", "Junction", Look::Junction).bare(),
    ],
    edges: &[EdgeKind::new("edge", "Connection", Head::None, Head::None).fields(ARCH_EDGE)],
    edge_label: true,
};

fn side(s: ast::ArchSide) -> &'static str {
    match s {
        ast::ArchSide::Top => "T",
        ast::ArchSide::Bottom => "B",
        ast::ArchSide::Left => "L",
        _ => "R",
    }
}

pub fn architecture_read(d: &ast::ArchitectureDiagram, src: &str) -> Graph {
    let mut g = Graph::default();
    for gr in &d.groups {
        let mut n = Node {
            id: gr.id.clone(),
            kind: "group".into(),
            text: gr.label.clone().unwrap_or_default(),
            parent: gr.parent.clone().unwrap_or_default(),
            ..Default::default()
        };
        set(&mut n.fields, "icon", gr.icon.clone().unwrap_or_default());
        g.nodes.push(n);
    }
    for s in &d.services {
        let mut n = Node {
            id: s.id.clone(),
            kind: "service".into(),
            text: s.label.clone().unwrap_or_default(),
            parent: s.parent.clone().unwrap_or_default(),
            ..Default::default()
        };
        set(&mut n.fields, "icon", s.icon.clone().unwrap_or_default());
        g.nodes.push(n);
    }
    for j in &d.junctions {
        g.nodes.push(Node { id: j.id.clone(), kind: "junction".into(), parent: j.parent.clone().unwrap_or_default(), ..Default::default() });
    }
    for a in &d.edges {
        if let (Some(from), Some(to)) = (g.index(&a.from), g.index(&a.to)) {
            let mut e = crate::graph::Edge { from, to, kind: "edge".into(), label: a.label.clone().unwrap_or_default(), ..Default::default() };
            set(&mut e.fields, "fromSide", side(a.from_side));
            set(&mut e.fields, "toSide", side(a.to_side));
            for (k, on) in [
                ("fromArrow", a.from_arrow),
                ("toArrow", a.to_arrow),
                ("fromGroup", a.group && g.nodes.get(from).is_some_and(|n| !n.parent.is_empty())),
            ] {
                if on {
                    set(&mut e.fields, k, "true");
                }
            }
            g.edges.push(e);
        }
    }
    g.restore_positions(src, true);
    g
}

/// The side of `a` facing `b` on the canvas.
fn facing(a: &Node, b: &Node) -> &'static str {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { "R" } else { "L" }
    } else if dy >= 0.0 {
        "B"
    } else {
        "T"
    }
}

pub fn architecture_write(g: &Graph) -> String {
    let mut out = vec!["architecture-beta".to_string()];
    let id = |n: &Node| if n.id.is_empty() { ident(&n.text, "a") } else { n.id.clone() };
    let tail = |n: &Node| {
        let icon = get(&n.fields, "icon");
        let mut s = String::new();
        if !icon.is_empty() {
            s.push_str(&format!("({})", ident(icon, "i")));
        }
        let t = one_line(&n.text).replace([']', '['], "");
        if !t.is_empty() {
            s.push_str(&format!("[{t}]"));
        }
        if !n.parent.is_empty() {
            s.push_str(&format!(" in {}", n.parent));
        }
        s
    };
    for kind in ["group", "service", "junction"] {
        for n in g.nodes.iter().filter(|n| n.kind == kind) {
            if kind == "junction" {
                let parent = if n.parent.is_empty() { String::new() } else { format!(" in {}", n.parent) };
                out.push(format!("    junction {}{parent}", id(n)));
            } else {
                out.push(format!("    {kind} {}{}", id(n), tail(n)));
            }
        }
    }
    for e in &g.edges {
        let (Some(a), Some(b)) = (g.nodes.get(e.from), g.nodes.get(e.to)) else { continue };
        let pick = |k: &str, auto: &'static str| match get(&e.fields, k) {
            s @ ("L" | "R" | "T" | "B") => s.to_string(),
            _ => auto.to_string(),
        };
        let (fs, ts) = (pick("fromSide", facing(a, b)), pick("toSide", facing(b, a)));
        let grp = |k: &str| if is(&e.fields, k) { "{group}" } else { "" };
        let label = one_line(&e.label).replace([']', '['], "");
        let mid = if label.is_empty() { "--".to_string() } else { format!("-[{label}]-") };
        out.push(format!(
            "    {}{}:{fs} {}{mid}{} {ts}:{}{}",
            id(a),
            grp("fromGroup"),
            if is(&e.fields, "fromArrow") { "<" } else { "" },
            if is(&e.fields, "toArrow") { ">" } else { "" },
            id(b),
            grp("toGroup"),
        ));
    }
    out.join("\n")
}

// ---- block ---------------------------------------------------------------

/// Canvas cell of a block diagram, points.
pub const BLOCK_W: f32 = 150.0;
pub const BLOCK_H: f32 = 90.0;

const SPAN: &[FieldSpec] = &[FieldSpec { key: "span", label: "Columns Wide", ty: crate::form::FieldType::Number }];
const BLOCK_SHAPES: &[(&str, ast::BlockShape, &str, &str)] = &[
    ("rectangle", ast::BlockShape::Rect, "[", "]"),
    ("rounded", ast::BlockShape::Round, "(", ")"),
    ("stadium", ast::BlockShape::Stadium, "([", "])"),
    ("database", ast::BlockShape::Cylinder, "[(", ")]"),
    ("circle", ast::BlockShape::Circle, "((", "))"),
    ("diamond", ast::BlockShape::Rhombus, "{", "}"),
    ("hexagon", ast::BlockShape::Hexagon, "{{", "}}"),
    ("subroutine", ast::BlockShape::Subroutine, "[[", "]]"),
    ("doubleCircle", ast::BlockShape::DoubleCircle, "(((", ")))"),
    ("flag", ast::BlockShape::Odd, ">", "]"),
    ("parallelogram", ast::BlockShape::LeanRight, "[/", "/]"),
    ("parallelogramAlt", ast::BlockShape::LeanLeft, "[\\", "\\]"),
    ("trapezoid", ast::BlockShape::Trapezoid, "[/", "\\]"),
    ("trapezoidAlt", ast::BlockShape::TrapezoidAlt, "[\\", "/]"),
];

const BLOCK_NODES: &[NodeKind] = &[
    NodeKind::new("rectangle", "Rectangle", Look::Rect).fields(SPAN),
    NodeKind::new("rounded", "Rounded Rectangle", Look::Rounded).fields(SPAN),
    NodeKind::new("stadium", "Terminator", Look::Stadium).fields(SPAN),
    NodeKind::new("diamond", "Decision", Look::Diamond).fields(SPAN),
    NodeKind::new("parallelogram", "Data", Look::Parallelogram).fields(SPAN),
    NodeKind::new("parallelogramAlt", "Data (Left)", Look::ParallelogramAlt).fields(SPAN),
    NodeKind::new("subroutine", "Predefined Process", Look::Subroutine).fields(SPAN),
    NodeKind::new("hexagon", "Preparation", Look::Hexagon).fields(SPAN),
    NodeKind::new("trapezoid", "Manual Operation", Look::Trapezoid).fields(SPAN),
    NodeKind::new("trapezoidAlt", "Trapezoid (Inverted)", Look::TrapezoidAlt).fields(SPAN),
    NodeKind::new("database", "Database", Look::Cylinder).fields(SPAN),
    NodeKind::new("circle", "Circle", Look::Circle).fields(SPAN),
    NodeKind::new("doubleCircle", "Double Circle", Look::DoubleCircle).fields(SPAN),
    NodeKind::new("flag", "Flag", Look::Flag).fields(SPAN),
];

pub const BLOCK: GraphSpec = GraphSpec {
    fields: &[FieldSpec { key: "columns", label: "Columns", ty: crate::form::FieldType::Number }],
    nodes: BLOCK_NODES,
    edges: LINKS,
    edge_label: true,
};

/// A block's columns.
fn span_of(n: &Node) -> usize {
    get(&n.fields, "span").trim().parse::<usize>().unwrap_or(1).clamp(1, 64)
}

pub fn block_read(d: &ast::BlockDiagram) -> Graph {
    let mut g = Graph::default();
    if let Some(c) = d.columns {
        set(&mut g.fields, "columns", c.min(64).to_string());
    }
    let cols = d.columns.unwrap_or(usize::MAX).max(1);
    let (mut row, mut col) = (0usize, 0usize);
    let advance = |n: usize, row: &mut usize, col: &mut usize| {
        if *col + n > cols && *col > 0 {
            *row += 1;
            *col = 0;
        }
        let at = (*row, *col);
        *col += n;
        if *col >= cols {
            *row += 1;
            *col = 0;
        }
        at
    };
    for it in &d.items {
        match it {
            ast::BlockItem::Block(b) => {
                let span = b.span.clamp(1, 64);
                let (r, c) = advance(span, &mut row, &mut col);
                let kind = BLOCK_SHAPES.iter().find(|(_, s, _, _)| *s == b.shape).map_or("rectangle", |(id, ..)| id);
                let mut n = Node {
                    id: b.id.clone(),
                    kind: kind.into(),
                    text: b.label.clone(),
                    x: c as f32 * BLOCK_W + span as f32 * BLOCK_W / 2.0,
                    y: r as f32 * BLOCK_H + BLOCK_H / 2.0,
                    ..Default::default()
                };
                if span > 1 {
                    set(&mut n.fields, "span", span.to_string());
                }
                g.nodes.push(n);
            }
            ast::BlockItem::Space(n) => {
                advance((*n).clamp(1, 64), &mut row, &mut col);
            }
            _ => {}
        }
    }
    for it in &d.items {
        if let ast::BlockItem::Edge(e) = it
            && let (Some(from), Some(to)) = (g.index(&e.from), g.index(&e.to))
        {
            let arrow = !matches!(e.head, ast::EdgeHead::None);
            let style = match e.style {
                ast::BlockLinkStyle::Dotted => LineStyle::Dotted,
                ast::BlockLinkStyle::Thick => LineStyle::Thick,
                _ => LineStyle::Solid,
            };
            g.edges.push(crate::graph::Edge {
                from,
                to,
                kind: link_kind(style, arrow).into(),
                label: e.label.clone().unwrap_or_default(),
                ..Default::default()
            });
        }
    }
    g
}

pub fn block_write(g: &Graph) -> String {
    let mut out = vec!["block-beta".to_string()];
    // Shapes in reading order, each in its cell of the canvas grid.
    let mut cells: Vec<(usize, usize, &Node)> = g
        .nodes
        .iter()
        .map(|n| {
            let span = span_of(n) as f32;
            let left = (n.x - span * BLOCK_W / 2.0) / BLOCK_W;
            (((n.y / BLOCK_H) - 0.5).round().max(0.0) as usize, left.round().max(0.0) as usize, n)
        })
        .collect();
    cells.sort_by_key(|(r, c, _)| (*r, *c));
    let widest = cells.iter().map(|(_, c, n)| c + span_of(n)).max().unwrap_or(1);
    let given = get(&g.fields, "columns").trim().parse::<usize>().ok().filter(|c| *c >= 1);
    let rows = cells.iter().map(|(r, _, _)| *r).max().unwrap_or(0) + 1;
    let cols = given.unwrap_or(if rows > 1 { widest } else { usize::MAX });
    if cols != usize::MAX {
        out.push(format!("    columns {}", cols.min(64)));
    }
    let mut items: Vec<String> = Vec::new();
    let (mut row, mut col) = (0usize, 0usize);
    for (r, c, n) in &cells {
        let span = span_of(n);
        // Spaces up to the cell: the rest of each row, then the start of this one.
        while row < *r && cols != usize::MAX {
            if col > 0 && col < cols {
                items.push(if cols - col == 1 { "space".into() } else { format!("space:{}", cols - col) });
            }
            row += 1;
            col = 0;
        }
        if *c > col {
            let gap = (*c - col).min(cols.saturating_sub(col).max(1));
            items.push(if gap == 1 { "space".into() } else { format!("space:{gap}") });
            col += gap;
        }
        let (_, _, o, cl) = BLOCK_SHAPES.iter().find(|(id, ..)| *id == n.kind).copied().unwrap_or(BLOCK_SHAPES[0]);
        let id = if n.id.is_empty() { ident(&n.text, "b") } else { n.id.clone() };
        let mut item = if n.text.is_empty() || (n.text == id && n.kind == "rectangle") { id } else { format!("{id}{o}{}{cl}", quoted(&n.text)) };
        if span > 1 {
            item.push_str(&format!(":{span}"));
        }
        items.push(item);
        col += span;
        if cols != usize::MAX && col >= cols {
            row += 1;
            col = 0;
        }
    }
    // A few items per line, as people write them.
    for chunk in items.chunks(6) {
        out.push(format!("    {}", chunk.join(" ")));
    }
    for e in &g.edges {
        let (Some(a), Some(b)) = (g.nodes.get(e.from), g.nodes.get(e.to)) else { continue };
        let (style, arrow) = link_style(&e.kind);
        let op = match (style, arrow) {
            (LineStyle::Solid, true) => "-->",
            (LineStyle::Solid, false) => "---",
            (LineStyle::Dotted, true) => "-.->",
            (LineStyle::Dotted, false) => "-.-",
            (LineStyle::Thick, true) => "==>",
            (LineStyle::Thick, false) => "===",
        };
        let label = one_line(&e.label);
        if label.is_empty() {
            out.push(format!("    {} {op} {}", a.id, b.id));
        } else {
            out.push(format!("    {} -- {} {op} {}", a.id, quoted(&label), b.id));
        }
    }
    out.join("\n")
}
