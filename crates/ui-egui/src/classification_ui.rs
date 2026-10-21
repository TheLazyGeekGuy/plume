//! Plume: the document's classification in the window. Its labels stay in view while editing:
//! in the title bar (one per dimension when there's room, a single compact label otherwise), in
//! the status bar, in Review › Classification and in File › Info. Each label opens its
//! dimension's values; a choice runs `classification.set` (undoable, marks the document edited,
//! written in the front matter when it is saved). The dimensions stay independent. File ›
//! Options › Document Classification manages the referentials and how the labels show.
//!
//! A level's colour always comes with its code and label, never alone.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::WordApp;
use crate::theme::{Tokens, regular, semibold};

/// A dimension the labels show: its command key, its name, and its values (front matter code,
/// interface label).
struct Dim {
    key: &'static str,
    title: &'static str,
    values: Vec<(String, String)>,
    current: Option<String>,
}

/// The interface labels of the dimensions' values.
fn value_label(dim: &str, v: &str) -> &'static str {
    match (dim, v) {
        ("context", "personal") => "Personal",
        ("context", "professional") => "Professional",
        ("context", "associative") => "Association",
        ("context", "other") => "Other",
        ("visibility", "private") => "Private",
        ("visibility", "shared") => "Shared",
        ("visibility", "public") => "Public",
        ("importance", "normal") => "Ordinary",
        ("importance", "important") => "Important",
        ("importance", "strategic") => "Strategic",
        ("personalData", "none") => "No personal data",
        ("personalData", "present") => "Personal data",
        ("personalData", "sensitive") => "Sensitive personal data",
        _ => "",
    }
}

/// The document's labels (`classification.get`).
pub fn state(app: &WordApp) -> Value {
    let settings = &app.session.prefs.classification;
    let labels = wordcraft_engine::classification::read(&app.session.doc);
    let reference = labels.reference.clone().or_else(|| labels.level.as_ref().map(|_| settings.default_reference.clone()));
    let known = reference.as_deref().and_then(|r| wordcraft_engine::classification::referential(settings, r));
    let level = known.as_ref().zip(labels.level.as_deref()).and_then(|(r, c)| r.level(c).cloned());
    json!({
        "reference": reference,
        "known": known.is_some() || reference.is_none(),
        "referenceName": known.as_ref().map(|r| format!("{} {}", r.name, r.version)),
        "level": labels.level,
        "levelLabel": level.as_ref().map(|l| l.label.clone()),
        "color": level.as_ref().map(|l| l.color.clone()),
        "icon": level.as_ref().map(|l| l.icon.clone()),
        "context": labels.context,
        "visibility": labels.visibility,
        "importance": labels.importance,
        "personalData": labels.personal_data,
    })
}

fn dims(st: &Value) -> Vec<Dim> {
    wordcraft_engine::classification::DIMENSIONS
        .iter()
        .map(|(_, key, values)| Dim {
            key,
            title: match *key {
                "context" => "Context",
                "visibility" => "Visibility",
                "importance" => "Importance",
                _ => "Personal data",
            },
            values: values.iter().map(|v| (v.to_string(), value_label(key, v).to_string())).collect(),
            current: st[*key].as_str().map(str::to_string),
        })
        .collect()
}

/// `#RRGGBB` as a colour.
fn color(hex: &str) -> Option<Color32> {
    let h = hex.strip_prefix('#')?;
    let n = u32::from_str_radix(h, 16).ok().filter(|_| h.len() == 6)?;
    Some(Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// A level's interface label: its code and translated label (`C2 · Confidential`), or the code
/// alone when it is its label (`TLP:AMBER`).
fn level_text(st: &Value) -> String {
    match (st["level"].as_str(), st["levelLabel"].as_str()) {
        (Some(code), Some(label)) if code == label => code.to_string(),
        (Some(code), Some(label)) => format!("{code} · {}", tl!(label)),
        (Some(code), None) => code.to_string(),
        _ => tl!("Unclassified").to_string(),
    }
}

/// The text a dimension's label shows.
fn dim_text(d: &Dim) -> String {
    match &d.current {
        Some(v) => match value_label(d.key, v) {
            "" => v.clone(),
            l => tl!(l).to_string(),
        },
        None => tl!(d.title).to_string(),
    }
}

/// Draw the level's pictogram (a dot, a shield or a lock) in its colour, outlined so a white or
/// pale level stays visible.
pub(crate) fn pictogram(ui: &Ui, center: egui::Pos2, icon: &str, fill: Color32, outline: Color32) {
    let p = ui.painter();
    match icon {
        "shield" => {
            let pts = vec![
                pos2(center.x - 4.5, center.y - 4.5),
                pos2(center.x + 4.5, center.y - 4.5),
                pos2(center.x + 4.5, center.y + 0.5),
                pos2(center.x, center.y + 5.0),
                pos2(center.x - 4.5, center.y + 0.5),
            ];
            p.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(1.0, outline)));
        }
        "lock" => {
            p.rect(
                Rect::from_center_size(pos2(center.x, center.y + 1.5), vec2(9.0, 6.5)),
                1.0,
                fill,
                Stroke::new(1.0, outline),
                egui::StrokeKind::Middle,
            );
            p.circle_stroke(pos2(center.x, center.y - 2.0), 2.8, Stroke::new(1.4, outline));
        }
        // Plume: an open padlock (a document saved without encryption).
        "unlock" => {
            p.rect(
                Rect::from_center_size(pos2(center.x - 1.0, center.y + 1.5), vec2(9.0, 6.5)),
                1.0,
                fill,
                Stroke::new(1.0, outline),
                egui::StrokeKind::Middle,
            );
            p.circle_stroke(pos2(center.x + 3.0, center.y - 3.0), 2.8, Stroke::new(1.4, outline));
        }
        _ => {
            p.circle(center, 4.5, fill, Stroke::new(1.0, outline));
        }
    }
}

/// Width of a label with `text`.
fn chip_width(ui: &Ui, text: &str, swatch: bool) -> f32 {
    let w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.to_string(), regular(11.5), Color32::WHITE).size().x);
    w + if swatch { 30.0 } else { 18.0 }
}

/// One label: a rounded box with the level's pictogram (when given) and its text; clicking it
/// opens `menu`.
pub(crate) fn chip(
    ui: &mut Ui,
    app: &mut WordApp,
    text: &str,
    swatch: Option<(Color32, String)>,
    a11y: &str,
    menu: impl FnOnce(&mut Ui, &mut WordApp),
) {
    let t = Tokens::get(ui.ctx());
    let w = chip_width(ui, text, swatch.is_some());
    let (r, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
    let fill = if resp.hovered() || resp.has_focus() { t.hover } else { t.input };
    ui.painter().rect(r, 10.0, fill, Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
    let mut x = r.min.x + 9.0;
    if let Some((c, icon)) = &swatch {
        pictogram(ui, pos2(x + 4.0, r.center().y), icon, *c, t.text);
        x += 13.0;
    }
    ui.painter().text(pos2(x, r.center().y), Align2::LEFT_CENTER, text, regular(11.5), t.text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, a11y));
    let resp = resp.on_hover_text(a11y);
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(220.0);
        menu(ui, app);
    });
}

/// A dimension's values; the current one checked; "Not set" removes it.
fn dim_menu(ui: &mut Ui, app: &mut WordApp, key: &str) {
    let st = state(app);
    let Some(d) = dims(&st).into_iter().find(|d| d.key == key) else { return };
    ui.label(egui::RichText::new(tl!(d.title)).strong());
    for (code, label) in &d.values {
        if ui.selectable_label(d.current.as_deref() == Some(code.as_str()), tl!(label.as_str())).clicked() {
            set(app, json!({ key: code }));
            ui.close();
        }
    }
    // A value Plume doesn't know is shown, never lost.
    if let Some(cur) = d.current.as_deref().filter(|c| !d.values.iter().any(|(v, _)| v == c)) {
        let _ = ui.selectable_label(true, cur);
    }
    ui.separator();
    if ui.button(tl!("Not set")).clicked() {
        set(app, json!({ key: null }));
        ui.close();
    }
}

fn set(app: &mut WordApp, params: Value) {
    if let Err(e) = app.run("classification.set", params) {
        app.status(e);
    }
}

/// The confidentiality levels of the document's referential, its definition, another
/// referential (with its level, chosen at once), and the settings.
fn level_menu(ui: &mut Ui, app: &mut WordApp) {
    let st = state(app);
    let settings = app.session.prefs.classification.clone();
    let reference = st["reference"].as_str().unwrap_or(&settings.default_reference).to_string();
    let known = wordcraft_engine::classification::referential(&settings, &reference);
    ui.label(egui::RichText::new(tl!("Confidentiality")).strong());
    match &known {
        Some(r) => {
            ui.label(
                egui::RichText::new(crate::i18n::fmt(tl!("Referential: {name}"), &[("name", &format!("{} {}", r.name, r.version))])).small().weak(),
            );
            for l in &r.levels {
                let text = if l.code == l.label { l.code.clone() } else { format!("{} · {}", l.code, tl!(l.label.as_str())) };
                let current = st["level"].as_str().is_some_and(|c| c.eq_ignore_ascii_case(&l.code));
                let resp = ui.horizontal(|ui| {
                    let (sw, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
                    pictogram(ui, sw.center(), &l.icon, color(&l.color).unwrap_or(Color32::GRAY), Tokens::get(ui.ctx()).text);
                    ui.selectable_label(current, text)
                });
                if resp.inner.on_hover_text(tl!(l.description.as_str())).clicked() {
                    set(app, json!({"level": l.code}));
                    ui.close();
                }
            }
        }
        // A referential Plume doesn't know: shown as written, kept.
        None => {
            ui.label(crate::i18n::fmt(tl!("Referential {name} (not known here; kept as written)"), &[("name", &reference)]));
            if let Some(level) = st["level"].as_str() {
                let _ = ui.selectable_label(true, level);
            }
        }
    }
    ui.separator();
    if ui.button(tl!("Not set")).clicked() {
        set(app, json!({"level": null}));
        ui.close();
    }
    if let Some(r) = &known {
        ui.menu_button(tl!("Referential definition"), |ui| {
            ui.set_max_width(420.0);
            ui.label(egui::RichText::new(format!("{} {}", r.name, r.version)).strong());
            ui.label(tl!(r.description.as_str()));
            for l in &r.levels {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(if l.code == l.label { l.code.clone() } else { format!("{} · {}", l.code, tl!(l.label.as_str())) }).strong(),
                );
                ui.label(egui::RichText::new(tl!(l.description.as_str())).small());
            }
        });
    }
    // Another referential: its level is chosen with it (never reclassified silently).
    ui.menu_button(tl!("Change referential"), |ui| {
        for r in wordcraft_engine::classification::all(&settings) {
            ui.menu_button(format!("{} {}", r.name, r.version), |ui| {
                for l in &r.levels {
                    let text = if l.code == l.label { l.code.clone() } else { format!("{} · {}", l.code, tl!(l.label.as_str())) };
                    if ui.button(text).clicked() {
                        if let Err(e) = app.run("classification.setReference", json!({"reference": r.id, "level": l.code})) {
                            app.status(e);
                        }
                        ui.close();
                    }
                }
            });
        }
    });
    if ui.button(tl!("Classification Options…")).clicked() {
        open_options(app);
        ui.close();
    }
}

fn open_options(app: &mut WordApp) {
    let _ = app.run("ui.backstage", json!({"value": true, "page": "options", "pane": "classification"}));
}

/// Every dimension in one menu (the compact label, the status bar).
fn all_menu(ui: &mut Ui, app: &mut WordApp) {
    let st = state(app);
    ui.menu_button(format!("{} — {}", tl!("Confidentiality"), level_text(&st)), |ui| level_menu(ui, app));
    for d in dims(&st) {
        ui.menu_button(format!("{} — {}", tl!(d.title), dim_text(&d)), |ui| dim_menu(ui, app, d.key));
    }
    ui.separator();
    if ui.button(tl!("Classification Options…")).clicked() {
        open_options(app);
        ui.close();
    }
}

/// The level's colour and pictogram, for a label.
fn level_swatch(st: &Value) -> Option<(Color32, String)> {
    let c = st["color"].as_str().and_then(color)?;
    Some((c, st["icon"].as_str().unwrap_or("dot").to_string()))
}

/// The labels the title bar shows in full: (dimension key, text).
fn full_labels(st: &Value) -> Vec<(&'static str, String)> {
    let mut v = vec![("level", level_text(st))];
    for d in dims(st) {
        // Personal data shows only when there is some.
        if d.key == "personalData" && !matches!(d.current.as_deref(), Some("present" | "sensitive")) {
            continue;
        }
        v.push((d.key, dim_text(&d)));
    }
    v
}

/// The compact label's text: the level, and a mark when there is personal data.
fn compact_text(st: &Value) -> String {
    let mut s = level_text(st);
    if matches!(st["personalData"].as_str(), Some("present" | "sensitive")) {
        s.push_str(" · ");
        s.push_str(tl!("Personal data"));
    }
    s
}

/// The title bar in the level's colour (Options › Colour the title bar with the level), so the
/// level is in view from across the room: its fill, the ink drawn on it (white or near black,
/// whichever reads), and whether it needs a striped edge because the colour is too close to the
/// plain title bar (black in the dark theme, white in the light one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    pub fill: Color32,
    pub ink: Color32,
    pub ink_dim: Color32,
    pub stripes: bool,
}

/// The document's [`Tint`], or `None` for the plain title bar (no level, unknown colour, or the
/// option or the labels turned off). `plain` is the theme's own title bar colour.
pub fn title_tint(app: &WordApp, plain: Color32) -> Option<Tint> {
    use wordcraft_engine::classification::Presentation;
    let settings = &app.session.prefs.classification;
    if !settings.color_title_bar || settings.presentation == Presentation::Hidden || !app.has_document() {
        return None;
    }
    let fill = state(app)["color"].as_str().and_then(color)?;
    Some(tint(fill, plain))
}

/// [`Tint`] for `fill` over a theme whose plain title bar is `plain`.
pub fn tint(fill: Color32, plain: Color32) -> Tint {
    let light = luminance(fill) > 0.4;
    let ink = if light { Color32::from_rgb(0x14, 0x14, 0x14) } else { Color32::WHITE };
    Tint { fill, ink, ink_dim: ink, stripes: contrast(fill, plain) < 1.5 }
}

/// WCAG relative luminance.
fn luminance(c: Color32) -> f32 {
    let ch = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// WCAG contrast ratio, 1 to 21.
fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The striped edge along the bottom of a tinted title bar whose colour is close to the plain
/// bar: diagonal bands of the level's colour and its ink, like a hazard tape.
pub fn paint_stripes(ui: &Ui, bar: Rect, tint: &Tint) {
    let h = 5.0;
    let band = Rect::from_min_max(pos2(bar.min.x, bar.max.y - h), bar.max);
    let p = ui.painter().with_clip_rect(band);
    p.rect_filled(band, 0.0, tint.fill);
    let step = 14.0;
    let mut x = band.min.x - h;
    while x < band.max.x {
        let pts = vec![pos2(x, band.max.y), pos2(x + h, band.min.y), pos2(x + h + step / 2.0, band.min.y), pos2(x + step / 2.0, band.max.y)];
        p.add(egui::Shape::convex_polygon(pts, tint.ink, Stroke::NONE));
        x += step;
    }
}

/// How wide the title bar's labels are: in full, and compact; `None` when hidden.
pub fn title_widths(app: &WordApp, ui: &Ui) -> Option<(f32, f32)> {
    use wordcraft_engine::classification::Presentation;
    let settings = &app.session.prefs.classification;
    if settings.presentation == Presentation::Hidden || !app.has_document() {
        return None;
    }
    let st = state(app);
    let compact = chip_width(ui, &compact_text(&st), true) + 4.0;
    if settings.presentation == Presentation::Compact {
        return Some((compact, compact));
    }
    let full: f32 = full_labels(&st).iter().map(|(k, text)| chip_width(ui, text, *k == "level") + 4.0).sum();
    Some((full, compact))
}

/// The title bar's labels in `rect`, in full or compact.
pub fn title_labels(app: &mut WordApp, ui: &mut Ui, rect: Rect, compact: bool) {
    let st = state(app);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.spacing_mut().item_spacing = vec2(4.0, 0.0);
    let a11y = |what: &str, text: &str| format!("{}: {text}", tl!(what));
    if compact {
        let text = compact_text(&st);
        chip(&mut child, app, &text, level_swatch(&st), &a11y("Classification", &text), all_menu);
        return;
    }
    for (key, text) in full_labels(&st) {
        if key == "level" {
            chip(&mut child, app, &text, level_swatch(&st), &a11y("Confidentiality", &text), level_menu);
        } else {
            let title = dims(&st).into_iter().find(|d| d.key == key).map_or("", |d| d.title);
            chip(&mut child, app, &text, None, &a11y(title, &text), move |ui, app| dim_menu(ui, app, key));
        }
    }
}

/// The status bar's reminder: the compact label.
pub fn status(app: &mut WordApp, ui: &mut Ui) {
    let st = state(app);
    let text = compact_text(&st);
    chip(ui, app, &text, level_swatch(&st), &format!("{}: {text}", tl!("Classification")), all_menu);
}

/// Review › Classification.
pub fn ribbon_group(app: &mut WordApp, ui: &mut Ui) {
    crate::widgets::group(ui, "Classification", None, app, |ui, app| {
        crate::widgets::menu_button(ui, app, "shield", Some("Confidentiality"), "Confidentiality", true, "classification.set", level_menu);
        crate::ribbon::stack(ui, |ui| {
            crate::widgets::menu_button(ui, app, "briefcase", Some("Context"), "Context", false, "classification.set", |ui, app| {
                dim_menu(ui, app, "context")
            });
            crate::widgets::menu_button(ui, app, "eye", Some("Visibility"), "Visibility", false, "classification.set", |ui, app| {
                dim_menu(ui, app, "visibility")
            });
            crate::widgets::menu_button(ui, app, "star", Some("Importance"), "Importance", false, "classification.set", |ui, app| {
                dim_menu(ui, app, "importance")
            });
        });
        crate::ribbon::stack(ui, |ui| {
            crate::widgets::menu_button(ui, app, "user", Some("Personal data"), "Personal data", false, "classification.set", |ui, app| {
                dim_menu(ui, app, "personalData")
            });
            crate::widgets::small(
                ui,
                app,
                "options",
                Some("Options"),
                "Classification Options",
                "ui.backstage",
                json!({"value": true, "page": "options", "pane": "classification"}),
                false,
            );
        });
    });
}

/// File › Info: every dimension, with its values.
pub fn info_section(app: &mut WordApp, ui: &mut Ui) {
    let st = state(app);
    ui.label(egui::RichText::new(tl!("Classification")).font(semibold(15.0)));
    egui::Grid::new("info_classification").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        ui.label(tl!("Confidentiality"));
        chip(ui, app, &level_text(&st), level_swatch(&st), &format!("{}: {}", tl!("Confidentiality"), level_text(&st)), level_menu);
        ui.end_row();
        ui.label(tl!("Referential"));
        ui.label(st["referenceName"].as_str().or(st["reference"].as_str()).unwrap_or("—"));
        ui.end_row();
        for d in dims(&st) {
            ui.label(tl!(d.title));
            let text = dim_text(&d);
            let key = d.key;
            chip(ui, app, &text, None, &format!("{}: {text}", tl!(d.title)), move |ui, app| dim_menu(ui, app, key));
            ui.end_row();
        }
    });
    ui.label(egui::RichText::new(tl!("A classification declares how the document may circulate; it doesn't encrypt the file.")).small().weak());
}

/// File › Options › Document Classification.
pub fn options_pane(app: &mut WordApp, ui: &mut Ui) {
    use wordcraft_engine::classification::Presentation;
    let settings = app.session.prefs.classification.clone();
    ui.label(egui::RichText::new(tl!("Document Classification")).font(semibold(15.0)));
    ui.add_space(4.0);
    let refs = wordcraft_engine::classification::all(&settings);
    egui::Grid::new("classification_options").num_columns(2).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
        ui.label(tl!("Default referential"));
        let current = refs
            .iter()
            .find(|r| r.id == settings.default_reference)
            .map_or(settings.default_reference.clone(), |r| format!("{} {}", r.name, r.version));
        egui::ComboBox::from_id_salt("class_default_ref").selected_text(current).show_ui(ui, |ui| {
            for r in &refs {
                if ui.selectable_label(r.id == settings.default_reference, format!("{} {}", r.name, r.version)).clicked() {
                    let _ = app.run("classification.options", json!({"defaultReference": r.id}));
                }
            }
        });
        ui.end_row();
        ui.label(tl!("Labels in the title bar"));
        ui.horizontal(|ui| {
            for (p, label, value) in [
                (Presentation::Full, "Each label", "full"),
                (Presentation::Compact, "One compact label", "compact"),
                (Presentation::Hidden, "Hidden", "hidden"),
            ] {
                if ui.radio(settings.presentation == p, tl!(label)).clicked() {
                    let _ = app.run("classification.options", json!({"presentation": value}));
                }
            }
        });
        ui.end_row();
    });
    let mut mark = settings.mark_exports;
    if ui.checkbox(&mut mark, tl!("Show the level on PDF and HTML copies")).changed() {
        let _ = app.run("classification.options", json!({"markExports": mark}));
    }
    let mut bar = settings.color_title_bar;
    if ui.checkbox(&mut bar, tl!("Colour the title bar with the level")).changed() {
        let _ = app.run("classification.options", json!({"colorTitleBar": bar}));
    }
    let mut warn = settings.warn_on_publish;
    if ui.checkbox(&mut warn, tl!("Warn before publishing a document its level asks to keep")).changed() {
        let _ = app.run("classification.options", json!({"warnOnPublish": warn}));
    }
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Referentials")).font(semibold(14.0)));
    for r in &refs {
        let builtin = wordcraft_engine::classification::builtin().iter().any(|b| b.id == r.id);
        let title = format!("{} {}", r.name, r.version);
        let title = if builtin { crate::i18n::fmt(tl!("{name} (built in)"), &[("name", &title)]) } else { title };
        egui::CollapsingHeader::new(title).id_salt(("class_ref", r.id.as_str())).show(ui, |ui| {
            ui.label(egui::RichText::new(tl!(r.description.as_str())).small());
            for l in &r.levels {
                ui.horizontal(|ui| {
                    let (sw, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
                    pictogram(ui, sw.center(), &l.icon, color(&l.color).unwrap_or(Color32::GRAY), Tokens::get(ui.ctx()).text);
                    let text = if l.code == l.label { l.code.clone() } else { format!("{} · {}", l.code, tl!(l.label.as_str())) };
                    ui.label(egui::RichText::new(text).strong());
                    ui.label(egui::RichText::new(tl!(l.description.as_str())).small().weak());
                });
            }
            if builtin {
                if ui.button(tl!("Duplicate to customise")).clicked() {
                    let mut copy = r.clone();
                    copy.id = format!("{}-custom", r.id);
                    copy.name = format!("{} ({})", r.name, tl!("custom"));
                    let _ = app.run("classification.saveReference", json!({"reference": copy}));
                }
            } else {
                editor(app, ui, r);
            }
        });
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(tl!("Colours always come with a level's code and label. Changing a document's referential asks for its new level: a document is never reclassified silently."))
            .small()
            .weak(),
    );
}

/// Edit one of the user's referentials: its levels' labels, descriptions, colours and rules.
fn editor(app: &mut WordApp, ui: &mut Ui, r: &wordcraft_engine::classification::Referential) {
    use wordcraft_engine::classification::{EncryptionAdvice, PublishRule};
    let id = egui::Id::new(("class_edit", r.id.as_str()));
    let mut draft = ui.data(|d| d.get_temp::<String>(id)).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| r.clone());
    ui.horizontal(|ui| {
        ui.label(tl!("Name"));
        ui.text_edit_singleline(&mut draft.name);
        ui.label(tl!("Version"));
        ui.add(egui::TextEdit::singleline(&mut draft.version).desired_width(60.0));
    });
    let mut remove = None;
    for (i, l) in draft.levels.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut l.code).desired_width(70.0).hint_text(tl!("Code")));
            ui.add(egui::TextEdit::singleline(&mut l.label).desired_width(120.0).hint_text(tl!("Label")));
            let mut c = color(&l.color).unwrap_or(Color32::GRAY);
            if ui.color_edit_button_srgba(&mut c).changed() {
                l.color = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
            }
            egui::ComboBox::from_id_salt(("class_icon", i)).width(70.0).selected_text(l.icon.clone()).show_ui(ui, |ui| {
                for icon in ["dot", "shield", "lock"] {
                    ui.selectable_value(&mut l.icon, icon.to_string(), icon);
                }
            });
            egui::ComboBox::from_id_salt(("class_pub", i)).width(100.0).selected_text(format!("{:?}", l.publish)).show_ui(ui, |ui| {
                for p in [PublishRule::Allow, PublishRule::Warn, PublishRule::Confirm] {
                    ui.selectable_value(&mut l.publish, p, format!("{p:?}"));
                }
            });
            egui::ComboBox::from_id_salt(("class_enc", i)).width(120.0).selected_text(format!("{:?}", l.encryption)).show_ui(ui, |ui| {
                for e in [
                    EncryptionAdvice::Optional,
                    EncryptionAdvice::Exchange,
                    EncryptionAdvice::Export,
                    EncryptionAdvice::WarnUnencrypted,
                    EncryptionAdvice::Required,
                ] {
                    ui.selectable_value(&mut l.encryption, e, format!("{e:?}"));
                }
            });
            if ui.small_button("−").on_hover_text(tl!("Remove")).clicked() {
                remove = Some(i);
            }
        });
        ui.add(egui::TextEdit::singleline(&mut l.description).desired_width(f32::INFINITY).hint_text(tl!("Description and rules of diffusion")));
    }
    if let Some(i) = remove {
        draft.levels.remove(i);
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("Add a level")).clicked() {
            draft.levels.push(wordcraft_engine::classification::Level {
                code: format!("L{}", draft.levels.len()),
                color: "#607D8B".into(),
                icon: "dot".into(),
                ..Default::default()
            });
        }
        if ui.button(tl!("Save")).clicked() {
            match app.run("classification.saveReference", json!({"reference": draft})) {
                Ok(_) => ui.data_mut(|d| d.remove::<String>(id)),
                Err(e) => app.status(e),
            }
            return;
        }
        if ui.button(tl!("Delete")).clicked() {
            let _ = app.run("classification.deleteReference", json!({"id": r.id}));
            ui.data_mut(|d| d.remove::<String>(id));
        }
    });
    if let Ok(s) = serde_json::to_string(&draft) {
        ui.data_mut(|d| d.insert_temp(id, s));
    }
}

/// Plume: the explicit decision a classified document's publication asks for: the warnings,
/// then Publish anyway (the command runs again with `acknowledged`) or Cancel.
pub fn confirm_dialog(app: &mut WordApp, ctx: &egui::Context) {
    let Some((id, params, warnings)) = app.classification_confirm.clone() else { return };
    let mut open = true;
    let mut decided: Option<bool> = None;
    egui::Window::new(tl!("Classified document"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(tl!("This document's classification asks for a decision before it is published:"));
            for w in &warnings {
                ui.label(format!("• {w}"));
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tl!("Publishing makes a readable copy there. The label doesn't protect it; encryption does.")).small().weak(),
            );
            ui.horizontal(|ui| {
                if ui.button(tl!("Publish anyway")).clicked() {
                    decided = Some(true);
                }
                if ui.button(tl!("Cancel")).clicked() {
                    decided = Some(false);
                }
            });
        });
    if !open {
        decided = Some(false);
    }
    if let Some(go) = decided {
        app.classification_confirm = None;
        if go {
            let mut p = params;
            p["acknowledged"] = json!(true);
            let _ = app.run(&id, p);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{Services, WordApp};

    fn app() -> WordApp {
        let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Notes"));
        s.native_markdown = true;
        WordApp::new(s, Services::default())
    }

    /// The labels change without opening anything: each dimension on its own, shown at once, the
    /// document marked edited.
    #[test]
    fn labels_change_from_the_window_and_show_at_once() {
        let mut a = app();
        assert_eq!(super::level_text(&super::state(&a)), "Unclassified");
        a.run("classification.set", json!({"level": "C2"})).unwrap();
        a.run("classification.set", json!({"visibility": "shared"})).unwrap();
        let st = super::state(&a);
        assert_eq!(super::level_text(&st), "C2 · Confidential");
        assert_eq!(st["visibility"], "shared");
        assert!(a.session.dirty);
        let labels: Vec<String> = super::full_labels(&st).into_iter().map(|(_, t)| t).collect();
        assert_eq!(labels, ["C2 · Confidential", "Context", "Shared", "Importance"], "personal data shows only when there is some");
    }

    /// Publishing a confidential document to a forge waits for an explicit decision.
    #[test]
    fn publishing_a_confidential_document_asks_first() {
        let mut a = app();
        a.run("classification.set", json!({"level": "C3"})).unwrap();
        let r = a.run("forge.saveAs", json!({"forge": "github", "repo": "acme/docs", "path": "x.md"})).unwrap();
        assert_eq!(r, json!({"pending": "classification"}));
        let (id, _, warnings) = a.classification_confirm.clone().unwrap();
        assert_eq!(id, "forge.saveAs");
        assert!(warnings.iter().any(|w| w.contains("C3")), "{warnings:?}");
        // A public document goes without asking (it then needs signing in, here).
        let mut b = app();
        b.run("classification.set", json!({"level": "C0"})).unwrap();
        assert!(b.run("forge.saveAs", json!({"forge": "github", "repo": "acme/docs", "path": "x.md"})).is_err());
        assert!(b.classification_confirm.is_none());
    }
}
