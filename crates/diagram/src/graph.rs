//! Plume: diagrams drawn on a canvas, shapes joined by connectors: flowcharts, state, class,
//! entity-relationship, requirement, C4, architecture and block diagrams. One model for all of
//! them ([`Graph`]); each kind says which shapes and connectors it has ([`GraphSpec`]) and is
//! written to and read from Mermaid in [`crate::codec`].
//!
//! Where each shape was put on the canvas is kept in `%% plume <id> <x> <y>` comments, which
//! Mermaid ignores, so a diagram opens as it was left.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::form::{FieldSpec, clip_text, finite};

/// Most shapes and connectors on a canvas.
pub const MAX_NODES: usize = 300;
pub const MAX_EDGES: usize = 600;
/// Most lines in a shape (a class's members, an entity's attributes).
pub const MAX_LINES: usize = 100;

/// How a shape is drawn on the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Rect,
    Rounded,
    Stadium,
    Circle,
    /// A small filled circle: a start state.
    Dot,
    /// A circle in a ring: an end state.
    Bullseye,
    Diamond,
    Hexagon,
    Parallelogram,
    ParallelogramAlt,
    Trapezoid,
    TrapezoidAlt,
    Cylinder,
    /// A thick bar: fork and join.
    Bar,
    /// A stick figure over a box: a person.
    Person,
    /// A box with a title and lines under a rule: classes, entities, requirements.
    Table,
    /// A dashed frame others sit in.
    Group,
    Subroutine,
    DoubleCircle,
    /// A flag pointing right (`>text]`).
    Flag,
    Cloud,
    /// A small dot joining connectors.
    Junction,
}

/// An end of a connector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    None,
    Arrow,
    /// A hollow triangle (inheritance, realization).
    Triangle,
    DiamondFilled,
    DiamondHollow,
    Cross,
    Circle,
}

/// A kind of shape a canvas offers.
#[derive(Clone, Copy, Debug)]
pub struct NodeKind {
    pub id: &'static str,
    pub label: &'static str,
    pub look: Look,
    /// Its properties beyond its text.
    pub fields: &'static [FieldSpec],
    /// It holds lines (members, attributes): their label.
    pub lines: Option<&'static str>,
    /// Others can sit in it (groups, boundaries).
    pub group: bool,
    /// It has no text of its own (start, end, fork, junction).
    pub bare: bool,
}

impl NodeKind {
    pub const fn new(id: &'static str, label: &'static str, look: Look) -> NodeKind {
        NodeKind { id, label, look, fields: &[], lines: None, group: false, bare: false }
    }
    pub const fn fields(mut self, f: &'static [FieldSpec]) -> NodeKind {
        self.fields = f;
        self
    }
    pub const fn lines(mut self, label: &'static str) -> NodeKind {
        self.lines = Some(label);
        self
    }
    pub const fn group(mut self) -> NodeKind {
        self.group = true;
        self
    }
    pub const fn bare(mut self) -> NodeKind {
        self.bare = true;
        self
    }
}

/// A kind of connector a canvas offers.
#[derive(Clone, Copy, Debug)]
pub struct EdgeKind {
    pub id: &'static str,
    pub label: &'static str,
    pub dashed: bool,
    pub thick: bool,
    /// At the start and at the end.
    pub tail: Head,
    pub head: Head,
    pub fields: &'static [FieldSpec],
}

impl EdgeKind {
    pub const fn new(id: &'static str, label: &'static str, tail: Head, head: Head) -> EdgeKind {
        EdgeKind { id, label, dashed: false, thick: false, tail, head, fields: &[] }
    }
    pub const fn dashed(mut self) -> EdgeKind {
        self.dashed = true;
        self
    }
    pub const fn thick(mut self) -> EdgeKind {
        self.thick = true;
        self
    }
    pub const fn fields(mut self, f: &'static [FieldSpec]) -> EdgeKind {
        self.fields = f;
        self
    }
}

/// What a canvas offers.
#[derive(Clone, Copy, Debug)]
pub struct GraphSpec {
    /// The diagram's own settings.
    pub fields: &'static [FieldSpec],
    pub nodes: &'static [NodeKind],
    pub edges: &'static [EdgeKind],
    /// Connectors have a label.
    pub edge_label: bool,
}

impl GraphSpec {
    pub fn node(&self, id: &str) -> Option<&'static NodeKind> {
        self.nodes.iter().find(|k| k.id == id)
    }
    pub fn edge(&self, id: &str) -> Option<&'static EdgeKind> {
        self.edges.iter().find(|k| k.id == id)
    }
}

/// A shape: `x`, `y` is its centre on the canvas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Node {
    /// Its id in the Mermaid text.
    pub id: String,
    /// Its [`NodeKind`] id.
    pub kind: String,
    pub text: String,
    pub fields: BTreeMap<String, String>,
    pub lines: Vec<String>,
    /// The id of the group it sits in, `""` for none.
    pub parent: String,
    pub x: f32,
    pub y: f32,
}

/// A connector from shape `from` to shape `to` (indices in [`Graph::nodes`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    /// Its [`EdgeKind`] id.
    pub kind: String,
    pub label: String,
    pub fields: BTreeMap<String, String>,
}

/// A diagram drawn on a canvas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub fields: BTreeMap<String, String>,
    /// Lines kept as written (styles).
    pub extra: Vec<String>,
}

impl Graph {
    /// A new shape id from `base`, unused by the others.
    pub fn new_id(&self, base: &str) -> String {
        let base = crate::form::ident(base, "n");
        crate::form::unique(&base, &|c| self.nodes.iter().any(|n| n.id == c))
    }

    /// Add a shape of `kind` at `x`, `y`; its index, `None` when the canvas is full.
    pub fn add(&mut self, kind: &str, id_base: &str, text: &str, x: f32, y: f32) -> Option<usize> {
        if self.nodes.len() >= MAX_NODES {
            return None;
        }
        let id = self.new_id(id_base);
        self.nodes.push(Node { id, kind: kind.to_string(), text: clip_text(text), x: finite(x), y: finite(y), ..Default::default() });
        Some(self.nodes.len() - 1)
    }

    /// Join shape `from` to shape `to` with a connector of `kind`; `None` when it can't.
    pub fn connect(&mut self, from: usize, to: usize, kind: &str) -> Option<usize> {
        if from >= self.nodes.len() || to >= self.nodes.len() || self.edges.len() >= MAX_EDGES {
            return None;
        }
        self.edges.push(Edge { from, to, kind: kind.to_string(), ..Default::default() });
        Some(self.edges.len() - 1)
    }

    /// Remove shape `i`, its connectors, and its place as a group.
    pub fn remove_node(&mut self, i: usize) {
        let Some(gone) = self.nodes.get(i).map(|n| n.id.clone()) else { return };
        self.nodes.remove(i);
        self.edges.retain(|e| e.from != i && e.to != i);
        for e in &mut self.edges {
            e.from -= usize::from(e.from > i);
            e.to -= usize::from(e.to > i);
        }
        for n in &mut self.nodes {
            if n.parent == gone {
                n.parent.clear();
            }
        }
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Which way the drawing runs, from where most connectors point: `LR`, `RL`, `TB`, `BT`;
    /// `None` without connectors.
    pub fn direction(&self) -> Option<&'static str> {
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
            None
        } else if dx >= dy {
            Some(if sx < 0.0 { "RL" } else { "LR" })
        } else {
            Some(if sy < 0.0 { "BT" } else { "TB" })
        }
    }

    /// The `%% plume` lines keeping where the shapes are.
    pub fn positions(&self) -> Vec<String> {
        self.nodes.iter().take(MAX_NODES).map(|n| format!("    %% plume {} {:.0} {:.0}", n.id, finite(n.x), finite(n.y))).collect()
    }

    /// Put back the positions `src`'s `%% plume` lines keep; shapes without one are placed
    /// rank by rank (`across`: ranks run down the page rather than across it). Whether every
    /// shape had one.
    pub fn restore_positions(&mut self, src: &str, across: bool) -> bool {
        self.place(across);
        let mut all = true;
        let placed: Vec<(String, f32, f32)> = src
            .lines()
            .filter_map(|l| {
                let mut w = l.trim().strip_prefix("%%")?.split_whitespace();
                (w.next()? == "plume").then_some(())?;
                let (id, x, y) = (w.next()?, w.next()?.parse::<f32>().ok()?, w.next()?.parse::<f32>().ok()?);
                Some((id.to_string(), finite(x), finite(y)))
            })
            .collect();
        for n in &mut self.nodes {
            match placed.iter().find(|(id, _, _)| *id == n.id) {
                Some((_, x, y)) => (n.x, n.y) = (*x, *y),
                None => all = false,
            }
        }
        all
    }

    /// Place the shapes rank by rank along the connectors, left to right (`across`) or top
    /// down.
    pub fn place(&mut self, across: bool) {
        let n = self.nodes.len();
        let mut rank: Vec<Option<usize>> = vec![None; n];
        let mut queue = std::collections::VecDeque::new();
        while let Some(start) = (0..n)
            .find(|&i| rank.get(i) == Some(&None) && !self.edges.iter().any(|e| e.to == i && e.from != i && rank.get(e.from) == Some(&None)))
            .or_else(|| (0..n).find(|&i| rank.get(i) == Some(&None)))
        {
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
        let mut row = vec![0usize; n + 1];
        for (i, node) in self.nodes.iter_mut().enumerate() {
            let r = rank.get(i).copied().flatten().unwrap_or(0);
            let k = row.get(r).copied().unwrap_or(0);
            if let Some(v) = row.get_mut(r) {
                *v += 1;
            }
            let (along, side) = (100.0 + r as f32 * 200.0, 70.0 + k as f32 * 120.0);
            (node.x, node.y) = if across { (along, side) } else { (side + 40.0, along - 30.0) };
        }
    }
}
