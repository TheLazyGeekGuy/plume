//! Plume: Insert › Markdown › Diagram, drawn rather than written. A diagram is made as in
//! Office: shapes placed on a canvas and joined by connectors, a sequence of messages between
//! people filled in as a table, a chart from its values. Plume writes each as Mermaid in the
//! Markdown file (`markdown.diagram`); the person never has to read or type Mermaid. Diagrams
//! of other kinds (Gantt, class, state…) found in a file keep their text, edited as text.

use egui::{Color32, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use serde_json::json;
use wordcraft_doc::chart::{ChartSeries, ChartSpec, ChartType};
use wordcraft_engine::diagram::mermaid;
use wordcraft_engine::diagram::sequence::{MAX_MESSAGES, MAX_PARTICIPANTS, Message, Sequence};
use wordcraft_engine::diagram::sketch::{LineStyle, Node, NodeShape, Sketch};

use crate::WordApp;

/// What the editor edits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// Shapes and connectors (a Mermaid flowchart).
    #[default]
    Shapes,
    Sequence,
    Chart,
    /// A diagram no editor shows: its text.
    Text,
}

/// What a click on the canvas does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Select,
    Place(NodeShape),
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
    /// A shape moved, held at `grab` from its centre.
    Move {
        node: usize,
        grab: Vec2,
    },
    /// A connector drawn from a shape to the pointer.
    Link {
        from: usize,
        to: Pos2,
    },
    Pan,
}

/// The chart editor's fields: values as typed.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartForm {
    pub kind: usize,
    pub title: String,
    pub series: Vec<String>,
    /// Category, then one value per series.
    pub rows: Vec<(String, Vec<String>)>,
}

/// The chart kinds the editor offers (Mermaid draws these).
const CHART_KINDS: [(&str, ChartType); 4] =
    [("Pie", ChartType::Pie), ("Column", ChartType::Column), ("Bar", ChartType::Bar), ("Line", ChartType::Line)];

impl Default for ChartForm {
    fn default() -> Self {
        ChartForm {
            kind: 0,
            title: "Time spent".into(),
            series: vec!["Hours".into()],
            rows: vec![("Writing".into(), vec!["60".into()]), ("Review".into(), vec!["25".into()]), ("Layout".into(), vec!["15".into()])],
        }
    }
}

impl ChartForm {
    fn of(spec: &ChartSpec) -> ChartForm {
        let kind = CHART_KINDS.iter().position(|(_, k)| *k == spec.kind).unwrap_or(1);
        let num = |v: Option<f64>| v.map(|v| if v.fract() == 0.0 && v.abs() < 1e15 { format!("{v:.0}") } else { v.to_string() }).unwrap_or_default();
        ChartForm {
            kind,
            title: spec.title.clone().unwrap_or_default(),
            series: spec.series.iter().map(|s| s.name.clone()).collect(),
            rows: spec
                .categories
                .iter()
                .enumerate()
                .map(|(i, c)| (c.clone(), spec.series.iter().map(|s| num(s.values.get(i).copied().flatten())).collect()))
                .collect(),
        }
    }

    fn spec(&self) -> ChartSpec {
        let kind = CHART_KINDS.get(self.kind).map_or(ChartType::Column, |(_, k)| *k);
        let n = if kind == ChartType::Pie { 1 } else { self.series.len().max(1) };
        ChartSpec {
            kind,
            categories: self.rows.iter().map(|(c, _)| c.trim().to_string()).collect(),
            series: (0..n)
                .map(|k| ChartSeries {
                    name: self.series.get(k).cloned().unwrap_or_default(),
                    values: self
                        .rows
                        .iter()
                        .map(|(_, v)| v.get(k).and_then(|x| x.trim().replace(',', ".").parse::<f64>().ok()).filter(|f| f.is_finite()))
                        .collect(),
                })
                .collect(),
            title: Some(self.title.trim().to_string()).filter(|t| !t.is_empty()),
            ..Default::default()
        }
    }
}

/// The diagram editor's state.
#[derive(Clone, Debug)]
pub struct DiagramEditor {
    pub kind: Kind,
    pub sketch: Sketch,
    pub seq: Sequence,
    pub chart: ChartForm,
    /// A diagram of another kind, as text.
    pub text: String,
    pub tool: Tool,
    pub pick: Option<Pick>,
    drag: Option<Drag>,
    pan: Vec2,
    /// The selected shape's text field takes the keyboard.
    focus_text: bool,
    /// Editing a diagram already in the document (Apply rather than Insert).
    pub editing: bool,
    pub error: String,
}

impl Default for DiagramEditor {
    fn default() -> Self {
        DiagramEditor {
            kind: Kind::Shapes,
            sketch: Sketch::default(),
            seq: Sequence {
                participants: vec!["Author".into(), "Reviewer".into()],
                messages: vec![
                    Message { from: 0, to: 1, text: "Request review".into(), reply: false },
                    Message { from: 1, to: 0, text: "Comments".into(), reply: true },
                ],
            },
            chart: ChartForm::default(),
            text: String::new(),
            tool: Tool::Place(NodeShape::Rectangle),
            pick: None,
            drag: None,
            pan: Vec2::ZERO,
            focus_text: false,
            editing: false,
            error: String::new(),
        }
    }
}

impl DiagramEditor {
    /// A new diagram of `kind`.
    pub fn new(kind: Kind) -> DiagramEditor {
        DiagramEditor { kind, ..Default::default() }
    }

    /// The editor for a diagram's source: the editor that shows it, else its text.
    pub fn open(source: &str) -> DiagramEditor {
        let mut e = DiagramEditor { editing: true, tool: Tool::Select, ..Default::default() };
        if let Some(sk) = Sketch::parse(source) {
            e.kind = Kind::Shapes;
            e.sketch = sk;
        } else if let Some(seq) = Sequence::parse(source) {
            e.kind = Kind::Sequence;
            e.seq = seq;
        } else if let Some(spec) = mermaid::chart_of(source) {
            e.kind = Kind::Chart;
            e.chart = ChartForm::of(&spec);
        } else {
            e.kind = Kind::Text;
            e.text = source.to_string();
        }
        e
    }

    /// The diagram as the document gets it (Mermaid), or why there is none yet.
    pub fn source(&self) -> Result<String, &'static str> {
        match self.kind {
            Kind::Shapes if self.sketch.nodes.is_empty() => Err("Draw at least one shape."),
            Kind::Shapes => Ok(self.sketch.to_mermaid()),
            Kind::Sequence if self.seq.participants.iter().all(|p| p.trim().is_empty()) => Err("Add at least one participant."),
            Kind::Sequence => Ok(self.seq.to_mermaid()),
            Kind::Chart => mermaid::from_chart(&self.chart.spec()).ok_or("Enter at least one value above zero."),
            Kind::Text => Ok(self.text.clone()),
        }
    }
}

/// The editor's body; `true` when the dialog closes.
pub fn show(app: &mut WordApp, ui: &mut Ui, e: &mut DiagramEditor) -> bool {
    // Enter in a text field ends the typing, not the dialog.
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if e.kind != Kind::Text {
        ui.horizontal(|ui| {
            for (kind, label) in [(Kind::Shapes, "Shapes and Connectors"), (Kind::Sequence, "Sequence"), (Kind::Chart, "Chart")] {
                if ui.selectable_label(e.kind == kind, tl!(label)).clicked() {
                    e.kind = kind;
                    e.error.clear();
                }
            }
        });
        ui.separator();
    }
    let source = e.source();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| match e.kind {
            Kind::Shapes => shapes(ui, e),
            Kind::Sequence => sequence(ui, e),
            Kind::Chart => chart(ui, e),
            Kind::Text => {
                ui.label(tl!("This kind of diagram has no visual editor yet; its text is kept as written."));
                ui.add(egui::TextEdit::multiline(&mut e.text).code_editor().desired_width(420.0).desired_rows(16).char_limit(64 * 1024));
            }
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_width(300.0);
            if e.kind == Kind::Shapes {
                properties(ui, e);
                ui.add_space(8.0);
            }
            ui.label(tl!("Preview"));
            match &source {
                Ok(src) => match crate::dialogs::diagram_preview(ui.ctx(), src) {
                    Ok(tex) => {
                        ui.add(egui::Image::new(&tex).max_size(vec2(300.0, 260.0)).bg_fill(Color32::WHITE));
                    }
                    Err(err) => {
                        ui.colored_label(ui.visuals().warn_fg_color, err);
                    }
                },
                Err(why) => {
                    ui.weak(tl!(why));
                }
            }
            if e.kind == Kind::Shapes {
                ui.add_space(4.0);
                ui.weak(tl!("Plume lays the diagram out; the canvas sets the order and direction."));
            }
        });
    });
    if !e.error.is_empty() {
        ui.colored_label(ui.visuals().error_fg_color, e.error.as_str());
    }
    let (ok, cancel) = crate::dialogs::buttons(ui, if e.editing { tl!("Apply") } else { tl!("Insert") });
    if cancel {
        return true;
    }
    if ok && !(typing && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
        match e.source() {
            Ok(src) => match app.run("markdown.diagram", json!({"source": src})) {
                Ok(_) => return true,
                Err(err) => e.error = err,
            },
            Err(why) => e.error = tl!(why).to_string(),
        }
    }
    false
}

/// A shape's size on the canvas.
fn node_size(n: &Node) -> Vec2 {
    let w = (n.text.chars().count() as f32 * 7.0 + 28.0).clamp(72.0, 200.0);
    match n.shape {
        NodeShape::Circle => Vec2::splat(w.min(110.0)),
        NodeShape::Diamond => vec2(w + 36.0, 64.0),
        NodeShape::Hexagon | NodeShape::Parallelogram => vec2(w + 20.0, 44.0),
        _ => vec2(w, 44.0),
    }
}

/// The point where the line from `n`'s centre `c` towards `dir` (a unit vector) leaves it.
fn boundary(n: &Node, c: Pos2, dir: Vec2) -> Pos2 {
    let h = node_size(n) / 2.0;
    let (dx, dy) = (dir.x.abs().max(1e-4), dir.y.abs().max(1e-4));
    let t = match n.shape {
        NodeShape::Circle => h.x,
        NodeShape::Diamond => 1.0 / (dx / h.x + dy / h.y),
        _ => (h.x / dx).min(h.y / dy),
    };
    c + dir * t
}

/// The outline of shape `kind` filling `r`.
fn outline(p: &Painter, kind: NodeShape, r: Rect, fill: Color32, stroke: Stroke) {
    let (c, w, h) = (r.center(), r.width(), r.height());
    let poly = |pts: Vec<Pos2>| Shape::convex_polygon(pts, fill, stroke);
    match kind {
        NodeShape::Rectangle => {
            p.rect(r, 0.0, fill, stroke, egui::StrokeKind::Inside);
        }
        NodeShape::Rounded => {
            p.rect(r, (h / 4.0).min(8.0), fill, stroke, egui::StrokeKind::Inside);
        }
        NodeShape::Stadium => {
            p.rect(r, h / 2.0, fill, stroke, egui::StrokeKind::Inside);
        }
        NodeShape::Circle => {
            p.circle(c, w.min(h) / 2.0, fill, stroke);
        }
        NodeShape::Diamond => {
            p.add(poly(vec![pos2(c.x, r.top()), pos2(r.right(), c.y), pos2(c.x, r.bottom()), pos2(r.left(), c.y)]));
        }
        NodeShape::Hexagon => {
            let k = (h / 2.0).min(w / 4.0);
            p.add(poly(vec![
                pos2(r.left() + k, r.top()),
                pos2(r.right() - k, r.top()),
                pos2(r.right(), c.y),
                pos2(r.right() - k, r.bottom()),
                pos2(r.left() + k, r.bottom()),
                pos2(r.left(), c.y),
            ]));
        }
        NodeShape::Parallelogram => {
            let k = (h / 2.0).min(w / 4.0);
            p.add(poly(vec![pos2(r.left() + k, r.top()), pos2(r.right(), r.top()), pos2(r.right() - k, r.bottom()), pos2(r.left(), r.bottom())]));
        }
        NodeShape::Database => {
            let k = (h / 6.0).max(2.0);
            p.rect(
                Rect::from_min_max(pos2(r.left(), r.top() + k), pos2(r.right(), r.bottom() - k)),
                0.0,
                fill,
                Stroke::NONE,
                egui::StrokeKind::Inside,
            );
            p.add(Shape::ellipse_filled(pos2(c.x, r.bottom() - k), vec2(w / 2.0, k), fill));
            p.add(Shape::ellipse_stroke(pos2(c.x, r.bottom() - k), vec2(w / 2.0, k), stroke));
            p.line_segment([pos2(r.left(), r.top() + k), pos2(r.left(), r.bottom() - k)], stroke);
            p.line_segment([pos2(r.right(), r.top() + k), pos2(r.right(), r.bottom() - k)], stroke);
            p.add(Shape::ellipse_filled(pos2(c.x, r.top() + k), vec2(w / 2.0, k), fill));
            p.add(Shape::ellipse_stroke(pos2(c.x, r.top() + k), vec2(w / 2.0, k), stroke));
        }
    }
}

/// An arrow from `a` to `b`, drawn as a connector of `style`.
fn connector(p: &Painter, a: Pos2, b: Pos2, style: LineStyle, arrow: bool, color: Color32) {
    let width = if style == LineStyle::Thick { 3.0 } else { 1.4 };
    let stroke = Stroke::new(width, color);
    let dir = (b - a).normalized();
    let end = if arrow { b - dir * 8.0 } else { b };
    if style == LineStyle::Dotted {
        p.extend(Shape::dashed_line(&[a, end], stroke, 4.0, 3.0));
    } else {
        p.line_segment([a, end], stroke);
    }
    if arrow {
        let n = vec2(-dir.y, dir.x) * 5.0;
        p.add(Shape::convex_polygon(vec![b, b - dir * 10.0 + n, b - dir * 10.0 - n], color, Stroke::NONE));
    }
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// The shape palette: Select, the shapes, Connector.
fn palette(ui: &mut Ui, e: &mut DiagramEditor) {
    ui.horizontal(|ui| {
        let ink = ui.visuals().text_color();
        let tool_button = |ui: &mut Ui, on: bool, tip: &str, draw: &dyn Fn(&Painter, Rect)| {
            let r = ui.add(egui::Button::new("").min_size(vec2(30.0, 26.0)).selected(on)).on_hover_text(tl!(tip));
            let inner = r.rect.shrink2(vec2(7.0, 7.0));
            draw(ui.painter(), inner);
            r.clicked()
        };
        if tool_button(ui, e.tool == Tool::Select, "Select and Move", &|p, r| {
            p.add(Shape::convex_polygon(
                vec![r.left_top(), pos2(r.left(), r.bottom()), pos2(r.center().x, r.center().y + 2.0), r.right_bottom() - vec2(0.0, 4.0)],
                ink,
                Stroke::NONE,
            ));
        }) {
            e.tool = Tool::Select;
        }
        ui.separator();
        for shape in NodeShape::ALL {
            if tool_button(ui, e.tool == Tool::Place(shape), shape.label(), &|p, r| outline(p, shape, r, Color32::TRANSPARENT, Stroke::new(1.3, ink)))
            {
                e.tool = Tool::Place(shape);
            }
        }
        ui.separator();
        if tool_button(ui, e.tool == Tool::Connect, "Connector", &|p, r| connector(p, r.left_bottom(), r.right_top(), LineStyle::Solid, true, ink)) {
            e.tool = Tool::Connect;
        }
    });
}

/// The canvas: shapes and connectors drawn and moved by hand.
fn shapes(ui: &mut Ui, e: &mut DiagramEditor) {
    palette(ui, e);
    let hint = match e.tool {
        Tool::Select => "Drag a shape to move it; drag from a blue handle to another shape to connect them.",
        Tool::Place(_) => "Click in the canvas to place the shape.",
        Tool::Connect => "Drag from one shape to another to connect them.",
    };
    ui.weak(tl!(hint));
    let (resp, painter) = ui.allocate_painter(vec2(540.0, 380.0), Sense::click_and_drag());
    let area = resp.rect;
    let painter = painter.with_clip_rect(area);
    let t = crate::theme::Tokens::get(ui.ctx());
    painter.rect_filled(area, 2.0, Color32::WHITE);
    painter.rect_stroke(area, 2.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    // A light grid, as Office's drawing canvas has.
    let mut x = area.left() + (e.pan.x.rem_euclid(20.0));
    while x < area.right() {
        let mut y = area.top() + (e.pan.y.rem_euclid(20.0));
        while y < area.bottom() {
            painter.circle_filled(pos2(x, y), 0.8, Color32::from_gray(205));
            y += 20.0;
        }
        x += 20.0;
    }
    let origin = area.min.to_vec2() + e.pan;
    let at = |n: &Node| pos2(n.x, n.y) + origin;
    let node_rect = |n: &Node| Rect::from_center_size(at(n), node_size(n));
    let handles = |n: &Node| {
        let r = node_rect(n);
        [pos2(r.center().x, r.top()), pos2(r.right(), r.center().y), pos2(r.center().x, r.bottom()), pos2(r.left(), r.center().y)]
    };
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let node_at = |sk: &Sketch, p: Pos2| sk.nodes.iter().rposition(|n| node_rect(n).expand(2.0).contains(p));
    let hovered = pointer.and_then(|p| node_at(&e.sketch, p));

    // Interaction.
    if resp.drag_started()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let handle = [e.pick.and_then(|k| if let Pick::Node(i) = k { Some(i) } else { None }), hovered]
            .into_iter()
            .flatten()
            .find(|&i| e.sketch.nodes.get(i).is_some_and(|n| handles(n).iter().any(|h| h.distance(p) < 7.0)));
        e.drag = match (e.tool, handle, node_at(&e.sketch, p)) {
            (_, Some(i), _) | (Tool::Connect, _, Some(i)) => Some(Drag::Link { from: i, to: p }),
            (_, None, Some(i)) => {
                e.pick = Some(Pick::Node(i));
                e.sketch.nodes.get(i).map(|n| Drag::Move { node: i, grab: p - at(n) })
            }
            (Tool::Select, None, None) => Some(Drag::Pan),
            _ => None,
        };
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        match &mut e.drag {
            Some(Drag::Move { node, grab }) => {
                if let Some(n) = e.sketch.nodes.get_mut(*node) {
                    let c = p - *grab - origin;
                    // Snapped to the grid.
                    (n.x, n.y) = (((c.x / 10.0).round() * 10.0).clamp(-5000.0, 5000.0), ((c.y / 10.0).round() * 10.0).clamp(-5000.0, 5000.0));
                }
            }
            Some(Drag::Link { to, .. }) => *to = p,
            Some(Drag::Pan) => e.pan += resp.drag_delta(),
            None => {}
        }
    }
    if resp.drag_stopped() {
        if let Some(Drag::Link { from, to }) = e.drag
            && let Some(target) = node_at(&e.sketch, to)
            && let Some(k) = e.sketch.connect(from, target)
        {
            e.pick = Some(Pick::Edge(k));
        }
        e.drag = None;
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        if let Tool::Place(shape) = e.tool {
            let c = p - origin;
            if let Some(i) = e.sketch.add(shape, tl!(shape.label()), (c.x / 10.0).round() * 10.0, (c.y / 10.0).round() * 10.0) {
                e.pick = Some(Pick::Node(i));
                e.focus_text = true;
                e.tool = Tool::Select;
            }
        } else if let Some(i) = node_at(&e.sketch, p) {
            e.pick = Some(Pick::Node(i));
        } else {
            let edge = e.sketch.edges.iter().position(|ed| match (e.sketch.nodes.get(ed.from), e.sketch.nodes.get(ed.to)) {
                (Some(a), Some(b)) => dist_to_segment(p, at(a), at(b)) < 6.0,
                _ => false,
            });
            e.pick = edge.map(Pick::Edge);
        }
    }
    if resp.double_clicked()
        && let Some(Pick::Node(_)) = e.pick
    {
        e.focus_text = true;
    }
    // Delete removes the selection, unless a text field has the keyboard.
    if ui.input(|i| i.key_pressed(egui::Key::Delete)) && ui.ctx().memory(|m| m.focused().is_none()) {
        delete(e);
    }

    // Drawing: connectors under shapes.
    let blue = Color32::from_rgb(0x25, 0x63, 0xEB);
    let ink = Color32::from_rgb(0x33, 0x41, 0x55);
    for (k, ed) in e.sketch.edges.iter().enumerate() {
        let (Some(a), Some(b)) = (e.sketch.nodes.get(ed.from), e.sketch.nodes.get(ed.to)) else { continue };
        let (ca, cb) = (at(a), at(b));
        let dir = (cb - ca).normalized();
        if !dir.x.is_finite() {
            continue;
        }
        let (pa, pb) = (boundary(a, ca, dir), boundary(b, cb, -dir));
        let color = if e.pick == Some(Pick::Edge(k)) { blue } else { ink };
        connector(&painter, pa, pb, ed.line, ed.arrow, color);
        if !ed.label.trim().is_empty() {
            let mid = pa + (pb - pa) / 2.0;
            let g = painter.layout_no_wrap(ed.label.clone(), egui::FontId::proportional(11.0), ink);
            let r = Rect::from_center_size(mid, g.size() + vec2(6.0, 2.0));
            painter.rect_filled(r, 2.0, Color32::WHITE);
            painter.galley(r.min + vec2(3.0, 1.0), g, ink);
        }
    }
    for (i, n) in e.sketch.nodes.iter().enumerate() {
        let r = node_rect(n);
        let picked = e.pick == Some(Pick::Node(i));
        outline(
            &painter,
            n.shape,
            r,
            Color32::from_rgb(0xEC, 0xF2, 0xFE),
            Stroke::new(if picked { 2.0 } else { 1.2 }, if picked { blue } else { Color32::from_rgb(0x93, 0x70, 0xDB) }),
        );
        let g = painter.layout(n.text.clone(), egui::FontId::proportional(12.0), Color32::from_rgb(0x1F, 0x29, 0x37), r.width() - 10.0);
        painter.galley(r.center() - g.size() / 2.0, g, Color32::BLACK);
        if picked || hovered == Some(i) {
            for h in handles(n) {
                painter.circle(h, 4.0, Color32::WHITE, Stroke::new(1.2, blue));
            }
        }
    }
    if let Some(Drag::Link { from, to }) = e.drag
        && let Some(a) = e.sketch.nodes.get(from)
    {
        connector(&painter, at(a), to, LineStyle::Dotted, true, blue);
    }
    if e.sketch.nodes.is_empty() {
        painter.text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            tl!("Pick a shape above, then click here to place it."),
            egui::FontId::proportional(13.0),
            Color32::from_gray(140),
        );
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("Arrange")).on_hover_text(tl!("Place the shapes step by step along the arrows")).clicked() {
            e.sketch.dir = e.sketch.direction();
            e.sketch.place();
            e.pan = Vec2::ZERO;
        }
        if ui.add_enabled(e.pick.is_some(), egui::Button::new(tl!("Delete"))).clicked() {
            delete(e);
        }
    });
}

fn delete(e: &mut DiagramEditor) {
    match e.pick.take() {
        Some(Pick::Node(i)) => e.sketch.remove_node(i),
        Some(Pick::Edge(k)) if k < e.sketch.edges.len() => {
            e.sketch.edges.remove(k);
        }
        _ => {}
    }
}

/// The selected shape's or connector's properties.
fn properties(ui: &mut Ui, e: &mut DiagramEditor) {
    match e.pick {
        Some(Pick::Node(i)) => {
            let Some(n) = e.sketch.nodes.get_mut(i) else { return };
            ui.strong(tl!("Shape"));
            ui.label(tl!("Text"));
            let r = ui.add(egui::TextEdit::singleline(&mut n.text).desired_width(280.0).char_limit(200));
            if e.focus_text {
                r.request_focus();
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                    let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n.text.chars().count()));
                    state.cursor.set_char_range(Some(all));
                    state.store(ui.ctx(), r.id);
                }
                e.focus_text = false;
            }
            egui::ComboBox::from_id_salt("diagram_shape").width(200.0).selected_text(tl!(n.shape.label())).show_ui(ui, |ui| {
                for s in NodeShape::ALL {
                    ui.selectable_value(&mut n.shape, s, tl!(s.label()));
                }
            });
        }
        Some(Pick::Edge(k)) => {
            let Some(ed) = e.sketch.edges.get_mut(k) else { return };
            ui.strong(tl!("Connector"));
            ui.label(tl!("Label"));
            ui.add(egui::TextEdit::singleline(&mut ed.label).desired_width(280.0).char_limit(200));
            ui.checkbox(&mut ed.arrow, tl!("Arrowhead"));
            ui.horizontal(|ui| {
                for (style, label) in [(LineStyle::Solid, "Solid"), (LineStyle::Dotted, "Dotted"), (LineStyle::Thick, "Thick")] {
                    ui.selectable_value(&mut ed.line, style, tl!(label));
                }
            });
            if ui.button(tl!("Reverse Direction")).clicked() {
                std::mem::swap(&mut ed.from, &mut ed.to);
            }
        }
        None => {
            ui.weak(tl!("Select a shape or a connector to change it."));
        }
    }
}

/// A sequence diagram as two tables: participants, then messages.
fn sequence(ui: &mut Ui, e: &mut DiagramEditor) {
    let s = &mut e.seq;
    ui.strong(tl!("Participants"));
    let mut remove = None;
    let many = s.participants.len() > 1;
    for (k, p) in s.participants.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(p).desired_width(220.0).char_limit(100));
            if many && ui.small_button("×").on_hover_text(tl!("Remove")).clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        s.participants.remove(k);
        s.messages.retain(|m| m.from != k && m.to != k);
        for m in &mut s.messages {
            m.from -= usize::from(m.from > k);
            m.to -= usize::from(m.to > k);
        }
    }
    if s.participants.len() < MAX_PARTICIPANTS && ui.button(tl!("Add Participant")).clicked() {
        let n = s.participants.len() + 1;
        s.participants.push(format!("{} {n}", tl!("Participant")));
    }
    ui.add_space(6.0);
    ui.strong(tl!("Messages"));
    let names = s.participants.clone();
    let mut action: Option<(usize, i32)> = None;
    egui::ScrollArea::vertical().id_salt("seq_messages").max_height(220.0).show(ui, |ui| {
        egui::Grid::new("seq_grid").num_columns(5).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
            ui.weak(tl!("From"));
            ui.weak(tl!("To"));
            ui.weak(tl!("Message"));
            ui.weak(tl!("Reply"));
            ui.end_row();
            for (k, m) in s.messages.iter_mut().enumerate() {
                for (salt, who) in [("f", &mut m.from), ("t", &mut m.to)] {
                    egui::ComboBox::from_id_salt(("seq", salt, k)).width(100.0).selected_text(names.get(*who).cloned().unwrap_or_default()).show_ui(
                        ui,
                        |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.selectable_value(who, i, n);
                            }
                        },
                    );
                }
                ui.add_sized(vec2(200.0, 22.0), egui::TextEdit::singleline(&mut m.text).char_limit(200));
                ui.checkbox(&mut m.reply, "");
                ui.horizontal(|ui| {
                    if ui.small_button("↑").clicked() {
                        action = Some((k, -1));
                    }
                    if ui.small_button("×").on_hover_text(tl!("Remove")).clicked() {
                        action = Some((k, 0));
                    }
                });
                ui.end_row();
            }
        });
    });
    match action {
        Some((k, 0)) if k < s.messages.len() => {
            s.messages.remove(k);
        }
        Some((k, _)) if k > 0 && k < s.messages.len() => s.messages.swap(k, k - 1),
        _ => {}
    }
    if s.messages.len() < MAX_MESSAGES && !s.participants.is_empty() && ui.button(tl!("Add Message")).clicked() {
        // The next message answers the last one by default.
        let (from, to) = s.messages.last().map_or((0, 1.min(s.participants.len() - 1)), |m| (m.to, m.from));
        s.messages.push(Message { from, to, text: String::new(), reply: false });
    }
}

/// A chart from its values, typed in a table as in Office's chart data sheet.
fn chart(ui: &mut Ui, e: &mut DiagramEditor) {
    let c = &mut e.chart;
    ui.horizontal(|ui| {
        ui.label(tl!("Chart type"));
        egui::ComboBox::from_id_salt("diagram_chart_kind").selected_text(tl!(CHART_KINDS.get(c.kind).map_or("Column", |k| k.0))).show_ui(ui, |ui| {
            for (k, (label, _)) in CHART_KINDS.iter().enumerate() {
                ui.selectable_value(&mut c.kind, k, tl!(label));
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(tl!("Title"));
        ui.add(egui::TextEdit::singleline(&mut c.title).desired_width(260.0).char_limit(200));
    });
    let pie = c.kind == 0;
    let cols = if pie { 1 } else { c.series.len().max(1) };
    while c.series.len() < cols {
        c.series.push(format!("{} {}", tl!("Series"), c.series.len() + 1));
    }
    let mut remove = None;
    egui::ScrollArea::vertical().id_salt("chart_rows").max_height(250.0).show(ui, |ui| {
        egui::Grid::new("chart_grid").num_columns(cols + 2).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
            ui.weak(tl!("Category"));
            for s in c.series.iter_mut().take(cols) {
                if pie {
                    ui.weak(tl!("Value"));
                } else {
                    ui.add_sized(vec2(80.0, 22.0), egui::TextEdit::singleline(s).char_limit(60));
                }
            }
            ui.end_row();
            for (k, (cat, vals)) in c.rows.iter_mut().enumerate() {
                ui.add_sized(vec2(160.0, 22.0), egui::TextEdit::singleline(cat).char_limit(100));
                vals.resize(c.series.len().max(1), String::new());
                for v in vals.iter_mut().take(cols) {
                    let bad = !v.trim().is_empty() && v.trim().replace(',', ".").parse::<f64>().is_err();
                    let mut te = egui::TextEdit::singleline(v).char_limit(30);
                    if bad {
                        te = te.text_color(ui.visuals().error_fg_color);
                    }
                    ui.add_sized(vec2(80.0, 22.0), te);
                }
                if ui.small_button("×").on_hover_text(tl!("Remove")).clicked() {
                    remove = Some(k);
                }
                ui.end_row();
            }
        });
    });
    if let Some(k) = remove {
        c.rows.remove(k);
    }
    ui.horizontal(|ui| {
        if c.rows.len() < 200 && ui.button(tl!("Add Row")).clicked() {
            c.rows.push((String::new(), vec![String::new(); c.series.len().max(1)]));
        }
        if !pie && c.series.len() < 8 && ui.button(tl!("Add Series")).clicked() {
            c.series.push(format!("{} {}", tl!("Series"), c.series.len() + 1));
        }
        if !pie && c.series.len() > 1 && ui.button(tl!("Remove Series")).clicked() {
            c.series.pop();
            for (_, v) in &mut c.rows {
                v.truncate(c.series.len());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_editor_makes_mermaid_and_opens_it_again() {
        let mut e = DiagramEditor::new(Kind::Shapes);
        assert!(e.source().is_err(), "nothing drawn yet");
        let a = e.sketch.add(NodeShape::Rectangle, "Write", 50.0, 50.0).unwrap();
        let b = e.sketch.add(NodeShape::Diamond, "OK?", 250.0, 50.0).unwrap();
        e.sketch.connect(a, b);
        let src = e.source().unwrap();
        assert_eq!(DiagramEditor::open(&src).kind, Kind::Shapes);
        e.kind = Kind::Sequence;
        assert_eq!(DiagramEditor::open(&e.source().unwrap()).seq, e.seq);
        e.kind = Kind::Chart;
        let pie = DiagramEditor::open(&e.source().unwrap());
        assert_eq!((pie.kind, pie.chart.rows.len()), (Kind::Chart, 3));
        e.chart.kind = 1;
        assert_eq!(DiagramEditor::open(&e.source().unwrap()).chart.kind, 1);
        let gantt = DiagramEditor::open("gantt\n    title Plan");
        assert_eq!((gantt.kind, gantt.source().unwrap().as_str()), (Kind::Text, "gantt\n    title Plan"));
    }
}
