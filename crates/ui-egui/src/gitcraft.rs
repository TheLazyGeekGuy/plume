//! GitCraft's document-first workspace: a narrow Git pane, the unmodified
//! central editor, and a contextual review pane. No fake branch data.
use egui::{Stroke, Ui};
use crate::{WordApp, panes};
use crate::theme::Tokens;

#[cfg(not(target_arch = "wasm32"))]
use wordcraft_vcs::git::{Repo, WorkspaceSnapshot};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Default)]
struct GitCache {
    path: Option<std::path::PathBuf>,
    snapshot: Option<WorkspaceSnapshot>,
    error: Option<String>,
}

fn visible(ui: &Ui, name: &'static str) -> bool {
    ui.data(|d| d.get_temp::<bool>(egui::Id::new(("gitcraft_visible", name))).unwrap_or(true))
}

fn set_visible(ui: &mut Ui, name: &'static str, value: bool) {
    ui.data_mut(|d| d.insert_temp(egui::Id::new(("gitcraft_visible", name)), value));
}

#[cfg(not(target_arch = "wasm32"))]
fn snapshot(app: &WordApp, ui: &mut Ui, refresh: bool) -> GitCache {
    let id = egui::Id::new("gitcraft_workspace_cache");
    let path = app.session.path.clone();
    let cached = ui.data(|d| d.get_temp::<GitCache>(id)).unwrap_or_default();
    if !refresh && cached.path == path {
        return cached;
    }
    let mut result = GitCache { path: path.clone(), ..Default::default() };
    if let Some(path) = path {
        match Repo::discover(&path).and_then(|repo| repo.workspace_snapshot()) {
            Ok(value) => result.snapshot = Some(value),
            Err(e) => result.error = Some(e.to_string()),
        }
    }
    ui.data_mut(|d| d.insert_temp(id, result.clone()));
    result
}

pub fn show(app: &mut WordApp, ui: &mut Ui) {
    // This toolbar is deliberately small: the page stays the dominant region.
    let mut left = visible(ui, "branches");
    let mut right = visible(ui, "review");
    ui.horizontal(|ui| {
        if ui.selectable_label(left, "GitCraft").on_hover_text("Show/hide branch graph").clicked() {
            left = !left;
            set_visible(ui, "branches", left);
        }
        if ui.selectable_label(right, "Review").on_hover_text("Show/hide review discussions").clicked() {
            right = !right;
            set_visible(ui, "review", right);
        }
        ui.label(egui::RichText::new("Document-first workspace").small().weak());
    });
    if left {
        let t = Tokens::get(ui.ctx());
        egui::Panel::left("gitcraft_branches_pane")
            .default_size(205.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(8).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| branches(app, ui));
    }
    if right {
        let t = Tokens::get(ui.ctx());
        egui::Panel::right("gitcraft_review_pane")
            .default_size(255.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(8).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| review(app, ui));
    }
}

fn branches(app: &mut WordApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading("GitCraft");
        if ui.small_button("×").on_hover_text("Hide branches").clicked() {
            set_visible(ui, "branches", false);
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cached = snapshot(app, ui, false);
        if let Some(s) = cached.snapshot {
            ui.label(egui::RichText::new(format!("● {}", s.branch)).strong());
            if let Some(remote) = &s.upstream {
                ui.label(egui::RichText::new(format!("Tracking: {remote}")).small().weak());
                let status = if s.ahead == 0 && s.behind == 0 {
                    "In sync with last fetched upstream".to_string()
                } else {
                    format!("↑ {} local  ↓ {} remote", s.ahead, s.behind)
                };
                ui.label(status);
            } else {
                ui.label("No upstream configured");
            }
            if s.dirty { ui.colored_label(ui.visuals().warn_fg_color, "● Local modifications"); }
            if ui.small_button("Refresh local Git state").clicked() {
                let _ = snapshot(app, ui, true);
            }
            ui.separator();
            ui.label(egui::RichText::new("Commit graph · all branches").strong());
            egui::ScrollArea::both().show(ui, |ui| {
                if s.graph.is_empty() {
                    ui.label("No commits yet");
                } else {
                    for line in &s.graph {
                        ui.label(egui::RichText::new(line).monospace().small());
                    }
                }
            });
            ui.label(egui::RichText::new("Remote status reflects the last fetch.").small().weak());
        } else {
            ui.label("Open a saved document inside a local Git repository to see its branches.");
            if let Some(err) = cached.error {
                ui.label(egui::RichText::new(err).small().weak());
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = app;
        ui.label("The local Git graph is available in the desktop application.");
    }
}

fn review(app: &mut WordApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading("Review");
        if ui.small_button("×").on_hover_text("Hide review pane").clicked() {
            set_visible(ui, "review", false);
        }
    });
    let tab_id = egui::Id::new("gitcraft_review_tab");
    let mut tab = ui.data(|d| d.get_temp::<u8>(tab_id)).unwrap_or(0);
    ui.horizontal(|ui| {
        for (n, label) in ["Discussions", "Changes", "Activity"].iter().enumerate() {
            if ui.selectable_label(tab == n as u8, *label).clicked() { tab = n as u8; }
        }
    });
    ui.data_mut(|d| d.insert_temp(tab_id, tab));
    ui.separator();
    match tab {
        0 => {
            ui.label(egui::RichText::new("Document comments").strong());
            panes::comments(app, ui);
            ui.label(egui::RichText::new("Forge discussions: integration pending").small().weak());
        }
        1 => {
            ui.label(egui::RichText::new("Changes").strong());
            #[cfg(not(target_arch = "wasm32"))]
            {
                let cached = snapshot(app, ui, false);
                if let Some(s) = cached.snapshot {
                    ui.label(if s.dirty { "Uncommitted changes detected" } else { "No uncommitted changes reported" });
                    ui.label(format!("Ahead: {} · Behind: {}", s.ahead, s.behind));
                    ui.label(egui::RichText::new("Block-level differences are not implemented yet.").small().weak());
                } else {
                    ui.label("Open a document in a Git repository.");
                }
            }
            #[cfg(target_arch = "wasm32")]
            ui.label("Local changes require the desktop application.");
        }
        _ => {
            ui.label(egui::RichText::new("Recent Git activity").strong());
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(s) = snapshot(app, ui, false).snapshot {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for line in s.graph.iter().take(20) {
                            ui.label(egui::RichText::new(line).monospace().small());
                        }
                    });
                } else {
                    ui.label("No local repository activity available.");
                }
            }
            #[cfg(target_arch = "wasm32")]
            ui.label("Local activity requires the desktop application.");
        }
    }
}
