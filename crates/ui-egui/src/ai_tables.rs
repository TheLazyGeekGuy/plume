//! Plume: help from a language model (File › Options › Artificial intelligence; Review › AI ›
//! Restructure Tables). The model runs where the user says — Ollama or LM Studio on this
//! computer by default; requests go out on a worker thread and the window shows each table's
//! outcome. Nothing is sent until the user turns it on and asks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::{RichText, Ui};
use serde_json::{Value, json};
use wordcraft_engine::ai;

use crate::WordApp;

/// A table's request, prepared on the UI thread.
struct Ask {
    index: usize,
    system: String,
    user: String,
    schema: Value,
}

/// A table's answer, from the worker.
struct Answer {
    index: usize,
    seconds: f64,
    reply: Result<Value, String>,
}

#[derive(Default)]
struct Progress {
    answers: Vec<Answer>,
    finished: bool,
}

struct Job {
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
    total: usize,
}

/// One table's outcome, shown in the window.
struct Outcome {
    index: usize,
    applied: bool,
    text: String,
}

#[derive(Default)]
pub struct AiTables {
    pub open: bool,
    job: Option<Job>,
    outcomes: Vec<Outcome>,
    message: String,
    /// Models the server offers (Options), and a listing in progress.
    models: Vec<String>,
    listing: Option<Arc<Mutex<Option<Result<Vec<String>, String>>>>>,
    models_message: String,
}

/// Start `work` on a worker thread (desktop); in the browser there's no model server to reach.
fn spawn(ctx: &egui::Context, work: impl FnOnce() + Send + 'static) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("plume-ai".into())
            .spawn(move || {
                work();
                ctx.request_repaint();
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (ctx, work);
        Err(tl!("A language model can't be reached from the browser.").to_string())
    }
}

/// File › Options: the model to use.
pub fn settings(app: &mut WordApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    poll_models(app);
    let mut s = app.session.prefs.ai.clone();
    ui.checkbox(&mut s.enabled, tl!("Use a language model"));
    ui.label(
        RichText::new(tl!(
            "Plume asks it to restructure the tables Markdown can't hold (forms and pages laid out in tables), and keeps a rewrite only when every word is still there. Ollama and LM Studio run on this computer: nothing leaves it."
        ))
        .small()
        .weak(),
    );
    ui.add_enabled_ui(s.enabled, |ui| {
        let server = match s.url.as_str() {
            ai::OLLAMA => 0,
            ai::LM_STUDIO => 1,
            _ => 2,
        };
        let names = ["Ollama", "LM Studio", tl!("Other server")];
        ui.horizontal(|ui| {
            ui.label(tl!("Server:"));
            egui::ComboBox::from_id_salt("ai_server").selected_text(names[server]).width(160.0).show_ui(ui, |ui| {
                for (i, name) in names.iter().enumerate() {
                    if ui.selectable_label(i == server, *name).clicked() && i != server {
                        s.url = match i {
                            0 => ai::OLLAMA.into(),
                            1 => ai::LM_STUDIO.into(),
                            _ => String::new(),
                        };
                    }
                }
            });
            if server == 2 {
                ui.add(egui::TextEdit::singleline(&mut s.url).hint_text("https://…/v1").desired_width(260.0));
            }
        });
        ui.horizontal(|ui| {
            ui.label(tl!("Model:"));
            let models = &app.ai_tables.models;
            egui::ComboBox::from_id_salt("ai_model")
                .selected_text(if s.model.is_empty() { tl!("Choose a model").to_string() } else { s.model.clone() })
                .width(220.0)
                .show_ui(ui, |ui| {
                    for m in models {
                        if ui.selectable_label(*m == s.model, m).clicked() {
                            s.model = m.clone();
                        }
                    }
                });
            let busy = app.ai_tables.listing.is_some();
            if ui.add_enabled(!busy, egui::Button::new(tl!("Refresh"))).clicked() {
                list_models(app, &ctx, &s);
            }
            if busy {
                ui.spinner();
            }
        });
        ui.horizontal(|ui| {
            ui.label(tl!("Reasoning:"));
            let levels = [("", tl!("Model's default")), ("low", tl!("Low")), ("medium", tl!("Medium")), ("high", tl!("High"))];
            let shown = levels.iter().find(|(k, _)| *k == s.reasoning).map_or(levels[0].1, |(_, n)| *n);
            egui::ComboBox::from_id_salt("ai_reasoning").selected_text(shown).width(160.0).show_ui(ui, |ui| {
                for (k, n) in levels {
                    if ui.selectable_label(k == s.reasoning, n).clicked() {
                        s.reasoning = k.to_string();
                    }
                }
            });
        });
        if !app.ai_tables.models_message.is_empty() {
            ui.label(RichText::new(&app.ai_tables.models_message).small().weak());
        }
        ui.label(
            RichText::new(tl!("Suggested in Ollama: gpt-oss:20b with 16 GB of memory or more, qwen3:4b with 8 GB (ollama pull gpt-oss:20b)."))
                .small()
                .weak(),
        );
    });
    if s != app.session.prefs.ai {
        app.session.prefs.ai = s;
    }
}

fn list_models(app: &mut WordApp, ctx: &egui::Context, s: &ai::Settings) {
    let Some(t) = app.session.ai_transport.clone() else {
        app.ai_tables.models_message = tl!("A language model can't be reached from here.").to_string();
        return;
    };
    let slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    let p = s.provider();
    match spawn(ctx, move || {
        let r = ai::list_models(t.as_ref(), &p);
        if let Ok(mut o) = out.lock() {
            *o = Some(r);
        }
    }) {
        Ok(()) => app.ai_tables.listing = Some(slot),
        Err(e) => app.ai_tables.models_message = e,
    }
}

fn poll_models(app: &mut WordApp) {
    let Some(slot) = &app.ai_tables.listing else { return };
    let Some(r) = slot.lock().ok().and_then(|mut s| s.take()) else { return };
    app.ai_tables.listing = None;
    match r {
        Ok(models) => {
            app.ai_tables.models_message = if models.is_empty() {
                tl!("The server has no models yet: download one first (ollama pull gpt-oss:20b).").to_string()
            } else {
                crate::i18n::fmt(tl!("{n} models on the server."), &[("n", &models.len().to_string())])
            };
            if app.session.prefs.ai.model.is_empty()
                && let Some(m) = models.iter().find(|m| m.starts_with("gpt-oss") || m.starts_with("qwen3")).or(models.first())
            {
                app.session.prefs.ai.model = m.clone();
            }
            app.ai_tables.models = models;
        }
        Err(e) => app.ai_tables.models_message = e,
    }
}

/// Send every table that can be restructured to the model, one after the other.
fn start(app: &mut WordApp, ctx: &egui::Context) {
    app.ai_tables.outcomes.clear();
    let Some(t) = app.session.ai_transport.clone() else {
        app.ai_tables.message = tl!("A language model can't be reached from here.").to_string();
        return;
    };
    let found = app.run("ai.tables", json!({})).unwrap_or_default();
    let mut asks = Vec::new();
    for c in found["tables"].as_array().into_iter().flatten().filter(|c| c["blocked"].is_null()) {
        let index = c["index"].as_u64().unwrap_or(0) as usize;
        if let Ok(r) = app.run("ai.tableRequest", json!({"index": index})) {
            asks.push(Ask {
                index,
                system: r["system"].as_str().unwrap_or("").to_string(),
                user: r["user"].as_str().unwrap_or("").to_string(),
                schema: r["schema"].clone(),
            });
        }
    }
    if asks.is_empty() {
        app.ai_tables.message = tl!("No table to restructure.").to_string();
        return;
    }
    let progress = Arc::new(Mutex::new(Progress::default()));
    let cancel = Arc::new(AtomicBool::new(false));
    let (out, stop, p, total) = (progress.clone(), cancel.clone(), app.session.prefs.ai.provider(), asks.len());
    let started = spawn(ctx, move || {
        for a in asks {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let clock = std::time::Instant::now();
            let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ai::chat_json(t.as_ref(), &p, &a.system, &a.user, &a.schema)))
                .unwrap_or_else(|_| Err("the request failed".into()));
            if let Ok(mut g) = out.lock() {
                g.answers.push(Answer { index: a.index, seconds: clock.elapsed().as_secs_f64(), reply });
            }
        }
        if let Ok(mut g) = out.lock() {
            g.finished = true;
        }
    });
    match started {
        Ok(()) => {
            app.ai_tables.message.clear();
            app.ai_tables.job = Some(Job { progress, cancel, total });
        }
        Err(e) => app.ai_tables.message = e,
    }
}

/// Apply the answers once the worker is done (the check refuses an answer whose table changed
/// meanwhile: its words no longer match).
fn finish(app: &mut WordApp) {
    let Some(job) = &app.ai_tables.job else { return };
    let done = job.progress.lock().map(|g| g.finished).unwrap_or(true);
    if !done {
        return;
    }
    let Some(job) = app.ai_tables.job.take() else { return };
    let answers = job.progress.lock().map(|mut g| std::mem::take(&mut g.answers)).unwrap_or_default();
    let mut outcomes = Vec::new();
    let mut proposals = Vec::new();
    for a in &answers {
        match &a.reply {
            Ok(v) => proposals.push(json!({"index": a.index, "answer": v})),
            Err(e) => outcomes.push(Outcome { index: a.index, applied: false, text: e.clone() }),
        }
    }
    let mut applied = 0;
    if !proposals.is_empty() {
        match app.run("ai.restructureTables", json!({"proposals": proposals})) {
            Ok(r) => {
                for res in r["results"].as_array().into_iter().flatten() {
                    let ok = res["applied"].as_bool().unwrap_or(false);
                    applied += usize::from(ok);
                    let secs = answers.iter().find(|a| Some(a.index as u64) == res["index"].as_u64()).map_or(0.0, |a| a.seconds);
                    let text = res["check"].as_str().or(res["reason"].as_str()).unwrap_or("").to_string();
                    outcomes.push(Outcome { index: res["index"].as_u64().unwrap_or(0) as usize, applied: ok, text: format!("{text} ({secs:.0} s)") });
                }
            }
            Err(e) => app.ai_tables.message = e,
        }
    }
    outcomes.sort_by_key(|o| o.index);
    let n = answers.len();
    app.ai_tables.message = if applied > 0 {
        crate::i18n::fmt(
            tl!("{applied} of {n} tables restructured. Undo (Ctrl+Z) puts them all back."),
            &[("applied", &applied.to_string()), ("n", &n.to_string())],
        )
    } else {
        tl!("No table was changed.").to_string()
    };
    app.ai_tables.outcomes = outcomes;
}

/// Review › AI › Restructure Tables: the window.
pub fn show(app: &mut WordApp, ctx: &egui::Context) {
    finish(app);
    if !app.ai_tables.open {
        return;
    }
    let mut open = true;
    let mut go = false;
    let mut cancel = false;
    let found = app.run("ai.tables", json!({})).unwrap_or_default();
    let tables: Vec<Value> = found["tables"].as_array().cloned().unwrap_or_default();
    let ready = tables.iter().filter(|c| c["blocked"].is_null()).count();
    let settings = app.session.prefs.ai.clone();
    egui::Window::new(tl!("Restructure Tables"))
        .id(egui::Id::new("ai-tables"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(460.0)
        .show(ctx, |ui| {
            if !settings.enabled || settings.model.is_empty() {
                ui.label(tl!("Choose a language model first: File › Options › Save › Artificial intelligence."));
                return;
            }
            ui.label(
                RichText::new(crate::i18n::fmt(tl!("Model: {model} at {url}"), &[("model", &settings.model), ("url", &settings.url)])).small().weak(),
            );
            ui.add_space(4.0);
            if tables.is_empty() {
                ui.label(tl!("No table in this document needs a model: Markdown holds its tables as they are."));
            } else {
                ui.label(crate::i18n::fmt(
                    tl!("{ready} of {n} tables can be sent to the model."),
                    &[("ready", &ready.to_string()), ("n", &tables.len().to_string())],
                ));
                egui::ScrollArea::vertical().max_height(160.0).id_salt("ai-tables-list").show(ui, |ui| {
                    for c in &tables {
                        let what = crate::i18n::fmt(
                            tl!("Block {index}: {rows} × {cols}, {words} words"),
                            &[
                                ("index", &c["index"].to_string()),
                                ("rows", &c["rows"].to_string()),
                                ("cols", &c["cols"].to_string()),
                                ("words", &c["words"].to_string()),
                            ],
                        );
                        match c["blocked"].as_str() {
                            Some(why) => ui.label(RichText::new(format!("{what} — {why}")).weak()),
                            None => ui.label(what),
                        };
                    }
                });
            }
            ui.add_space(6.0);
            match &app.ai_tables.job {
                Some(job) => {
                    let done = job.progress.lock().map(|g| g.answers.len()).unwrap_or(0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(crate::i18n::fmt(
                            tl!("Table {i} of {n}…"),
                            &[("i", &(done + 1).min(job.total).to_string()), ("n", &job.total.to_string())],
                        ));
                        cancel = ui.button(tl!("Stop")).clicked();
                    });
                    ctx.request_repaint_after(std::time::Duration::from_millis(300));
                }
                None => {
                    go = ui.add_enabled(ready > 0, egui::Button::new(tl!("Restructure"))).clicked();
                }
            }
            if !app.ai_tables.message.is_empty() {
                ui.label(&app.ai_tables.message);
            }
            for o in &app.ai_tables.outcomes {
                let mark = if o.applied { tl!("Changed") } else { tl!("Unchanged") };
                ui.label(RichText::new(format!("{} {} — {mark}: {}", tl!("Block"), o.index, o.text)).small());
            }
        });
    if cancel && let Some(job) = &app.ai_tables.job {
        job.cancel.store(true, Ordering::Relaxed);
    }
    if go {
        start(app, ctx);
    }
    if !open {
        app.ai_tables.open = false;
        if let Some(job) = &app.ai_tables.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::sync::Arc;

    use serde_json::{Value, json};
    use wordcraft_engine::Session;
    use wordcraft_engine::doc::{Block, Document, Table, table::Cell};
    use wordcraft_engine::vcs::forge::{Request, Response, Transport};

    use crate::{Services, WordApp};

    /// A model server that copies every paragraph of the table it's sent, in order.
    struct Faithful;

    fn paragraphs(t: &Value, out: &mut Vec<Value>) {
        for cell in t["rows"].as_array().into_iter().flatten().flat_map(|r| r.as_array().into_iter().flatten()) {
            for c in cell["content"].as_array().into_iter().flatten() {
                match c.as_str() {
                    Some(s) => out.push(json!({"type": "paragraph", "text": s})),
                    None => paragraphs(&c["table"], out),
                }
            }
        }
    }

    impl Transport for Faithful {
        fn send(&self, req: &Request) -> Result<Response, String> {
            let body: Value = serde_json::from_str(req.body.as_deref().unwrap_or("{}")).map_err(|e| e.to_string())?;
            let user = body["messages"][1]["content"].as_str().unwrap_or("");
            let table: Value = serde_json::from_str(user.split_once('\n').map_or("{}", |(_, t)| t)).map_err(|e| e.to_string())?;
            let mut blocks = Vec::new();
            paragraphs(&table, &mut blocks);
            let answer = json!({"kind": "layout", "blocks": blocks}).to_string();
            Ok(Response { status: 200, body: json!({"choices": [{"message": {"content": answer}}]}).to_string().into_bytes() })
        }
    }

    fn app_with_form() -> WordApp {
        let mut outer = Table::new(2, 2, 400.0);
        outer.rows[0].cells[0] = Cell::with_text("Name:");
        outer.rows[0].cells[1] = Cell::with_text("Jane Doe");
        let mut inner = Table::new(1, 2, 200.0);
        inner.rows[0].cells[0] = Cell::with_text("Lyon");
        inner.rows[0].cells[1] = Cell::with_text("France");
        outer.rows[1].cells[0] = Cell::with_text("City:");
        outer.rows[1].cells[1].blocks.push(Arc::new(Block::Table(inner)));
        let mut d = Document::from_text("Before the form");
        d.body.push(Arc::new(Block::Table(outer)));
        let mut app = WordApp::new(Session::new(d), Services::default());
        app.session.ai_transport = Some(Arc::new(Faithful));
        app.session.prefs.ai = wordcraft_engine::ai::Settings { enabled: true, model: "faithful".into(), ..Default::default() };
        app
    }

    #[test]
    fn the_window_restructures_on_a_worker_and_one_undo_restores() {
        let mut app = app_with_form();
        let ctx = egui::Context::default();
        assert!(!wordcraft_engine::ai::Settings::default().enabled, "off until the user turns it on");
        super::start(&mut app, &ctx);
        assert!(app.ai_tables.job.is_some());
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.ai_tables.job.is_some() {
            super::finish(&mut app);
            assert!(std::time::Instant::now() < end, "the worker never finished");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.ai_tables.outcomes.iter().all(|o| o.applied), "{}", app.ai_tables.message);
        assert!(app.session.doc.body.iter().all(|b| b.as_table().is_none()));
        assert!(app.session.doc.plain_text(wordcraft_engine::doc::StoryRef::Body).contains("France"));
        app.run("edit.undo", json!({})).unwrap();
        assert!(app.session.doc.body.iter().any(|b| b.as_table().is_some()), "undo brings the table back");
    }

    #[test]
    fn without_a_server_it_says_so() {
        let mut app = app_with_form();
        app.session.ai_transport = None;
        super::start(&mut app, &egui::Context::default());
        assert!(app.ai_tables.job.is_none());
        assert!(!app.ai_tables.message.is_empty());
    }
}
