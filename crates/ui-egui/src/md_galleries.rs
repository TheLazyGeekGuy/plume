//! Plume: the Insert › Markdown menus as galleries. Each callout is shown as it looks on the
//! page, with what it is for; each kind of diagram as a picture of its template.

use egui::{Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::diagram::codec::Kind;

use crate::WordApp;
use crate::theme::c32;

/// A callout kind: its label, what it is for, and how its Markdown is written.
struct Callout {
    kind: &'static str,
    label: &'static str,
    purpose: &'static str,
    syntax: &'static str,
}

const CALLOUTS: [Callout; 8] = [
    Callout { kind: "note", label: "Note", purpose: "Something worth knowing, even when skimming", syntax: "> [!NOTE]" },
    Callout { kind: "tip", label: "Tip", purpose: "Advice for doing it better or faster", syntax: "::: tip" },
    Callout { kind: "important", label: "Important", purpose: "What the reader must know to succeed", syntax: "> [!IMPORTANT]" },
    Callout { kind: "warning", label: "Warning", purpose: "A point that needs care", syntax: "::: warning" },
    Callout { kind: "caution", label: "Caution", purpose: "A risk, or what goes wrong if ignored", syntax: "> [!CAUTION]" },
    Callout { kind: "info", label: "Info", purpose: "Extra information, for those who want it", syntax: "::: info" },
    Callout { kind: "danger", label: "Danger", purpose: "An action that harms or can't be undone", syntax: "::: danger" },
    Callout { kind: "details", label: "Details", purpose: "Folded text the reader opens with a click", syntax: "::: details" },
];

/// Size of a callout's card.
const CARD: egui::Vec2 = vec2(250.0, 66.0);

/// The Callout menu: every kind, drawn as on the page, two by row.
pub fn callouts(ui: &mut Ui, app: &mut WordApp) {
    let enabled = crate::widgets::enabled(app, "markdown.callout");
    egui::Grid::new("callout_gallery").spacing(vec2(8.0, 8.0)).show(ui, |ui| {
        for (i, c) in CALLOUTS.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(CARD, if enabled { Sense::click() } else { Sense::hover() });
            let label = tl!(c.label).to_string();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
            paint_callout(ui, rect, c, resp.hovered() && enabled);
            let resp = resp.on_hover_text(format!("{}\n{}", tl!(c.purpose), c.syntax));
            if resp.clicked() {
                crate::widgets::run_from_ribbon(app, "markdown.callout", json!({"kind": c.kind}));
                ui.close();
            }
            if i % 2 == 1 {
                ui.end_row();
            }
        }
    });
}

/// A callout as the page draws it: its tint, its rule on the left, its sign and title in its
/// colour, and what it is for as its text.
fn paint_callout(ui: &Ui, rect: Rect, c: &Callout, hovered: bool) {
    let (rule, tint) = wordcraft_doc::props::container_colors(c.kind);
    let (rule, tint) = (c32(rule), c32(tint));
    let p = ui.painter();
    p.rect_filled(rect, 4.0, tint);
    p.rect_filled(Rect::from_min_size(rect.min, vec2(4.0, rect.height())), egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, rule);
    if hovered {
        p.rect_stroke(rect, 4.0, Stroke::new(1.5, ui.visuals().selection.stroke.color), StrokeKind::Outside);
    }
    let sign = Rect::from_center_size(pos2(rect.left() + 20.0, rect.top() + 18.0), vec2(14.0, 14.0));
    paint_sign(p, sign, c.kind, rule);
    p.text(pos2(rect.left() + 34.0, sign.center().y), egui::Align2::LEFT_CENTER, tl!(c.label), FontId::proportional(14.0), rule);
    // The page is white in both themes, so its text is dark.
    let text = Color32::from_rgb(0x3C, 0x41, 0x47);
    let galley = p.layout(tl!(c.purpose).to_string(), FontId::proportional(12.0), text, rect.width() - 24.0);
    p.galley(pos2(rect.left() + 12.0, rect.top() + 32.0), galley, text);
}

/// The sign before a callout's title, drawn in its colour.
fn paint_sign(p: &egui::Painter, r: Rect, kind: &str, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let c = r.center();
    let mark = |p: &egui::Painter, ch: &str| {
        p.text(c + vec2(0.0, 0.5), egui::Align2::CENTER_CENTER, ch, FontId::proportional(10.5), color);
    };
    match kind {
        "warning" => {
            let tri = vec![pos2(c.x, r.top()), pos2(r.right(), r.bottom()), pos2(r.left(), r.bottom())];
            p.add(egui::Shape::closed_line(tri, stroke));
            p.text(c + vec2(0.0, 2.0), egui::Align2::CENTER_CENTER, "!", FontId::proportional(9.5), color);
        }
        "danger" => {
            // An octagon, as on a stop sign.
            let pts = (0..8)
                .map(|i| {
                    let a = std::f32::consts::FRAC_PI_8 + i as f32 * std::f32::consts::FRAC_PI_4;
                    c + vec2(a.cos(), a.sin()) * r.width() / 2.0
                })
                .collect();
            p.add(egui::Shape::closed_line(pts, stroke));
            mark(p, "!");
        }
        "tip" => {
            // A check mark in a circle.
            p.circle_stroke(c, r.width() / 2.0, stroke);
            p.line_segment([c + vec2(-3.5, 0.0), c + vec2(-1.0, 2.8)], stroke);
            p.line_segment([c + vec2(-1.0, 2.8), c + vec2(3.8, -2.6)], stroke);
        }
        "details" => {
            // The fold's arrow.
            let tri = vec![pos2(r.left() + 3.0, r.top() + 2.0), pos2(r.right() - 2.0, c.y), pos2(r.left() + 3.0, r.bottom() - 2.0)];
            p.add(egui::Shape::convex_polygon(tri, color, Stroke::NONE));
        }
        "important" | "caution" => {
            p.circle_stroke(c, r.width() / 2.0, stroke);
            mark(p, "!");
        }
        _ => {
            p.circle_stroke(c, r.width() / 2.0, stroke);
            mark(p, "i");
        }
    }
}

/// The Diagram menu: the groups as tabs, and a picture of each kind in the one shown.
pub fn diagrams(ui: &mut Ui, app: &mut WordApp) {
    use crate::diagram_editor::DiagramEditor;
    let id = egui::Id::new("diagram_menu_group");
    let mut category: &'static str = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(Kind::CATEGORIES[0]);
    let picked = crate::diagram_editor::kind_grid(ui, &mut category, vec2(150.0, 84.0), 540.0, 420.0, "diagram_menu_grid", false);
    ui.ctx().data_mut(|d| d.insert_temp(id, category));
    if let Some(kind) = picked {
        app.dialog = Some(crate::dialogs::Dialog::Diagram(Box::new(DiagramEditor::new(kind))));
        ui.close();
    }
    ui.separator();
    if ui.button(tl!("All Diagrams…")).clicked() {
        app.dialog = Some(crate::dialogs::Dialog::Diagram(Box::new(DiagramEditor::gallery())));
        ui.close();
    }
}
