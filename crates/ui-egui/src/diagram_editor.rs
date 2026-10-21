//! Plume: Insert › Markdown › Diagram, drawn rather than written, for every kind of Mermaid
//! diagram. A gallery of the kinds (with a picture of each) starts a new diagram; flowcharts,
//! state, class, entity-relationship, requirement, C4, architecture and block diagrams are
//! drawn on a canvas ([`crate::diagram_canvas`]); the others are filled in as tables
//! ([`crate::diagram_sheet`]). Plume writes the Mermaid (`markdown.diagram`); the person never
//! has to read or type it. A diagram using what an editor can't show says so before anything is
//! dropped, and can still be edited as text.

use egui::{Color32, Ui, vec2};
use serde_json::json;
use wordcraft_engine::diagram::codec::{self, Diagram, Kind, Model, Opened};
use wordcraft_engine::diagram::form::{FieldSpec, FieldType};

use crate::WordApp;
use crate::diagram_canvas::Canvas;

/// What the dialog shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The kinds of diagrams, to start a new one.
    #[default]
    Gallery,
    /// The diagram's editor.
    Visual,
    /// The diagram's text, for what no editor shows.
    Text,
}

/// The diagram editor's state.
#[derive(Clone, Debug, Default)]
pub struct DiagramEditor {
    pub mode: Mode,
    pub diagram: Option<Diagram>,
    /// Opened from a source the editor can't show all of.
    pub partial: bool,
    /// The source it was opened from, and the text edited in [`Mode::Text`].
    pub original: String,
    pub text: String,
    pub canvas: Canvas,
    /// The gallery's group shown (`""`: all).
    pub category: &'static str,
    /// Editing a diagram already in the document (Apply rather than Insert).
    pub editing: bool,
    pub error: String,
}

impl DiagramEditor {
    /// The gallery, for a new diagram.
    pub fn gallery() -> DiagramEditor {
        DiagramEditor::default()
    }

    /// A new diagram of `kind`, from its template.
    pub fn new(kind: Kind) -> DiagramEditor {
        let mut e = DiagramEditor::default();
        e.start(kind);
        e
    }

    fn start(&mut self, kind: Kind) {
        self.diagram = codec::new(kind);
        self.mode = if self.diagram.is_some() { Mode::Visual } else { Mode::Text };
        self.text = kind.template().to_string();
        self.partial = false;
        self.canvas = Canvas::default();
        self.error.clear();
    }

    /// The editor of a diagram's source.
    pub fn open(source: &str) -> DiagramEditor {
        let mut e = DiagramEditor { editing: true, original: source.to_string(), text: source.to_string(), ..Default::default() };
        match codec::open(source) {
            Opened::Visual(d) => {
                e.diagram = Some(d);
                e.mode = Mode::Visual;
            }
            Opened::Partial(d) => {
                e.diagram = Some(d);
                e.mode = Mode::Visual;
                e.partial = true;
            }
            Opened::Text => e.mode = Mode::Text,
        }
        e
    }

    /// The diagram as the document gets it (Mermaid), or why there is none yet.
    pub fn source(&self) -> Result<String, &'static str> {
        match (self.mode, &self.diagram) {
            (Mode::Text, _) => Ok(self.text.clone()),
            (Mode::Visual, Some(d)) => {
                if let Model::Graph(g) = &d.model
                    && g.nodes.is_empty()
                {
                    return Err("Draw at least one shape.");
                }
                Ok(codec::write(d))
            }
            _ => Err("Pick a kind of diagram."),
        }
    }
}

/// One field's editor; whether it changed. `refs`: the values a reference offers (id, label).
pub(crate) fn field(
    ui: &mut Ui,
    f: &FieldSpec,
    v: &mut String,
    refs: &[(String, String)],
    salt: impl std::hash::Hash + std::fmt::Debug,
    width: f32,
) -> bool {
    match f.ty {
        FieldType::Text => ui.add_sized(vec2(width, 20.0), egui::TextEdit::singleline(v).char_limit(500)).changed(),
        FieldType::Number => {
            let bad = !v.trim().is_empty() && v.trim().replace(',', ".").parse::<f64>().is_err();
            let mut te = egui::TextEdit::singleline(v).char_limit(30);
            if bad {
                te = te.text_color(ui.visuals().error_fg_color);
            }
            ui.add_sized(vec2(width.min(90.0), 20.0), te).changed()
        }
        FieldType::Bool => {
            let mut on = v == "true";
            let changed = ui.checkbox(&mut on, "").changed();
            if changed {
                *v = if on { "true".into() } else { String::new() };
            }
            changed
        }
        FieldType::Choice(values) => {
            let mut changed = false;
            let shown = values.iter().find(|(val, _)| val == v).map_or_else(|| v.clone(), |(_, l)| tl!(l).to_string());
            egui::ComboBox::from_id_salt(("diagram_field", salt)).width(width).selected_text(shown).show_ui(ui, |ui| {
                for (val, label) in values {
                    if ui.selectable_label(v == val, tl!(label)).clicked() {
                        *v = val.to_string();
                        changed = true;
                    }
                }
            });
            changed
        }
        FieldType::Ref(_) => {
            let mut changed = false;
            let shown = refs.iter().find(|(id, _)| id == v).map_or_else(|| v.clone(), |(_, l)| l.clone());
            egui::ComboBox::from_id_salt(("diagram_ref", salt)).width(width).selected_text(shown).show_ui(ui, |ui| {
                if ui.selectable_label(v.is_empty(), "—").clicked() {
                    v.clear();
                    changed = true;
                }
                for (id, label) in refs {
                    if ui.selectable_label(v == id, label).clicked() {
                        v.clone_from(id);
                        changed = true;
                    }
                }
            });
            changed
        }
    }
}

/// The picture of `source`, at most `max` (kept while the source stays the same).
fn picture(ui: &mut Ui, source: &str, max: egui::Vec2) {
    match crate::dialogs::diagram_preview(ui.ctx(), source) {
        Ok(tex) => {
            ui.add(egui::Image::new(&tex).max_size(max).bg_fill(Color32::WHITE));
        }
        Err(err) => {
            ui.colored_label(ui.visuals().warn_fg_color, err);
        }
    }
}

/// The gallery: every kind of diagram, by group, with a picture of each.
fn gallery(ui: &mut Ui, e: &mut DiagramEditor) {
    if let Some(kind) = kind_grid(ui, &mut e.category, vec2(196.0, 110.0), 900.0, 480.0, "diagram_gallery", true) {
        e.start(kind);
    }
}

/// The groups as tabs (with "All" when `all`; `""` is all), then a card per kind of the group
/// shown: a picture of its template, `card` big, and its name. The kind clicked.
pub fn kind_grid(ui: &mut Ui, category: &mut &'static str, card: egui::Vec2, width: f32, height: f32, salt: &str, all: bool) -> Option<Kind> {
    ui.horizontal_wrapped(|ui| {
        ui.set_max_width(width);
        if all && ui.selectable_label(category.is_empty(), tl!("All")).clicked() {
            *category = "";
        }
        for cat in Kind::CATEGORIES {
            if ui.selectable_label(*category == cat, tl!(cat)).clicked() {
                *category = cat;
            }
        }
    });
    ui.separator();
    let mut picked = None;
    egui::ScrollArea::vertical().id_salt(salt).max_height(height).auto_shrink([false, true]).show(ui, |ui| {
        ui.set_width(width);
        for cat in Kind::CATEGORIES {
            if !category.is_empty() && *category != cat {
                continue;
            }
            if category.is_empty() {
                ui.strong(tl!(cat));
            }
            // Rows of as many cards as fit: a menu grows to its content, so nothing wraps there.
            let per_row = ((width + 8.0) / (card.x + 20.0)).floor().max(1.0) as usize;
            let kinds: Vec<Kind> = Kind::ALL.into_iter().filter(|k| k.category() == cat).collect();
            for row in kinds.chunks(per_row) {
                ui.horizontal(|ui| {
                    for &kind in row {
                        let r = ui
                            .scope(|ui| {
                                let frame = egui::Frame::group(ui.style()).inner_margin(6.0);
                                frame
                                    .show(ui, |ui| {
                                        ui.set_width(card.x);
                                        ui.vertical_centered(|ui| {
                                            ui.allocate_ui(card, |ui| {
                                                ui.centered_and_justified(|ui| picture(ui, kind.template(), card - vec2(6.0, 4.0)));
                                            });
                                            ui.label(tl!(kind.label()));
                                        });
                                    })
                                    .response
                            })
                            .inner;
                        let r = r.interact(egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                        if r.hovered() {
                            ui.painter().rect_stroke(
                                r.rect,
                                4.0,
                                egui::Stroke::new(1.5, ui.visuals().selection.stroke.color),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if r.clicked() {
                            picked = Some(kind);
                        }
                    }
                });
            }
            ui.add_space(6.0);
        }
    });
    picked
}

/// The editor's body; `true` when the dialog closes.
pub fn show(app: &mut WordApp, ui: &mut Ui, e: &mut DiagramEditor) -> bool {
    // Enter in a text field ends the typing, not the dialog.
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    match e.mode {
        Mode::Gallery => {
            ui.label(tl!("Choose a kind of diagram:"));
            gallery(ui, e);
            ui.add_space(8.0);
            let mut close = false;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
            });
            return close;
        }
        Mode::Text => {
            ui.label(tl!("This diagram uses what the visual editor can't show yet; its text is kept as written."));
            ui.horizontal_top(|ui| {
                ui.add(egui::TextEdit::multiline(&mut e.text).code_editor().desired_width(520.0).desired_rows(20).char_limit(64 * 1024));
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(tl!("Preview"));
                    picture(ui, &e.text, vec2(380.0, 420.0));
                });
            });
            if e.diagram.is_some() && ui.button(tl!("Back to the Visual Editor")).clicked() {
                // What was typed is opened again when an editor reads it.
                if let Opened::Visual(d) | Opened::Partial(d) = codec::open(&e.text) {
                    e.diagram = Some(d);
                }
                e.mode = Mode::Visual;
            }
        }
        Mode::Visual => {
            let Some(d) = e.diagram.as_mut() else {
                e.mode = Mode::Gallery;
                return false;
            };
            let kind = d.kind;
            ui.horizontal(|ui| {
                ui.strong(tl!(kind.label()));
                if ui.button(tl!("Change Kind…")).clicked() {
                    e.mode = Mode::Gallery;
                }
            });
            if e.partial {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    tl!("Parts of this diagram (notes, groups or styles) can't be shown here: applying changes made here drops them. Edit it as text to keep them."),
                );
            }
            ui.separator();
            let source = codec::write(d);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| match (&mut d.model, kind.graph(), kind.sheet()) {
                    (Model::Graph(g), Some(spec), _) => crate::diagram_canvas::show(ui, kind, spec, g, &mut e.canvas),
                    (Model::Sheet(s), _, Some(spec)) => crate::diagram_sheet::show(ui, spec, s),
                    _ => {
                        ui.label(tl!("This kind of diagram has no visual editor yet; its text is kept as written."));
                    }
                });
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    if let (Model::Graph(g), Some(spec)) = (&mut d.model, kind.graph()) {
                        crate::diagram_canvas::properties(ui, kind, spec, g, &mut e.canvas);
                        ui.add_space(8.0);
                    }
                    ui.label(tl!("Preview"));
                    picture(ui, &source, vec2(300.0, 300.0));
                    if kind.graph().is_some() {
                        ui.add_space(4.0);
                        ui.weak(tl!("Plume lays the diagram out; the canvas sets the order and direction."));
                    }
                });
            });
            if mode_text_button(ui) {
                e.text = if e.partial { e.original.clone() } else { source };
                e.mode = Mode::Text;
            }
        }
    }
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

/// The small "Edit as Text" link, for those who want the Mermaid.
fn mode_text_button(ui: &mut Ui) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(tl!("Edit as Text")).small()).frame(false))
        .on_hover_text(tl!("The diagram's Mermaid text"))
        .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_starts_in_its_visual_editor_and_opens_again() {
        for kind in Kind::ALL {
            let e = DiagramEditor::new(kind);
            assert_eq!(e.mode, Mode::Visual, "{kind:?}");
            let src = e.source().unwrap();
            let again = DiagramEditor::open(&src);
            assert_eq!((again.mode, again.partial, again.diagram.map(|d| d.kind)), (Mode::Visual, false, Some(kind)), "{src}");
        }
        let gantt = DiagramEditor::open("flowchart LR\n    subgraph one\n    a --> b\n    end");
        assert_eq!(gantt.mode, Mode::Text);
        assert!(DiagramEditor::gallery().source().is_err());
    }
}
