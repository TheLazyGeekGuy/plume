//! Plume: the canvas of the diagram editor, for the diagrams drawn as shapes joined by
//! connectors (flowcharts, state, class, entity-relationship, requirement, C4, architecture and
//! block diagrams). As in Office: pick a shape in the palette and click to place it, drag it to
//! move it, drag from a blue handle to another shape to connect them; groups (boundaries) frame
//! the shapes dropped in them. The panel beside it edits what is selected.

use egui::{Color32, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use wordcraft_engine::diagram::codec::{Kind, graphs};
use wordcraft_engine::diagram::form::{self, FieldSpec, get, set};
use wordcraft_engine::diagram::graph::{EdgeKind, Graph, GraphSpec, Head, Look, Node, NodeKind};

/// What a click on the canvas does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Select,
    Place(&'static str),
    Connect,
}

/// The shape or connector selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Node(usize),
    Edge(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Move { node: usize, grab: Vec2, last: Pos2 },
    Link { from: usize, to: Pos2 },
    Pan,
}

/// The canvas's state between frames.
#[derive(Clone, Debug, Default)]
pub struct Canvas {
    pub tool: Tool,
    /// The kind of connector the connector tool draws.
    pub edge_kind: &'static str,
    pub pick: Option<Pick>,
    drag: Option<Drag>,
    pan: Vec2,
    /// Canvas points per screen point; 0 until the drawing is first fitted in view.
    zoom: f32,
    /// The selected shape's text field takes the keyboard.
    pub focus_text: bool,
}

const BLUE: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
const INK: Color32 = Color32::from_rgb(0x33, 0x41, 0x55);
const FILL: Color32 = Color32::from_rgb(0xEC, 0xF2, 0xFE);
const LINE: Color32 = Color32::from_rgb(0x93, 0x70, 0xDB);
const TEXT: Color32 = Color32::from_rgb(0x1F, 0x29, 0x37);

/// The kind of a shape, with a fallback.
fn kind_of<'a>(spec: &'a GraphSpec, n: &Node) -> Option<&'a NodeKind> {
    spec.node(&n.kind).or_else(|| spec.nodes.first())
}

fn look_of(spec: &GraphSpec, n: &Node) -> Look {
    kind_of(spec, n).map_or(Look::Rect, |k| k.look)
}

/// The lines shown in a shape under its name: its members, or its fields.
fn body_lines(spec: &GraphSpec, n: &Node) -> Vec<String> {
    let Some(k) = kind_of(spec, n) else { return Vec::new() };
    if k.lines.is_some() {
        return n.lines.iter().take(12).cloned().collect();
    }
    if k.look == Look::Table {
        return k
            .fields
            .iter()
            .filter(|f| !get(&n.fields, f.key).is_empty())
            .map(|f| format!("{}: {}", tl!(f.label), get(&n.fields, f.key)))
            .take(8)
            .collect();
    }
    Vec::new()
}

fn text_w(s: &str) -> f32 {
    s.chars().count() as f32 * 6.8
}

/// The text a shape shows.
fn shown(n: &Node) -> String {
    n.text.clone()
}

/// A shape's size on the canvas (groups excepted: they frame their members).
fn size(kind: Kind, spec: &GraphSpec, n: &Node) -> Vec2 {
    let t = text_w(&shown(n));
    let w = (t + 30.0).clamp(80.0, 230.0);
    if kind == Kind::Block {
        let span = get(&n.fields, "span").trim().parse::<f32>().unwrap_or(1.0).clamp(1.0, 64.0);
        return vec2(span * graphs::BLOCK_W - 24.0, 50.0);
    }
    match look_of(spec, n) {
        Look::Dot => Vec2::splat(18.0),
        Look::Bullseye => Vec2::splat(22.0),
        Look::Junction => Vec2::splat(14.0),
        Look::Bar => vec2(90.0, 10.0),
        Look::Circle | Look::DoubleCircle => Vec2::splat(w.clamp(60.0, 130.0)),
        Look::Diamond if kind_of(spec, n).is_some_and(|k| k.bare) => Vec2::splat(34.0),
        Look::Diamond => vec2(w + 40.0, 64.0),
        Look::Person => vec2(w.max(100.0), 74.0),
        Look::Table => {
            let lines = body_lines(spec, n);
            let widest = lines.iter().map(|l| text_w(l) * 0.9).fold(t, f32::max);
            vec2((widest + 28.0).clamp(110.0, 320.0), 30.0 + lines.len() as f32 * 16.0 + if lines.is_empty() { 0.0 } else { 6.0 })
        }
        Look::Group => vec2(170.0, 110.0),
        Look::Hexagon | Look::Parallelogram | Look::ParallelogramAlt | Look::Trapezoid | Look::TrapezoidAlt => vec2(w + 24.0, 46.0),
        _ => vec2(w, 46.0),
    }
}

/// Every shape's frame on the canvas, in canvas coordinates: groups around their members.
fn frames(kind: Kind, spec: &GraphSpec, g: &Graph) -> Vec<Rect> {
    let mut out: Vec<Rect> = g.nodes.iter().map(|n| Rect::from_center_size(pos2(n.x, n.y), size(kind, spec, n))).collect();
    // Groups frame their members, innermost first (a few passes cover nesting).
    for _ in 0..6 {
        for (i, n) in g.nodes.iter().enumerate() {
            if !kind_of(spec, n).is_some_and(|k| k.group) {
                continue;
            }
            let members: Vec<Rect> =
                g.nodes.iter().enumerate().filter(|(j, m)| *j != i && m.parent == n.id).filter_map(|(j, _)| out.get(j).copied()).collect();
            if let Some(first) = members.first() {
                let r = members.iter().fold(*first, |a, b| a.union(*b));
                if let Some(slot) = out.get_mut(i) {
                    *slot = Rect::from_min_max(r.min - vec2(16.0, 30.0), r.max + vec2(16.0, 14.0));
                }
            }
        }
    }
    out
}

/// The point where the line from the centre of `r` towards `dir` leaves a shape of `look`.
fn boundary(look: Look, r: Rect, dir: Vec2) -> Pos2 {
    let h = r.size() / 2.0;
    let (dx, dy) = (dir.x.abs().max(1e-4), dir.y.abs().max(1e-4));
    let t = match look {
        Look::Circle | Look::DoubleCircle | Look::Dot | Look::Bullseye | Look::Junction => h.x.min(h.y),
        Look::Diamond => 1.0 / (dx / h.x + dy / h.y),
        _ => (h.x / dx).min(h.y / dy),
    };
    r.center() + dir * t
}

/// A shape of `look` filling `r`.
pub fn outline(p: &Painter, look: Look, r: Rect, fill: Color32, stroke: Stroke) {
    let (c, w, h) = (r.center(), r.width(), r.height());
    let poly = |pts: Vec<Pos2>| Shape::convex_polygon(pts, fill, stroke);
    let k = (h / 2.0).min(w / 4.0);
    match look {
        Look::Rect | Look::Table => {
            p.rect(r, 0.0, fill, stroke, egui::StrokeKind::Inside);
        }
        Look::Rounded => {
            p.rect(r, (h / 4.0).min(8.0), fill, stroke, egui::StrokeKind::Inside);
        }
        Look::Stadium => {
            p.rect(r, h / 2.0, fill, stroke, egui::StrokeKind::Inside);
        }
        Look::Circle => {
            p.circle(c, w.min(h) / 2.0, fill, stroke);
        }
        Look::DoubleCircle => {
            p.circle(c, w.min(h) / 2.0, fill, stroke);
            p.circle_stroke(c, w.min(h) / 2.0 - 4.0, stroke);
        }
        Look::Dot => {
            p.circle_filled(c, w.min(h) / 2.0, stroke.color);
        }
        Look::Bullseye => {
            p.circle_stroke(c, w.min(h) / 2.0, stroke);
            p.circle_filled(c, w.min(h) / 2.0 - 4.0, stroke.color);
        }
        Look::Junction => {
            p.circle_filled(c, w.min(h) / 2.0, stroke.color);
        }
        Look::Bar => {
            p.rect_filled(r, 2.0, stroke.color);
        }
        Look::Diamond => {
            p.add(poly(vec![pos2(c.x, r.top()), pos2(r.right(), c.y), pos2(c.x, r.bottom()), pos2(r.left(), c.y)]));
        }
        Look::Hexagon => {
            p.add(poly(vec![
                pos2(r.left() + k, r.top()),
                pos2(r.right() - k, r.top()),
                pos2(r.right(), c.y),
                pos2(r.right() - k, r.bottom()),
                pos2(r.left() + k, r.bottom()),
                pos2(r.left(), c.y),
            ]));
        }
        Look::Parallelogram => {
            p.add(poly(vec![pos2(r.left() + k, r.top()), pos2(r.right(), r.top()), pos2(r.right() - k, r.bottom()), pos2(r.left(), r.bottom())]));
        }
        Look::ParallelogramAlt => {
            p.add(poly(vec![r.left_top(), pos2(r.right() - k, r.top()), r.right_bottom(), pos2(r.left() + k, r.bottom())]));
        }
        Look::Trapezoid => {
            p.add(poly(vec![pos2(r.left() + k, r.top()), pos2(r.right() - k, r.top()), r.right_bottom(), r.left_bottom()]));
        }
        Look::TrapezoidAlt => {
            p.add(poly(vec![r.left_top(), r.right_top(), pos2(r.right() - k, r.bottom()), pos2(r.left() + k, r.bottom())]));
        }
        Look::Flag => {
            p.add(poly(vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]));
            let notch = (h / 3.0).min(w / 5.0);
            p.add(Shape::convex_polygon(vec![r.left_top(), pos2(r.left() + notch, c.y), r.left_bottom()], Color32::WHITE, Stroke::NONE));
            p.line_segment([r.left_top(), pos2(r.left() + notch, c.y)], stroke);
            p.line_segment([pos2(r.left() + notch, c.y), r.left_bottom()], stroke);
        }
        Look::Subroutine => {
            p.rect(r, 0.0, fill, stroke, egui::StrokeKind::Inside);
            let i = (w / 12.0).clamp(2.0, 8.0);
            p.line_segment([pos2(r.left() + i, r.top()), pos2(r.left() + i, r.bottom())], stroke);
            p.line_segment([pos2(r.right() - i, r.top()), pos2(r.right() - i, r.bottom())], stroke);
        }
        Look::Cylinder => {
            let e = (h / 7.0).max(2.0);
            p.rect(
                Rect::from_min_max(pos2(r.left(), r.top() + e), pos2(r.right(), r.bottom() - e)),
                0.0,
                fill,
                Stroke::NONE,
                egui::StrokeKind::Inside,
            );
            p.add(Shape::ellipse_filled(pos2(c.x, r.bottom() - e), vec2(w / 2.0, e), fill));
            p.add(Shape::ellipse_stroke(pos2(c.x, r.bottom() - e), vec2(w / 2.0, e), stroke));
            p.line_segment([pos2(r.left(), r.top() + e), pos2(r.left(), r.bottom() - e)], stroke);
            p.line_segment([pos2(r.right(), r.top() + e), pos2(r.right(), r.bottom() - e)], stroke);
            p.add(Shape::ellipse_filled(pos2(c.x, r.top() + e), vec2(w / 2.0, e), fill));
            p.add(Shape::ellipse_stroke(pos2(c.x, r.top() + e), vec2(w / 2.0, e), stroke));
        }
        Look::Person => {
            let head = (h * 0.16).max(3.0);
            p.circle(pos2(c.x, r.top() + head), head, fill, stroke);
            p.rect(Rect::from_min_max(pos2(r.left(), r.top() + head * 2.4), r.max), 6.0, fill, stroke, egui::StrokeKind::Inside);
        }
        Look::Group => {
            p.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(0x93, 0x70, 0xDB, 14));
            p.extend(Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], stroke, 5.0, 3.0));
        }
        Look::Cloud => {
            p.rect(r, h / 2.0, fill, stroke, egui::StrokeKind::Inside);
        }
    }
}

/// A connector end's head at `at`, pointing along `dir`; where the line should stop.
fn head(p: &Painter, h: Head, at: Pos2, dir: Vec2, color: Color32) -> Pos2 {
    let n = vec2(-dir.y, dir.x);
    match h {
        Head::None => at,
        Head::Arrow => {
            p.add(Shape::convex_polygon(vec![at, at - dir * 10.0 + n * 5.0, at - dir * 10.0 - n * 5.0], color, Stroke::NONE));
            at - dir * 9.0
        }
        Head::Triangle => {
            p.add(Shape::convex_polygon(vec![at, at - dir * 12.0 + n * 7.0, at - dir * 12.0 - n * 7.0], Color32::WHITE, Stroke::new(1.3, color)));
            at - dir * 12.0
        }
        Head::DiamondFilled | Head::DiamondHollow => {
            let fill = if h == Head::DiamondFilled { color } else { Color32::WHITE };
            p.add(Shape::convex_polygon(
                vec![at, at - dir * 8.0 + n * 5.0, at - dir * 16.0, at - dir * 8.0 - n * 5.0],
                fill,
                Stroke::new(1.3, color),
            ));
            at - dir * 16.0
        }
        Head::Cross => {
            let c = at - dir * 6.0;
            p.line_segment([c + (dir + n) * 4.0, c - (dir + n) * 4.0], Stroke::new(1.5, color));
            p.line_segment([c + (dir - n) * 4.0, c - (dir - n) * 4.0], Stroke::new(1.5, color));
            at
        }
        Head::Circle => {
            p.circle(at - dir * 5.0, 5.0, Color32::WHITE, Stroke::new(1.3, color));
            at - dir * 10.0
        }
    }
}

/// A connector of `kind` from `a` to `b`.
pub fn connector(p: &Painter, kind: Option<&EdgeKind>, a: Pos2, b: Pos2, color: Color32) {
    let dir = (b - a).normalized();
    if !dir.x.is_finite() || !dir.y.is_finite() {
        return;
    }
    let (tail, hd, dashed, thick) = kind.map_or((Head::None, Head::Arrow, false, false), |k| (k.tail, k.head, k.dashed, k.thick));
    let start = head(p, tail, a, -dir, color);
    let end = head(p, hd, b, dir, color);
    let stroke = Stroke::new(if thick { 3.0 } else { 1.4 }, color);
    if dashed {
        p.extend(Shape::dashed_line(&[start, end], stroke, 5.0, 3.0));
    } else {
        p.line_segment([start, end], stroke);
    }
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// Labels near a connector's ends (multiplicities, cardinalities).
fn end_labels(kind: Kind, e: &wordcraft_engine::diagram::graph::Edge) -> (String, String) {
    match kind {
        Kind::Class => (get(&e.fields, "fromCard").to_string(), get(&e.fields, "toCard").to_string()),
        Kind::Er => {
            let c = |k: &str| match get(&e.fields, k) {
                "zeroOne" => "0..1",
                "many" => "1..*",
                "zeroMany" => "0..*",
                _ => "1",
            };
            (c("fromCard").into(), c("toCard").into())
        }
        _ => (String::new(), String::new()),
    }
}

/// The ids of a shape and of the groups inside it, to the bottom.
fn descendants(g: &Graph, i: usize) -> Vec<usize> {
    let mut out = vec![i];
    let mut k = 0;
    while let Some(&at) = out.get(k) {
        if let Some(id) = g.nodes.get(at).map(|n| n.id.clone()) {
            for (j, n) in g.nodes.iter().enumerate() {
                if n.parent == id && !out.contains(&j) && out.len() < 1000 {
                    out.push(j);
                }
            }
        }
        k += 1;
    }
    out
}

/// The palette: Select, each kind of shape, the connector and its kind.
fn palette(ui: &mut Ui, spec: &GraphSpec, c: &mut Canvas) {
    let ink = ui.visuals().text_color();
    ui.horizontal_wrapped(|ui| {
        let tool_button = |ui: &mut Ui, on: bool, tip: &str, draw: &dyn Fn(&Painter, Rect)| {
            let r = ui.add(egui::Button::new("").min_size(vec2(32.0, 26.0)).selected(on)).on_hover_text(tl!(tip));
            draw(ui.painter(), r.rect.shrink2(vec2(7.0, 6.0)));
            r.clicked()
        };
        if tool_button(ui, c.tool == Tool::Select, "Select and Move", &|p, r| {
            p.add(Shape::convex_polygon(
                vec![r.left_top(), pos2(r.left(), r.bottom()), pos2(r.center().x, r.center().y + 2.0), r.right_bottom() - vec2(0.0, 4.0)],
                ink,
                Stroke::NONE,
            ));
        }) {
            c.tool = Tool::Select;
        }
        ui.separator();
        for k in spec.nodes {
            if tool_button(ui, c.tool == Tool::Place(k.id), k.label, &|p, r| outline(p, k.look, r, Color32::TRANSPARENT, Stroke::new(1.3, ink))) {
                c.tool = Tool::Place(k.id);
            }
        }
        ui.separator();
        let edge = spec.edge(c.edge_kind).or(spec.edges.first());
        if tool_button(ui, c.tool == Tool::Connect, "Connector", &|p, r| connector(p, edge, r.left_bottom(), r.right_top(), ink)) {
            c.tool = Tool::Connect;
        }
        if spec.edges.len() > 1 {
            let shown = edge.map_or("", |k| k.label);
            egui::ComboBox::from_id_salt("diagram_edge_kind").width(150.0).selected_text(tl!(shown)).show_ui(ui, |ui| {
                for k in spec.edges {
                    if ui.selectable_label(c.edge_kind == k.id, tl!(k.label)).clicked() {
                        c.edge_kind = k.id;
                        c.tool = Tool::Connect;
                    }
                }
            });
        }
    });
}

/// Add a shape of `kind` at `at` (canvas coordinates): text and id from its kind.
fn place(kind: Kind, g: &mut Graph, k: &NodeKind, at: Pos2) -> Option<usize> {
    let label = tl!(k.label).to_string();
    let base = match kind {
        Kind::Class | Kind::Requirement => form::ident(&label, "n"),
        Kind::Er => form::ident(&label, "E").to_uppercase(),
        _ => form::ident(k.id, "n"),
    };
    let text = if k.bare { String::new() } else { label };
    let i = g.add(k.id, &base, &text, at.x, at.y)?;
    if matches!(kind, Kind::Class | Kind::Requirement)
        && let Some(n) = g.nodes.get_mut(i)
    {
        n.text = n.id.clone();
    }
    Some(i)
}

/// Snap a block to its grid cell.
fn snap_block(n: &mut Node) {
    let span = get(&n.fields, "span").trim().parse::<f32>().unwrap_or(1.0).clamp(1.0, 64.0);
    let col = ((n.x - span * graphs::BLOCK_W / 2.0) / graphs::BLOCK_W).round().max(0.0);
    let row = ((n.y - graphs::BLOCK_H / 2.0) / graphs::BLOCK_H).round().max(0.0);
    n.x = col * graphs::BLOCK_W + span * graphs::BLOCK_W / 2.0;
    n.y = row * graphs::BLOCK_H + graphs::BLOCK_H / 2.0;
}

/// The canvas.
pub fn show(ui: &mut Ui, kind: Kind, spec: &GraphSpec, g: &mut Graph, c: &mut Canvas) {
    if spec.edge(c.edge_kind).is_none() {
        c.edge_kind = spec.edges.first().map_or("", |k| k.id);
    }
    palette(ui, spec, c);
    let hint = match c.tool {
        Tool::Select => "Drag a shape to move it; drag from a blue handle to another shape to connect them.",
        Tool::Place(_) => "Click in the canvas to place the shape.",
        Tool::Connect => "Drag from one shape to another to connect them.",
    };
    ui.weak(tl!(hint));
    let (resp, painter) = ui.allocate_painter(vec2(600.0, 400.0), Sense::click_and_drag());
    let area = resp.rect;
    let painter = painter.with_clip_rect(area);
    let t = crate::theme::Tokens::get(ui.ctx());
    painter.rect_filled(area, 2.0, Color32::WHITE);
    painter.rect_stroke(area, 2.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    // The first time, the drawing is fitted in view.
    if c.zoom <= 0.0 {
        fit(kind, spec, g, c, area);
    }
    let z = c.zoom.clamp(0.25, 2.0);
    let origin = area.min.to_vec2() + c.pan;
    let fs = |pt: f32| egui::FontId::proportional((pt * z).max(6.0));
    // The grid: dots, or a block diagram's cells.
    if kind == Kind::Block {
        let mut x = area.left() + c.pan.x.rem_euclid(graphs::BLOCK_W * z);
        while x < area.right() {
            painter.line_segment([pos2(x, area.top()), pos2(x, area.bottom())], Stroke::new(1.0, Color32::from_gray(236)));
            x += graphs::BLOCK_W * z;
        }
        let mut y = area.top() + c.pan.y.rem_euclid(graphs::BLOCK_H * z);
        while y < area.bottom() {
            painter.line_segment([pos2(area.left(), y), pos2(area.right(), y)], Stroke::new(1.0, Color32::from_gray(236)));
            y += graphs::BLOCK_H * z;
        }
    } else {
        let step = 20.0 * z;
        let mut x = area.left() + c.pan.x.rem_euclid(step);
        while x < area.right() {
            let mut y = area.top() + c.pan.y.rem_euclid(step);
            while y < area.bottom() {
                painter.circle_filled(pos2(x, y), 0.8, Color32::from_gray(205));
                y += step;
            }
            x += step;
        }
    }
    let to_screen = |p: Pos2| (p.to_vec2() * z + origin).to_pos2();
    let to_canvas = |p: Pos2| ((p.to_vec2() - origin) / z).to_pos2();
    let rects: Vec<Rect> = frames(kind, spec, g).into_iter().map(|r| Rect::from_min_max(to_screen(r.min), to_screen(r.max))).collect();
    let groups: Vec<bool> = g.nodes.iter().map(|n| kind_of(spec, n).is_some_and(|k| k.group)).collect();
    let is_group = |i: usize| groups.get(i).copied().unwrap_or(false);
    // Shapes on top first, then groups, smallest first.
    let node_at = |p: Pos2| -> Option<usize> {
        let mut hits: Vec<usize> = (0..rects.len()).filter(|&i| rects.get(i).is_some_and(|r| r.expand(3.0).contains(p))).collect();
        hits.sort_by(|&a, &b| {
            let area = |i: usize| rects.get(i).map_or(0.0, |r| r.area());
            is_group(a).cmp(&is_group(b)).then(area(a).total_cmp(&area(b)))
        });
        hits.first().copied()
    };
    let handles =
        |r: Rect| [pos2(r.center().x, r.top()), pos2(r.right(), r.center().y), pos2(r.center().x, r.bottom()), pos2(r.left(), r.center().y)];
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let hovered = pointer.and_then(node_at);

    // Interaction.
    if resp.drag_started()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let picked = match c.pick {
            Some(Pick::Node(i)) => Some(i),
            _ => None,
        };
        let handle = [picked, hovered].into_iter().flatten().find(|&i| rects.get(i).is_some_and(|r| handles(*r).iter().any(|h| h.distance(p) < 7.0)));
        c.drag = match (c.tool, handle, node_at(p)) {
            (_, Some(i), _) | (Tool::Connect, _, Some(i)) => Some(Drag::Link { from: i, to: p }),
            (_, None, Some(i)) => {
                c.pick = Some(Pick::Node(i));
                g.nodes.get(i).map(|n| Drag::Move { node: i, grab: p - to_screen(pos2(n.x, n.y)), last: p })
            }
            (Tool::Select, None, None) => Some(Drag::Pan),
            _ => None,
        };
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        match &mut c.drag {
            Some(Drag::Move { node, grab, last }) => {
                let delta = (p - *last) / z;
                *last = p;
                // A group moves with its members.
                let moving = if is_group(*node) { descendants(g, *node) } else { vec![*node] };
                for i in moving {
                    if let Some(n) = g.nodes.get_mut(i) {
                        n.x = form::finite(n.x + delta.x);
                        n.y = form::finite(n.y + delta.y);
                    }
                }
                let _ = grab;
            }
            Some(Drag::Link { to, .. }) => *to = p,
            Some(Drag::Pan) => c.pan += resp.drag_delta(),
            None => {}
        }
    }
    if resp.drag_stopped() {
        match c.drag {
            Some(Drag::Link { from, to }) => {
                if let Some(target) = node_at(to)
                    && (target != from || kind == Kind::State)
                    && let Some(k) = g.connect(from, target, c.edge_kind)
                {
                    if kind == Kind::Architecture
                        && let (Some(a), Some(b)) = (g.nodes.get(from).cloned(), g.nodes.get(target).cloned())
                        && let Some(e) = g.edges.get_mut(k)
                    {
                        // Office-like: the sides facing each other.
                        let side = |a: &Node, b: &Node| {
                            let (dx, dy) = (b.x - a.x, b.y - a.y);
                            if dx.abs() >= dy.abs() {
                                if dx >= 0.0 { "R" } else { "L" }
                            } else if dy >= 0.0 {
                                "B"
                            } else {
                                "T"
                            }
                        };
                        set(&mut e.fields, "fromSide", side(&a, &b));
                        set(&mut e.fields, "toSide", side(&b, &a));
                    }
                    c.pick = Some(Pick::Edge(k));
                }
            }
            Some(Drag::Move { node, .. }) => {
                if kind == Kind::Block
                    && let Some(n) = g.nodes.get_mut(node)
                {
                    snap_block(n);
                } else if let Some(n) = g.nodes.get_mut(node) {
                    n.x = (n.x / 10.0).round() * 10.0;
                    n.y = (n.y / 10.0).round() * 10.0;
                }
                // Dropped in a group: it joins it; dragged out of its group: it leaves it.
                if spec.nodes.iter().any(|k| k.group) {
                    let inside = descendants(g, node);
                    let others = frames(
                        kind,
                        spec,
                        &Graph {
                            nodes: g
                                .nodes
                                .iter()
                                .enumerate()
                                .map(|(j, n)| if inside.contains(&j) { Node { parent: "\u{1}".into(), ..n.clone() } } else { n.clone() })
                                .collect(),
                            ..Default::default()
                        },
                    );
                    let centre = g.nodes.get(node).map(|n| pos2(n.x, n.y));
                    let host = centre.and_then(|ctr| {
                        (0..g.nodes.len()).filter(|j| !inside.contains(j) && is_group(*j) && others.get(*j).is_some_and(|r| r.contains(ctr))).min_by(
                            |a, b| {
                                let area = |i: &usize| others.get(*i).map_or(0.0, |r| r.area());
                                area(a).total_cmp(&area(b))
                            },
                        )
                    });
                    let host_id = host.and_then(|h| g.nodes.get(h)).map(|h| h.id.clone()).unwrap_or_default();
                    if let Some(n) = g.nodes.get_mut(node) {
                        n.parent = host_id;
                    }
                }
            }
            _ => {}
        }
        c.drag = None;
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        if let Tool::Place(id) = c.tool {
            if let Some(k) = spec.node(id) {
                let at = to_canvas(p);
                if let Some(i) = place(kind, g, k, pos2((at.x / 10.0).round() * 10.0, (at.y / 10.0).round() * 10.0)) {
                    if kind == Kind::Block
                        && let Some(n) = g.nodes.get_mut(i)
                    {
                        snap_block(n);
                    }
                    // Placed in a group: it joins it.
                    if let Some(host) = node_at(p).filter(|h| *h != i && is_group(*h)).and_then(|h| g.nodes.get(h)).map(|h| h.id.clone())
                        && let Some(n) = g.nodes.get_mut(i)
                    {
                        n.parent = host;
                    }
                    c.pick = Some(Pick::Node(i));
                    c.focus_text = !k.bare;
                }
            }
            c.tool = Tool::Select;
        } else if let Some(i) = node_at(p) {
            c.pick = Some(Pick::Node(i));
        } else {
            let edge = g.edges.iter().position(|e| match (rects.get(e.from), rects.get(e.to)) {
                (Some(a), Some(b)) => dist_to_segment(p, a.center(), b.center()) < 6.0,
                _ => false,
            });
            c.pick = edge.map(Pick::Edge);
        }
    }
    if resp.double_clicked() && matches!(c.pick, Some(Pick::Node(_))) {
        c.focus_text = true;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Delete)) && ui.ctx().memory(|m| m.focused().is_none()) {
        delete(g, c);
    }

    // Drawing: groups, connectors, shapes.
    let mut order: Vec<usize> = (0..g.nodes.len()).collect();
    order.sort_by(|&a, &b| {
        let area = |i: usize| rects.get(i).map_or(0.0, |r| r.area());
        is_group(b).cmp(&is_group(a)).then(area(b).total_cmp(&area(a)))
    });
    for &i in order.iter().filter(|&&i| is_group(i)) {
        let (Some(n), Some(r)) = (g.nodes.get(i), rects.get(i)) else { continue };
        let picked = c.pick == Some(Pick::Node(i));
        outline(&painter, Look::Group, *r, FILL, Stroke::new(if picked { 2.0 } else { 1.2 }, if picked { BLUE } else { LINE }));
        let label = if n.text.is_empty() { tl!(kind_of(spec, n).map_or("", |k| k.label)).to_string() } else { n.text.clone() };
        painter.text(r.left_top() + vec2(8.0, 6.0), egui::Align2::LEFT_TOP, label, fs(11.5), INK);
    }
    for (k, e) in g.edges.iter().enumerate() {
        let (Some(a), Some(b), Some(ra), Some(rb)) = (g.nodes.get(e.from), g.nodes.get(e.to), rects.get(e.from), rects.get(e.to)) else { continue };
        let color = if c.pick == Some(Pick::Edge(k)) { BLUE } else { INK };
        // Architecture connectors carry their arrowheads as properties.
        let arch;
        let ek = if kind == Kind::Architecture {
            let h = |k: &str| if form::is(&e.fields, k) { Head::Arrow } else { Head::None };
            arch = EdgeKind::new("", "", h("fromArrow"), h("toArrow"));
            Some(&arch)
        } else {
            spec.edge(&e.kind)
        };
        if e.from == e.to {
            // A transition to itself: a loop above the shape.
            let top = ra.center_top();
            painter.add(Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
                [top + vec2(-12.0, 0.0), top + vec2(-30.0, -40.0), top + vec2(30.0, -40.0), top + vec2(12.0, 0.0)],
                false,
                Color32::TRANSPARENT,
                Stroke::new(1.4, color),
            )));
            continue;
        }
        let dir = (rb.center() - ra.center()).normalized();
        if !dir.x.is_finite() {
            continue;
        }
        let (mut pa, mut pb) = (boundary(look_of(spec, a), *ra, dir), boundary(look_of(spec, b), *rb, -dir));
        // Connectors between the same two shapes side by side, not on top of each other.
        let pair = |x: &wordcraft_engine::diagram::graph::Edge| (x.from.min(x.to), x.from.max(x.to)) == (e.from.min(e.to), e.from.max(e.to));
        let total = g.edges.iter().filter(|x| pair(x)).count();
        if total > 1 {
            let nth = g.edges.iter().take(k).filter(|x| pair(x)).count() as f32;
            let canon = if e.from < e.to { dir } else { -dir };
            let shift = vec2(-canon.y, canon.x) * (nth - (total as f32 - 1.0) / 2.0) * 14.0 * z;
            pa += shift;
            pb += shift;
        }
        connector(&painter, ek, pa, pb, color);
        let font = fs(10.5);
        if !e.label.trim().is_empty() {
            let mid = pa + (pb - pa) / 2.0;
            let gal = painter.layout_no_wrap(e.label.clone(), font.clone(), INK);
            let r = Rect::from_center_size(mid, gal.size() + vec2(6.0, 2.0));
            painter.rect_filled(r, 2.0, Color32::WHITE);
            painter.galley(r.min + vec2(3.0, 1.0), gal, INK);
        }
        let (la, lb) = end_labels(kind, e);
        let side = vec2(-dir.y, dir.x) * 9.0;
        for (txt, at) in [(la, pa + dir * 16.0 + side), (lb, pb - dir * 16.0 + side)] {
            if !txt.is_empty() {
                painter.text(at, egui::Align2::CENTER_CENTER, txt, font.clone(), INK);
            }
        }
    }
    for &i in order.iter().filter(|&&i| !is_group(i)) {
        let (Some(n), Some(r)) = (g.nodes.get(i), rects.get(i)) else { continue };
        let picked = c.pick == Some(Pick::Node(i));
        let look = look_of(spec, n);
        let stroke = Stroke::new(if picked { 2.0 } else { 1.2 }, if picked { BLUE } else { LINE });
        outline(&painter, look, *r, FILL, stroke);
        let lines = body_lines(spec, n);
        if look == Look::Table {
            let title = painter.layout(shown(n), fs(12.0), TEXT, r.width() - 10.0);
            painter.galley(pos2(r.center().x - title.size().x / 2.0, r.top() + 6.0), title, TEXT);
            if !lines.is_empty() {
                let y = r.top() + 26.0 * z;
                painter.line_segment([pos2(r.left(), y), pos2(r.right(), y)], stroke);
                for (k, l) in lines.iter().enumerate() {
                    painter.text(pos2(r.left() + 7.0 * z, y + (4.0 + k as f32 * 16.0) * z), egui::Align2::LEFT_TOP, l, fs(10.5), TEXT);
                }
            }
        } else if !matches!(look, Look::Dot | Look::Bullseye | Look::Junction | Look::Bar) {
            let gal = painter.layout(shown(n), fs(12.0), TEXT, r.width() - 12.0);
            let y = if look == Look::Person { r.bottom() - 20.0 * z - gal.size().y / 2.0 } else { r.center().y - gal.size().y / 2.0 };
            painter.galley(pos2(r.center().x - gal.size().x / 2.0, y), gal, TEXT);
        }
    }
    for &i in &order {
        if (c.pick == Some(Pick::Node(i)) || hovered == Some(i))
            && let Some(r) = rects.get(i)
        {
            for h in handles(*r) {
                painter.circle(h, 4.0, Color32::WHITE, Stroke::new(1.2, BLUE));
            }
        }
    }
    if let Some(Drag::Link { from, to }) = c.drag
        && let Some(r) = rects.get(from)
    {
        painter.extend(Shape::dashed_line(&[r.center(), to], Stroke::new(1.4, BLUE), 4.0, 3.0));
    }
    if g.nodes.is_empty() {
        painter.text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            tl!("Pick a shape above, then click here to place it."),
            egui::FontId::proportional(13.0),
            Color32::from_gray(140),
        );
    }
    ui.horizontal(|ui| {
        if kind != Kind::Block && ui.button(tl!("Arrange")).on_hover_text(tl!("Place the shapes step by step along the arrows")).clicked() {
            let across = g.direction().is_none_or(|d| matches!(d, "LR" | "RL"));
            g.place(across);
            c.zoom = 0.0;
        }
        ui.separator();
        if ui.button("−").on_hover_text(tl!("Zoom Out")).clicked() {
            c.zoom = (z / 1.25).max(0.25);
        }
        ui.label(format!("{:.0} %", z * 100.0));
        if ui.button("+").on_hover_text(tl!("Zoom In")).clicked() {
            c.zoom = (z * 1.25).min(2.0);
        }
        if ui.button(tl!("Fit")).on_hover_text(tl!("Show the whole drawing")).clicked() {
            c.zoom = 0.0;
        }
        if ui.add_enabled(c.pick.is_some(), egui::Button::new(tl!("Delete"))).clicked() {
            delete(g, c);
        }
    });
}

/// Zoom and scroll the canvas so the whole drawing shows, at most at full size.
fn fit(kind: Kind, spec: &GraphSpec, g: &Graph, c: &mut Canvas, area: Rect) {
    let rects = frames(kind, spec, g);
    let Some(first) = rects.first() else {
        (c.zoom, c.pan) = (1.0, Vec2::ZERO);
        return;
    };
    let b = rects.iter().fold(*first, |a, r| a.union(*r)).expand(24.0);
    let z = (area.width() / b.width().max(1.0)).min(area.height() / b.height().max(1.0)).clamp(0.3, 1.0);
    c.zoom = z;
    // The drawing's centre at the canvas's centre.
    c.pan = area.size() / 2.0 - b.center().to_vec2() * z;
}

fn delete(g: &mut Graph, c: &mut Canvas) {
    match c.pick.take() {
        Some(Pick::Node(i)) => g.remove_node(i),
        Some(Pick::Edge(k)) if k < g.edges.len() => {
            g.edges.remove(k);
        }
        _ => {}
    }
}

/// The panel beside the canvas: the selected shape's or connector's properties, else the
/// diagram's own.
pub fn properties(ui: &mut Ui, kind: Kind, spec: &GraphSpec, g: &mut Graph, c: &mut Canvas) {
    match c.pick {
        Some(Pick::Node(i)) if i < g.nodes.len() => {
            let groups: Vec<(String, String)> = g
                .nodes
                .iter()
                .enumerate()
                .filter(|(j, n)| *j != i && kind_of(spec, n).is_some_and(|k| k.group))
                .map(|(_, n)| (n.id.clone(), if n.text.is_empty() { n.id.clone() } else { n.text.clone() }))
                .collect();
            let Some(n) = g.nodes.get_mut(i) else { return };
            let nk = kind_of(spec, n).copied();
            ui.strong(tl!(nk.map_or("Shape", |k| k.label)));
            egui::Grid::new("diagram_node_props").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                if spec.nodes.len() > 1 {
                    ui.label(tl!("Shape"));
                    egui::ComboBox::from_id_salt("diagram_node_kind").width(170.0).selected_text(tl!(nk.map_or("", |k| k.label))).show_ui(ui, |ui| {
                        for k in spec.nodes {
                            if ui.selectable_label(n.kind == k.id, tl!(k.label)).clicked() {
                                n.kind = k.id.to_string();
                            }
                        }
                    });
                    ui.end_row();
                }
                if !nk.is_some_and(|k| k.bare) {
                    ui.label(tl!(match kind {
                        Kind::Class | Kind::Requirement => "Name",
                        _ => "Text",
                    }));
                    let r = ui.add(egui::TextEdit::singleline(&mut n.text).desired_width(180.0).char_limit(200));
                    if c.focus_text {
                        r.request_focus();
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                            let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n.text.chars().count()));
                            state.cursor.set_char_range(Some(all));
                            state.store(ui.ctx(), r.id);
                        }
                        c.focus_text = false;
                    }
                    ui.end_row();
                }
                for f in nk.map_or(&[][..], |k| k.fields) {
                    map_field(ui, f, &mut n.fields, ("node", i));
                }
                if !groups.is_empty() || !n.parent.is_empty() {
                    ui.label(tl!("In Group"));
                    let current = groups.iter().find(|(id, _)| *id == n.parent).map_or_else(|| tl!("None").to_string(), |(_, l)| l.clone());
                    egui::ComboBox::from_id_salt("diagram_node_group").width(170.0).selected_text(current).show_ui(ui, |ui| {
                        ui.selectable_value(&mut n.parent, String::new(), tl!("None"));
                        for (id, l) in &groups {
                            ui.selectable_value(&mut n.parent, id.clone(), l);
                        }
                    });
                    ui.end_row();
                }
            });
            if let Some(label) = nk.and_then(|k| k.lines) {
                ui.label(tl!(label));
                let mut text = n.lines.join("\n");
                if ui.add(egui::TextEdit::multiline(&mut text).desired_width(280.0).desired_rows(5).char_limit(8000)).changed() {
                    n.lines = text.lines().map(str::to_string).take(wordcraft_engine::diagram::graph::MAX_LINES).collect();
                }
            }
        }
        Some(Pick::Edge(k)) if k < g.edges.len() => {
            let Some(e) = g.edges.get_mut(k) else { return };
            let ek = spec.edge(&e.kind).copied();
            ui.strong(tl!(ek.map_or("Connector", |k| k.label)));
            egui::Grid::new("diagram_edge_props").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                if spec.edges.len() > 1 {
                    ui.label(tl!("Type"));
                    egui::ComboBox::from_id_salt("diagram_edge_type")
                        .width(170.0)
                        .selected_text(tl!(ek.map_or(e.kind.as_str(), |k| k.label)))
                        .show_ui(ui, |ui| {
                            for kd in spec.edges {
                                if ui.selectable_label(e.kind == kd.id, tl!(kd.label)).clicked() {
                                    e.kind = kd.id.to_string();
                                }
                            }
                        });
                    ui.end_row();
                }
                if spec.edge_label {
                    ui.label(tl!("Label"));
                    ui.add(egui::TextEdit::singleline(&mut e.label).desired_width(180.0).char_limit(200));
                    ui.end_row();
                }
                for f in ek.map_or(&[][..], |k| k.fields) {
                    map_field(ui, f, &mut e.fields, ("edge", k));
                }
            });
            if ui.button(tl!("Reverse Direction")).clicked() {
                std::mem::swap(&mut e.from, &mut e.to);
            }
        }
        _ => {
            if spec.fields.is_empty() {
                ui.weak(tl!("Select a shape or a connector to change it."));
            } else {
                ui.strong(tl!(kind.label()));
                egui::Grid::new("diagram_graph_props").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                    for f in spec.fields {
                        map_field(ui, f, &mut g.fields, ("diagram", 0));
                    }
                });
                ui.weak(tl!("Select a shape or a connector to change it."));
            }
        }
    }
}

/// One labelled field of a map of fields, as a grid row.
fn map_field(ui: &mut Ui, f: &FieldSpec, m: &mut std::collections::BTreeMap<String, String>, salt: (&str, usize)) {
    ui.label(tl!(f.label));
    let mut v = get(m, f.key).to_string();
    if crate::diagram_editor::field(ui, f, &mut v, &[], (salt.0, salt.1, f.key), 180.0) {
        set(m, f.key, v);
    }
    ui.end_row();
}
