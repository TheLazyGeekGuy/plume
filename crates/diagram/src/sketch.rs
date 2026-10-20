//! Plume: a diagram drawn by hand, as Office draws one: shapes placed on a canvas, joined by
//! connectors. It is written as a Mermaid `flowchart` (the shapes are its nodes, the connectors
//! its links) and read back from one, so a drawn diagram stays text in the Markdown file:
//!
//! ```text
//! flowchart LR
//!     n1["Draft"]
//!     n2{"Reviewed?"}
//!     n1 --> n2
//!     n2 -->|"no"| n1
//!     %% plume n1 40 60
//!     %% plume n2 240 60
//! ```
//!
//! The `%% plume` comments (ignored by every Mermaid renderer) keep where each shape was put, so
//! the canvas opens as it was left; the direction follows the drawing. A flowchart written by
//! hand opens too, its shapes placed rank by rank. What a sketch can't show (subgraphs, `&`
//! chains, other diagram types) is left to the source editor; style lines are kept as written.

use serde::{Deserialize, Serialize};
use wordcraft_doc::InlineObject;
use wordcraft_doc::para::{GroupChild, ShapeKind};

/// Most shapes and links in a sketch.
pub const MAX_NODES: usize = 300;
pub const MAX_EDGES: usize = 600;
/// Longest text of a shape or link.
pub const MAX_TEXT: usize = 200;
/// Largest canvas coordinate.
const MAX_COORD: f32 = 20_000.0;

/// A shape's outline, and its Mermaid brackets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeShape {
    #[default]
    Rectangle,
    Rounded,
    Stadium,
    Circle,
    Diamond,
    Hexagon,
    Parallelogram,
    Database,
    Subroutine,
    DoubleCircle,
    /// A flag pointing right (`id>text]`).
    Flag,
    ParallelogramAlt,
    Trapezoid,
    TrapezoidAlt,
}

impl NodeShape {
    pub const ALL: [NodeShape; 14] = [
        NodeShape::Rectangle,
        NodeShape::Rounded,
        NodeShape::Stadium,
        NodeShape::Circle,
        NodeShape::Diamond,
        NodeShape::Hexagon,
        NodeShape::Parallelogram,
        NodeShape::Database,
        NodeShape::Subroutine,
        NodeShape::DoubleCircle,
        NodeShape::Flag,
        NodeShape::ParallelogramAlt,
        NodeShape::Trapezoid,
        NodeShape::TrapezoidAlt,
    ];

    /// The label the shape palette shows.
    pub fn label(self) -> &'static str {
        match self {
            NodeShape::Rectangle => "Rectangle",
            NodeShape::Rounded => "Rounded Rectangle",
            NodeShape::Stadium => "Terminator",
            NodeShape::Circle => "Circle",
            NodeShape::Diamond => "Decision",
            NodeShape::Hexagon => "Preparation",
            NodeShape::Parallelogram => "Data",
            NodeShape::Database => "Database",
            NodeShape::Subroutine => "Predefined Process",
            NodeShape::DoubleCircle => "Double Circle",
            NodeShape::Flag => "Flag",
            NodeShape::ParallelogramAlt => "Data (Left)",
            NodeShape::Trapezoid => "Manual Operation",
            NodeShape::TrapezoidAlt => "Trapezoid (Inverted)",
        }
    }

    /// The shape's id in Plume's canvas (its [`crate::graph::NodeKind`]).
    pub fn id(self) -> &'static str {
        match self {
            NodeShape::Rectangle => "rectangle",
            NodeShape::Rounded => "rounded",
            NodeShape::Stadium => "stadium",
            NodeShape::Circle => "circle",
            NodeShape::Diamond => "diamond",
            NodeShape::Hexagon => "hexagon",
            NodeShape::Parallelogram => "parallelogram",
            NodeShape::Database => "database",
            NodeShape::Subroutine => "subroutine",
            NodeShape::DoubleCircle => "doubleCircle",
            NodeShape::Flag => "flag",
            NodeShape::ParallelogramAlt => "parallelogramAlt",
            NodeShape::Trapezoid => "trapezoid",
            NodeShape::TrapezoidAlt => "trapezoidAlt",
        }
    }

    pub fn from_id(id: &str) -> NodeShape {
        NodeShape::ALL.into_iter().find(|s| s.id() == id).unwrap_or_default()
    }

    /// Opening and closing brackets, longest first so `((` is read before `(`.
    fn brackets(self) -> (&'static str, &'static str) {
        match self {
            NodeShape::Rectangle => ("[", "]"),
            NodeShape::Rounded => ("(", ")"),
            NodeShape::Stadium => ("([", "])"),
            NodeShape::Circle => ("((", "))"),
            NodeShape::Diamond => ("{", "}"),
            NodeShape::Hexagon => ("{{", "}}"),
            NodeShape::Parallelogram => ("[/", "/]"),
            NodeShape::Database => ("[(", ")]"),
            NodeShape::Subroutine => ("[[", "]]"),
            NodeShape::DoubleCircle => ("(((", ")))"),
            NodeShape::Flag => (">", "]"),
            NodeShape::ParallelogramAlt => ("[\\", "\\]"),
            NodeShape::Trapezoid => ("[/", "\\]"),
            NodeShape::TrapezoidAlt => ("[\\", "/]"),
        }
    }

    /// The shape a Word shape stands for in a flowchart.
    fn of(kind: ShapeKind) -> NodeShape {
        match kind {
            ShapeKind::RoundedRectangle => NodeShape::Rounded,
            ShapeKind::Ellipse | ShapeKind::FlowchartOr => NodeShape::Circle,
            ShapeKind::Diamond => NodeShape::Diamond,
            _ => NodeShape::Rectangle,
        }
    }
}

/// How a link is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineStyle {
    #[default]
    Solid,
    Dotted,
    Thick,
}

/// A shape on the canvas: `x`, `y` is its centre.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub text: String,
    pub shape: NodeShape,
    pub x: f32,
    pub y: f32,
}

/// A connector from shape `from` to shape `to` (indices in [`Sketch::nodes`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: String,
    /// An arrowhead at `to`.
    pub arrow: bool,
    pub line: LineStyle,
}

/// Which way a flowchart runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    #[default]
    LR,
    RL,
    TD,
    BT,
}

impl Dir {
    pub fn name(self) -> &'static str {
        match self {
            Dir::LR => "LR",
            Dir::RL => "RL",
            Dir::TD => "TD",
            Dir::BT => "BT",
        }
    }
    pub fn parse(s: &str) -> Option<Dir> {
        match s {
            "LR" => Some(Dir::LR),
            "RL" => Some(Dir::RL),
            "TD" | "TB" => Some(Dir::TD),
            "BT" => Some(Dir::BT),
            _ => None,
        }
    }
}

/// A drawn diagram.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Sketch {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// The direction read from the source; written only when the drawing doesn't say.
    pub dir: Dir,
    /// Style lines (`style`, `classDef`, `class`, `linkStyle`, `click`) kept as written.
    pub extra: Vec<String>,
}

/// `s` cut to the longest text, at a character boundary, on one line.
fn clip(s: &str) -> String {
    s.replace(['\r', '\n'], " ").chars().take(MAX_TEXT).collect()
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v.clamp(-MAX_COORD, MAX_COORD) } else { 0.0 }
}

/// Text in a Mermaid quoted label.
fn quote(s: &str) -> String {
    format!("\"{}\"", clip(s).replace('"', "#quot;"))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or(s);
    clip(&s.replace("#quot;", "\"").replace("<br>", " ").replace("<br/>", " "))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 60 && id.chars().all(|c| c.is_alphanumeric() || c == '_') && !matches!(id, "end" | "graph" | "subgraph")
}

impl Sketch {
    /// A new shape's id, unused by the others.
    pub fn new_id(&self) -> String {
        let mut k = self.nodes.len() + 1;
        loop {
            let id = format!("n{k}");
            if !self.nodes.iter().any(|n| n.id == id) {
                return id;
            }
            k += 1;
        }
    }

    /// Add a shape at `x`, `y`; its index, or `None` when the sketch is full.
    pub fn add(&mut self, shape: NodeShape, text: &str, x: f32, y: f32) -> Option<usize> {
        if self.nodes.len() >= MAX_NODES {
            return None;
        }
        let id = self.new_id();
        self.nodes.push(Node { id, text: clip(text), shape, x: finite(x), y: finite(y) });
        Some(self.nodes.len() - 1)
    }

    /// Join shape `from` to shape `to` with an arrow; `None` when it can't (same shape, out of
    /// range, already joined that way, or the sketch is full).
    pub fn connect(&mut self, from: usize, to: usize) -> Option<usize> {
        if from == to || from >= self.nodes.len() || to >= self.nodes.len() || self.edges.len() >= MAX_EDGES {
            return None;
        }
        if self.edges.iter().any(|e| e.from == from && e.to == to) {
            return None;
        }
        self.edges.push(Edge { from, to, arrow: true, ..Default::default() });
        Some(self.edges.len() - 1)
    }

    /// Remove shape `i` and its links.
    pub fn remove_node(&mut self, i: usize) {
        if i >= self.nodes.len() {
            return;
        }
        self.nodes.remove(i);
        self.edges.retain(|e| e.from != i && e.to != i);
        for e in &mut self.edges {
            if e.from > i {
                e.from -= 1;
            }
            if e.to > i {
                e.to -= 1;
            }
        }
    }

    /// The direction the drawing runs: where most links point.
    pub fn direction(&self) -> Dir {
        let (mut dx, mut dy, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for e in &self.edges {
            let (Some(a), Some(b)) = (self.nodes.get(e.from), self.nodes.get(e.to)) else { continue };
            let (x, y) = (b.x - a.x, b.y - a.y);
            dx += x.abs();
            dy += y.abs();
            sx += x;
            sy += y;
        }
        if dx == 0.0 && dy == 0.0 {
            return self.dir;
        }
        if dx >= dy {
            if sx < 0.0 { Dir::RL } else { Dir::LR }
        } else if sy < 0.0 {
            Dir::BT
        } else {
            Dir::TD
        }
    }

    /// The sketch as Mermaid.
    pub fn to_mermaid(&self) -> String {
        let mut out = vec![format!("flowchart {}", self.direction().name())];
        for n in self.nodes.iter().take(MAX_NODES) {
            let (o, c) = n.shape.brackets();
            out.push(format!("    {}{o}{}{c}", n.id, quote(&n.text)));
        }
        for e in self.edges.iter().take(MAX_EDGES) {
            let (Some(a), Some(b)) = (self.nodes.get(e.from), self.nodes.get(e.to)) else { continue };
            let op = match (e.line, e.arrow) {
                (LineStyle::Solid, true) => "-->",
                (LineStyle::Solid, false) => "---",
                (LineStyle::Dotted, true) => "-.->",
                (LineStyle::Dotted, false) => "-.-",
                (LineStyle::Thick, true) => "==>",
                (LineStyle::Thick, false) => "===",
            };
            if e.label.trim().is_empty() {
                out.push(format!("    {} {op} {}", a.id, b.id));
            } else {
                out.push(format!("    {} {op}|{}| {}", a.id, quote(&e.label).replace('|', "#124;"), b.id));
            }
        }
        for l in &self.extra {
            out.push(format!("    {l}"));
        }
        for n in self.nodes.iter().take(MAX_NODES) {
            out.push(format!("    %% plume {} {:.0} {:.0}", n.id, finite(n.x), finite(n.y)));
        }
        out.join("\n")
    }

    /// A flowchart read back; `None` when it isn't one a sketch can show.
    pub fn parse(src: &str) -> Option<Sketch> {
        let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty());
        let head = lines.next()?;
        let mut words = head.split_whitespace();
        if !matches!(words.next()?, "flowchart" | "graph") {
            return None;
        }
        let dir = match words.next() {
            Some(d) => Dir::parse(d.trim_end_matches(';'))?,
            None => Dir::TD,
        };
        let mut s = Sketch { dir, ..Default::default() };
        let mut placed: Vec<(String, f32, f32)> = Vec::new();
        for line in lines.take(MAX_NODES + MAX_EDGES + 200) {
            if let Some(c) = line.strip_prefix("%%") {
                let mut w = c.split_whitespace();
                if w.next() == Some("plume")
                    && let (Some(id), Some(x), Some(y)) = (w.next(), w.next(), w.next())
                    && let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>())
                {
                    placed.push((id.to_string(), finite(x), finite(y)));
                }
                continue;
            }
            let first = line.split_whitespace().next().unwrap_or("");
            if matches!(first, "style" | "classDef" | "class" | "linkStyle" | "click") {
                s.extra.push(line.to_string());
                continue;
            }
            s.statement(line.trim_end_matches(';'))?;
        }
        if s.nodes.is_empty() {
            return None;
        }
        s.place();
        for (id, x, y) in placed {
            if let Some(n) = s.nodes.iter_mut().find(|n| n.id == id) {
                (n.x, n.y) = (x, y);
            }
        }
        Some(s)
    }

    /// One line: a shape, or shapes joined by links (`a --> b -->|yes| c`).
    fn statement(&mut self, line: &str) -> Option<()> {
        let mut rest = line;
        let (mut prev, r) = self.node_ref(rest)?;
        rest = r.trim_start();
        while !rest.is_empty() {
            let (style, arrow, label, r) = link(rest)?;
            let (next, r) = self.node_ref(r.trim_start())?;
            if self.edges.len() >= MAX_EDGES {
                return None;
            }
            self.edges.push(Edge { from: prev, to: next, label, arrow, line: style });
            prev = next;
            rest = r.trim_start();
        }
        Some(())
    }

    /// A shape reference (`id`, `id["text"]`, `id{text}`…): its index (added when new) and the
    /// rest of the line.
    fn node_ref<'a>(&mut self, s: &'a str) -> Option<(usize, &'a str)> {
        let end = s.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(s.len());
        let id = s.get(..end)?;
        if !valid_id(id) {
            return None;
        }
        let rest = s.get(end..)?;
        // The longest opening bracket, then the nearest closing one (`[/a\\]` is a
        // trapezoid, `[/a/]` a parallelogram).
        let mut best: Option<(usize, usize, NodeShape)> = None;
        for shape in NodeShape::ALL {
            let (o, c) = shape.brackets();
            let Some(inner) = rest.strip_prefix(o) else { continue };
            let close = if inner.starts_with('"') {
                inner.get(1..).and_then(|r| r.find('"')).map(|q| q + 2).and_then(|q| inner.get(q..)?.find(c).map(|k| k + q))
            } else {
                inner.find(c)
            };
            let Some(close) = close else { continue };
            if best.is_none_or(|(ol, cl, _)| o.len() > ol || (o.len() == ol && close < cl)) {
                best = Some((o.len(), close, shape));
            }
        }
        let found = match best {
            Some((ol, close, shape)) => {
                let inner = rest.get(ol..)?;
                Some((shape, unquote(inner.get(..close)?), inner.get(close + shape.brackets().1.len()..)?))
            }
            None => None,
        };
        let i = match self.nodes.iter().position(|n| n.id == id) {
            Some(i) => i,
            None => {
                if self.nodes.len() >= MAX_NODES {
                    return None;
                }
                self.nodes.push(Node { id: id.to_string(), text: id.to_string(), ..Default::default() });
                self.nodes.len() - 1
            }
        };
        match found {
            Some((shape, text, rest)) => {
                let n = self.nodes.get_mut(i)?;
                n.shape = shape;
                n.text = text;
                Some((i, rest))
            }
            None => Some((i, rest)),
        }
    }

    /// Place the shapes rank by rank along the direction (each one step after the shapes
    /// linking to it), for a flowchart without `%% plume` positions.
    pub fn place(&mut self) {
        let n = self.nodes.len();
        // Breadth first from the shapes nothing links to (links back to a shape already
        // placed are ignored, so a loop doesn't push it further).
        let mut rank: Vec<Option<usize>> = vec![None; n];
        let mut queue = std::collections::VecDeque::new();
        loop {
            let start = (0..n)
                .find(|&i| rank.get(i) == Some(&None) && !self.edges.iter().any(|e| e.to == i && e.from != i && rank.get(e.from) == Some(&None)));
            let Some(start) = start.or_else(|| (0..n).find(|&i| rank.get(i) == Some(&None))) else { break };
            if let Some(r) = rank.get_mut(start) {
                *r = Some(0);
            }
            queue.push_back(start);
            while let Some(i) = queue.pop_front() {
                let r = rank.get(i).copied().flatten().unwrap_or(0);
                for e in self.edges.iter().filter(|e| e.from == i) {
                    if let Some(t @ None) = rank.get_mut(e.to) {
                        *t = Some(r + 1);
                        queue.push_back(e.to);
                    }
                }
            }
        }
        let rank: Vec<usize> = rank.into_iter().map(|r| r.unwrap_or(0)).collect();
        let mut row = vec![0usize; n + 1];
        for (i, node) in self.nodes.iter_mut().enumerate() {
            let r = rank.get(i).copied().unwrap_or(0);
            let k = row.get(r).copied().unwrap_or(0);
            if let Some(v) = row.get_mut(r) {
                *v += 1;
            }
            let (along, across) = (90.0 + r as f32 * 190.0, 60.0 + k as f32 * 100.0);
            (node.x, node.y) = match self.dir {
                Dir::LR | Dir::RL => (along, across),
                Dir::TD | Dir::BT => (across + 60.0, along - 30.0),
            };
            if matches!(self.dir, Dir::RL) {
                node.x = 90.0 + (n.saturating_sub(1)) as f32 * 190.0 - node.x;
            }
            if matches!(self.dir, Dir::BT) {
                node.y = 60.0 + (n.saturating_sub(1)) as f32 * 190.0 - node.y;
            }
        }
    }

    /// A Word drawing (a drawing canvas or group) of shapes joined by connectors, as a sketch:
    /// each shape with text or a connector glued to it becomes a node, each connector glued at
    /// both ends a link. `text` gives a text box story's text. `None` when the drawing has
    /// pictures or charts, or no connector joining two shapes.
    pub fn from_drawing(children: &[GroupChild], text: &dyn Fn(u32) -> String) -> Option<Sketch> {
        let mut s = Sketch::default();
        let mut by_id: Vec<(u32, usize)> = Vec::new();
        let mut links = Vec::new();
        for c in children.iter().take(MAX_NODES + MAX_EDGES) {
            let InlineObject::Shape { kind, w, h, story, extra, .. } = &c.obj else {
                return None;
            };
            if kind.is_connector() || matches!(kind, ShapeKind::Line | ShapeKind::Arrow) {
                if let (Some(a), Some(b)) = (extra.start, extra.end) {
                    links.push((a.id, b.id, extra.arrow_start, extra.arrow_end));
                }
                continue;
            }
            let t = story.map(text).unwrap_or_default();
            if t.trim().is_empty() && extra.id == 0 {
                continue;
            }
            let i = s.add(NodeShape::of(*kind), t.trim(), c.x + w / 2.0, c.y + h / 2.0)?;
            if extra.id != 0 {
                by_id.push((extra.id, i));
            }
        }
        let find = |id: u32| by_id.iter().find(|(d, _)| *d == id).map(|(_, i)| *i);
        for (a, b, arrow_start, arrow_end) in links {
            let (Some(a), Some(b)) = (find(a), find(b)) else { continue };
            // An arrowhead only at the start: the link runs the other way.
            let (from, to) = if arrow_start && !arrow_end { (b, a) } else { (a, b) };
            if let Some(e) = s.connect(from, to)
                && let Some(e) = s.edges.get_mut(e)
            {
                e.arrow = arrow_start || arrow_end;
            }
        }
        if s.edges.is_empty() {
            return None;
        }
        // Word's coordinates are points; the canvas's are about the same.
        s.dir = s.direction();
        Some(s)
    }
}

/// A link at the start of `s`: its style, arrowhead, label, and the rest.
fn link(s: &str) -> Option<(LineStyle, bool, String, &str)> {
    let end = s.find(|c: char| !matches!(c, '-' | '.' | '=' | '>')).unwrap_or(s.len());
    let op = s.get(..end)?;
    let mut rest = s.get(end..)?;
    if op.len() < 2 {
        return None;
    }
    let style = if op.contains('=') {
        LineStyle::Thick
    } else if op.contains('.') {
        LineStyle::Dotted
    } else {
        LineStyle::Solid
    };
    let (op, mut label) = if matches!(op, "--" | "-." | "==") {
        // `-- text -->`: the label runs to the link's end.
        let close = ["-->", "---", ".->", "-.-", "==>", "==="].iter().filter_map(|c| rest.find(c).map(|k| (k, *c))).min_by_key(|(k, _)| *k)?;
        let label = rest.get(..close.0)?.trim().to_string();
        let tail = rest.get(close.0..)?;
        let e = tail.find(|c: char| !matches!(c, '-' | '.' | '=' | '>')).unwrap_or(tail.len());
        rest = tail.get(e..)?;
        (tail.get(..e)?, label)
    } else {
        (op, String::new())
    };
    if let Some(r) = rest.strip_prefix('|') {
        let close = r.find('|')?;
        label = r.get(..close)?.to_string();
        rest = r.get(close + 1..)?;
    }
    let label = unquote(&label.replace("#124;", "|"));
    Some((style, op.ends_with('>'), label, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sketch_round_trips_through_mermaid_and_draws() {
        let mut s = Sketch::default();
        let a = s.add(NodeShape::Stadium, "Start", 40.0, 60.0).unwrap();
        let b = s.add(NodeShape::Diamond, "Reviewed \"twice\"?", 240.0, 60.0).unwrap();
        let c = s.add(NodeShape::Database, "Archive", 440.0, 60.0).unwrap();
        s.connect(a, b);
        let e = s.connect(b, c).unwrap();
        s.edges[e].label = "yes | sure".into();
        s.edges[e].line = LineStyle::Dotted;
        let back = s.connect(b, a).unwrap();
        s.edges[back].line = LineStyle::Thick;
        s.edges[back].arrow = false;
        let src = s.to_mermaid();
        assert!(src.starts_with("flowchart LR\n    n1([\"Start\"])"), "{src}");
        let again = Sketch::parse(&src).unwrap();
        assert_eq!(again.nodes, s.nodes);
        assert_eq!(again.edges, s.edges);
        assert_eq!(again.to_mermaid(), src);
        crate::draw(&src).unwrap();
        for shape in NodeShape::ALL {
            let mut one = Sketch::default();
            one.add(shape, "x", 0.0, 0.0);
            crate::draw(&one.to_mermaid()).unwrap();
        }
    }

    #[test]
    fn hand_written_flowcharts_open_and_others_do_not() {
        let s = Sketch::parse("graph TD;\n  A[Draft] --> B{Reviewed?};\n  B -- no --> A\n  B -->|yes| C(Publish)\n  style A fill:#f9f\n").unwrap();
        assert_eq!(s.nodes.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["Draft", "Reviewed?", "Publish"]);
        assert_eq!(s.edges.len(), 3);
        assert_eq!(s.edges[1].label, "no");
        assert_eq!(s.edges[2].label, "yes");
        assert_eq!(s.extra, ["style A fill:#f9f"]);
        assert!(s.nodes[1].y > s.nodes[0].y, "placed top down");
        assert_eq!(s.direction(), Dir::TD);
        for other in [
            "sequenceDiagram\n A->>B: hi",
            "flowchart LR\n subgraph one\n a-->b\n end",
            "flowchart LR\n a & b --> c",
            "pie\n \"a\" : 1",
            "",
            "flowchart XY\n a",
        ] {
            assert!(Sketch::parse(other).is_none(), "{other}");
        }
    }

    #[test]
    fn hostile_sources_never_panic() {
        for src in [
            "flowchart LR\n a[\"unclosed",
            "flowchart LR\n a -->|x",
            "flowchart LR\n a --",
            "flowchart LR\n é((ü)) ==> ß",
            "graph\n a-- -->b",
            "flowchart LR\n %% plume a NaN inf",
        ] {
            let _ = Sketch::parse(src).map(|s| s.to_mermaid());
        }
        let many: String = (0..2000).map(|k| format!("a{k} --> a{}\n", k + 1)).collect();
        assert!(Sketch::parse(&format!("flowchart LR\n{many}")).is_none());
    }
}
