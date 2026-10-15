//! File › Export › PDF with a template (Plume): pick a template, export (a TeX engine compiles
//! it on a worker thread), and edit templates — their settings (YAML) and their LaTeX — with a
//! PDF preview of the open document, or start a new template from one.

use egui::{RichText, Ui, vec2};
use serde_json::{Value, json};

use crate::WordApp;

/// Where a worker leaves an export's result.
type Slot = std::sync::Arc<std::sync::Mutex<Option<Result<Value, String>>>>;

#[derive(Default)]
pub struct PdfTemplates {
    /// The template exports use (its id); empty = the default.
    pub selected: String,
    /// (id, name) of the templates found, read when the Export page opens.
    list: Vec<(String, String)>,
    listed: bool,
    /// An export running: its slot, and whether it's a preview (opened when done).
    job: Option<(Slot, bool)>,
    pub editor: Option<Editor>,
    /// The last export's outcome, shown under the buttons.
    message: String,
    /// A path picked for an export, started on the next frame.
    pub pending: Option<String>,
}

pub struct Editor {
    id: String,
    name: String,
    metadata: String,
    template: String,
    /// 0: settings, 1: LaTeX.
    tab: u8,
    new_name: String,
    message: String,
    dirty: bool,
}

impl PdfTemplates {
    /// The templates' names (the user's own, shown untranslated).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.list.iter().map(|(_, n)| n.as_str())
    }

    fn id(&self) -> &str {
        if self.selected.is_empty() { wordcraft_engine::pdf_template::DEFAULT_TEMPLATE } else { &self.selected }
    }
}

fn refresh(app: &mut WordApp) {
    let list = app.run("pdfTemplate.list", json!({})).ok();
    let items = list.as_ref().and_then(|v| v["templates"].as_array().cloned()).unwrap_or_default();
    app.pdf_templates.list =
        items.iter().filter_map(|t| Some((t["id"].as_str()?.to_string(), t["name"].as_str().unwrap_or("").to_string()))).collect();
    app.pdf_templates.listed = true;
}

/// Start an export to `path` on a worker thread.
pub fn start_export(app: &mut WordApp, ctx: &egui::Context, path: &str, preview: bool) {
    let params = json!({"path": path, "template": app.pdf_templates.id()});
    let job = match wordcraft_engine::pdf_template::PdfJob::prepare(&app.session, &params) {
        Ok(j) => j,
        Err(e) => {
            app.pdf_templates.message = e.to_string();
            return;
        }
    };
    let slot = Slot::default();
    let out = slot.clone();
    let work = move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.run().map_err(|e| e.to_string())))
            .unwrap_or_else(|_| Err("the PDF couldn't be made".into()));
        if let Ok(mut s) = out.lock() {
            *s = Some(r);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ctx = ctx.clone();
        if let Err(e) = std::thread::Builder::new().name("plume-pdf".into()).spawn(move || {
            work();
            ctx.request_repaint();
        }) {
            app.pdf_templates.message = e.to_string();
            return;
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        work();
    }
    app.pdf_templates.message = tl!("Making the PDF… (the first time, the TeX engine may download what it needs)").to_string();
    app.pdf_templates.job = Some((slot, preview));
}

/// Each frame: start a picked export, pick up a finished one.
pub fn tick(app: &mut WordApp, ctx: &egui::Context) {
    if let Some(path) = app.pdf_templates.pending.take() {
        start_export(app, ctx, &path, false);
    }
    poll(app, ctx);
}

/// Pick up a finished export.
fn poll(app: &mut WordApp, ctx: &egui::Context) {
    let Some((slot, preview)) = &app.pdf_templates.job else { return };
    let done = slot.lock().ok().and_then(|mut s| s.take());
    let preview = *preview;
    match done {
        None => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
        Some(r) => {
            app.pdf_templates.job = None;
            match r {
                Ok(v) => {
                    let path = v["path"].as_str().unwrap_or("").to_string();
                    app.pdf_templates.message = crate::i18n::fmt(tl!("Exported {path}"), &[("path", &path)]);
                    app.status(app.pdf_templates.message.clone());
                    if preview {
                        ctx.open_url(egui::OpenUrl::new_tab(format!("file://{path}")));
                    }
                }
                Err(e) => {
                    app.status(e.clone());
                    app.pdf_templates.message = e;
                }
            }
        }
    }
}

fn preview_path(app: &WordApp) -> String {
    std::env::temp_dir().join(format!("plume-preview-{}-{}.pdf", std::process::id(), app.pdf_templates.id())).to_string_lossy().into_owned()
}

/// The Export page's "PDF with a template" section (or the template editor, when open).
pub fn section(app: &mut WordApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if !app.pdf_templates.listed {
        refresh(app);
    }
    if app.pdf_templates.editor.is_some() {
        editor(app, ui);
        return;
    }
    ui.label(RichText::new(tl!("PDF with a template")).strong().size(15.0));
    ui.label(tl!("Lay the document out with a document template: a title page, headers and footers, covers and letterheads. The document's front matter sets the template's options."));
    ui.add_space(6.0);
    let current = app.pdf_templates.id().to_string();
    let shown = app.pdf_templates.list.iter().find(|(id, _)| *id == current).map(|(_, n)| n.clone()).unwrap_or_else(|| current.clone());
    let mut pick = None;
    ui.horizontal(|ui| {
        ui.label(tl!("Template:"));
        egui::ComboBox::from_id_salt("pdf_template").selected_text(shown).width(220.0).show_ui(ui, |ui| {
            for (id, name) in &app.pdf_templates.list {
                if ui.selectable_label(*id == current, name).clicked() {
                    pick = Some(id.clone());
                }
            }
        });
    });
    if let Some(id) = pick {
        app.pdf_templates.selected = id;
    }
    let busy = app.pdf_templates.job.is_some();
    ui.horizontal(|ui| {
        if ui.add_enabled(!busy, egui::Button::new(tl!("Export PDF…")).min_size(vec2(150.0, 30.0))).clicked() {
            let name = format!("{}.pdf", app.title_stem());
            let _ = app.ask_file(crate::file_dialogs::FileDialogRequest::Save { name }, crate::file_dialogs::AfterPick::ExportTemplate);
        }
        if ui.add_enabled(!busy, egui::Button::new(tl!("Preview")).min_size(vec2(100.0, 30.0))).clicked() {
            let path = preview_path(app);
            start_export(app, &ctx, &path, true);
        }
        if ui.add_enabled(!busy, egui::Button::new(tl!("Edit template…")).min_size(vec2(130.0, 30.0))).clicked() {
            open_editor(app, &current);
        }
    });
    if busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(&app.pdf_templates.message);
        });
    } else if !app.pdf_templates.message.is_empty() {
        ui.label(RichText::new(&app.pdf_templates.message).weak());
    }
    if let Ok(engine) = app.run("pdfTemplate.engine", json!({}))
        && engine["found"] != json!(true)
    {
        ui.label(
            RichText::new(tl!("No TeX engine was found. Install Tectonic (brew install tectonic) or MacTeX, then export again."))
                .color(egui::Color32::from_rgb(200, 120, 0)),
        );
    }
}

fn open_editor(app: &mut WordApp, id: &str) {
    match app.run("pdfTemplate.read", json!({"id": id})) {
        Ok(v) => {
            let name = v["name"].as_str().unwrap_or(id).to_string();
            app.pdf_templates.editor = Some(Editor {
                id: id.to_string(),
                new_name: format!("{name} ({})", tl!("copy")),
                name,
                metadata: v["metadata"].as_str().unwrap_or("").to_string(),
                template: v["template"].as_str().unwrap_or("").to_string(),
                tab: 0,
                message: String::new(),
                dirty: false,
            })
        }
        Err(e) => app.pdf_templates.message = e.to_string(),
    }
}

fn save_editor(app: &mut WordApp) -> bool {
    let Some(ed) = &app.pdf_templates.editor else { return false };
    let params = json!({"id": ed.id, "metadata": ed.metadata, "template": ed.template, "name": ed.name});
    let r = app.run("pdfTemplate.save", params);
    let Some(ed) = app.pdf_templates.editor.as_mut() else { return false };
    match r {
        Ok(_) => {
            ed.dirty = false;
            ed.message = tl!("Template saved.").to_string();
            true
        }
        Err(e) => {
            ed.message = e.to_string();
            false
        }
    }
}

fn editor(app: &mut WordApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let busy = app.pdf_templates.job.is_some();
    let mut close = false;
    let mut save = false;
    let mut preview = false;
    let mut duplicate = None;
    let mut reset = false;
    if let Some(ed) = app.pdf_templates.editor.as_mut() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(crate::i18n::fmt(tl!("Template: {name}"), &[("name", &ed.name)])).strong().size(15.0));
            if ed.dirty {
                ui.label(RichText::new(tl!("(not saved)")).weak());
            }
        });
        ui.horizontal(|ui| {
            ui.label(tl!("Name:"));
            if ui.add(egui::TextEdit::singleline(&mut ed.name).desired_width(240.0)).changed() {
                ed.dirty = true;
            }
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut ed.tab, 0, tl!("Settings (YAML)"));
            ui.selectable_value(&mut ed.tab, 1, tl!("Layout (LaTeX)"));
        });
        let hint = if ed.tab == 0 {
            tl!(
                "Options such as titlepage, titlepage-color, header-left, footer-right, page-background, geometry, backcover. A document's front matter overrides them."
            )
        } else {
            tl!("The LaTeX template, in pandoc's template language ($title$, $if(toc)$…$endif$, $for(author)$…$endfor$).")
        };
        ui.label(RichText::new(hint).weak());
        let text = if ed.tab == 0 { &mut ed.metadata } else { &mut ed.template };
        let height = (ui.available_height() - 120.0).max(240.0);
        egui::ScrollArea::vertical().id_salt(("pdf_tpl_edit", ed.tab)).max_height(height).show(ui, |ui| {
            if ui.add(egui::TextEdit::multiline(text).code_editor().desired_width(f32::INFINITY).desired_rows(24)).changed() {
                ed.dirty = true;
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            save = ui.add_enabled(ed.dirty, egui::Button::new(tl!("Save"))).clicked();
            preview = ui.add_enabled(!busy, egui::Button::new(tl!("Save and preview"))).clicked();
            if ed.id == wordcraft_engine::pdf_template::DEFAULT_TEMPLATE && ui.button(tl!("Restore the original")).clicked() {
                reset = true;
            }
            close = ui.button(tl!("Close")).clicked();
        });
        ui.horizontal(|ui| {
            ui.label(tl!("New template from this one:"));
            ui.add(egui::TextEdit::singleline(&mut ed.new_name).desired_width(200.0));
            if ui.add_enabled(!ed.new_name.trim().is_empty(), egui::Button::new(tl!("Create"))).clicked() {
                duplicate = Some(ed.new_name.trim().to_string());
            }
        });
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(&app.pdf_templates.message);
            });
        } else if !ed.message.is_empty() {
            ui.label(RichText::new(&ed.message).weak());
        }
        if !busy && !app.pdf_templates.message.is_empty() && ed.message.is_empty() {
            ui.label(RichText::new(&app.pdf_templates.message).weak());
        }
    }
    if save {
        save_editor(app);
    }
    if preview && save_editor(app) {
        if let Some(ed) = &app.pdf_templates.editor {
            app.pdf_templates.selected = ed.id.clone();
        }
        let path = preview_path(app);
        if let Some(ed) = app.pdf_templates.editor.as_mut() {
            ed.message.clear();
        }
        start_export(app, &ctx, &path, true);
    }
    if reset {
        let r = app.run("pdfTemplate.reset", json!({"id": wordcraft_engine::pdf_template::DEFAULT_TEMPLATE}));
        match r {
            Ok(_) => open_editor(app, wordcraft_engine::pdf_template::DEFAULT_TEMPLATE),
            Err(e) => {
                if let Some(ed) = app.pdf_templates.editor.as_mut() {
                    ed.message = e.to_string();
                }
            }
        }
    }
    if let Some(name) = duplicate {
        // The copy starts from what's on screen: save it first.
        let id = app.pdf_templates.editor.as_ref().map(|e| e.id.clone()).unwrap_or_default();
        if app.pdf_templates.editor.as_ref().is_some_and(|e| e.dirty) && !save_editor(app) {
            return;
        }
        match app.run("pdfTemplate.duplicate", json!({"id": id, "name": name})) {
            Ok(v) => {
                let new_id = v["id"].as_str().unwrap_or("").to_string();
                refresh(app);
                app.pdf_templates.selected = new_id.clone();
                open_editor(app, &new_id);
                if let Some(ed) = app.pdf_templates.editor.as_mut() {
                    ed.message = tl!("New template created: it is the one exports use now.").to_string();
                }
            }
            Err(e) => {
                if let Some(ed) = app.pdf_templates.editor.as_mut() {
                    ed.message = e.to_string();
                }
            }
        }
    }
    if close {
        app.pdf_templates.editor = None;
        refresh(app);
    }
}
