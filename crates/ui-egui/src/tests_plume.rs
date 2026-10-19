//! Plume's Markdown profile in the interface: Office-only tabs and buttons are not drawn while
//! it is on, and come back when it is off.

use egui_kittest::kittest::Queryable;

use crate::{Services, WordApp};

fn harness(strict: bool) -> egui_kittest::Harness<'static, WordApp> {
    let mut s = wordcraft_engine::Session::new(wordcraft_engine::sample::sample_document());
    s.native_markdown = true;
    s.prefs.markdown_strict = strict;
    let mut a = WordApp::new(s, Services::default());
    a.ui.tab = "Home".into();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1800.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    );
    h.run_steps(4);
    h
}

#[test]
fn the_profile_hides_office_tabs_and_buttons() {
    let h = harness(true);
    for tab in ["Home", "Insert", "References", "Review", "View"] {
        assert!(h.query_by_label(tab).is_some(), "{tab} is shown");
    }
    for tab in ["Draw", "Design", "Layout", "Mailings", "Developer"] {
        assert!(h.query_by_label(tab).is_none(), "{tab} is hidden");
    }
    assert!(h.query_by_label("Bold").is_some() && h.query_by_label("Italic").is_some());
    for b in ["Increase Font Size", "Decrease Font Size", "Phonetic Guide"] {
        assert!(h.query_by_label(b).is_none(), "{b} is hidden");
    }
}

/// Export offers PDF, Markdown and the web and text formats, not the Office ones.
#[test]
fn export_leaves_out_the_office_formats() {
    for (strict, office) in [(true, false), (false, true)] {
        let mut h = harness(strict);
        let _ = h.state_mut().run("ui.backstage", serde_json::json!({"value": true, "page": "export"}));
        h.run_steps(4);
        assert!(h.query_by_label("PDF document (*.pdf)").is_some() && h.query_by_label("Markdown (*.md)").is_some());
        for f in ["Word document (*.docx)", "OpenDocument Text (*.odt)", "Rich Text Format (*.rtf)"] {
            assert_eq!(h.query_by_label(f).is_some(), office, "{f} with the profile {}", if strict { "on" } else { "off" });
        }
    }
}

#[test]
fn without_the_profile_everything_is_back() {
    let h = harness(false);
    for tab in ["Draw", "Design", "Layout", "Mailings"] {
        assert!(h.query_by_label(tab).is_some(), "{tab} is shown");
    }
    assert!(h.query_by_label("Increase Font Size").is_some());
}

/// A Markdown document is named by its front matter title: in the window title and in the
/// Recent list, its file under it.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_markdown_document_is_named_by_its_title() {
    let dir = std::env::temp_dir().join(format!("plume-title-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let md = dir.join("guide-install.md");
    std::fs::write(&md, "---\ntitle: Guide d'installation\ntoto: valeur\n---\n\nTexte.\n").unwrap();
    let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
    s.native_markdown = true;
    s.run("file.open", &serde_json::json!({"path": md.to_string_lossy()})).unwrap();
    let mut a = WordApp::new(s, Services::default());
    assert_eq!(a.display_title(), "Guide d'installation");
    assert_eq!(a.title_stem(), "guide-install", "the file name stays the suggested name");
    assert_eq!(super::backstage_recent_title(&md.to_string_lossy()).as_deref(), Some("Guide d'installation"));
    a.ui.recent = vec![md.to_string_lossy().to_string()];
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1800.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    );
    h.run_steps(4);
    assert!(h.state().sent_title.starts_with("Guide d'installation"), "{}", h.state().sent_title);
    // File › Info shows the front matter's other fields, editable.
    let _ = h.state_mut().run("ui.backstage", serde_json::json!({"value": true, "page": "info"}));
    h.run_steps(4);
    assert!(h.query_by_label("Front matter").is_some() && h.query_by_label("toto").is_some());
    // Open lists it by its title.
    let _ = h.state_mut().run("ui.backstage", serde_json::json!({"value": true, "page": "open"}));
    h.run_steps(4);
    assert!(h.query_by_label("Guide d'installation").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Insert › Markdown: Code Block and Callout; in a code block, Leave Block too.
#[test]
fn insert_offers_markdown_blocks() {
    let mut h = harness(true);
    h.state_mut().ui.tab = "Insert".into();
    h.run_steps(4);
    assert!(h.query_by_label("Code Block").is_some() && h.query_by_label("Callout").is_some());
    assert!(h.query_by_label("Leave Block").is_none());
    h.state_mut().run("markdown.codeBlock", serde_json::json!({"language": "js"})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Leave Block").is_some());
}

/// A locked main branch: Save Version says the version goes on the person's work branch and
/// offers to request the merge into main.
#[test]
fn save_version_offers_the_merge_request() {
    use wordcraft_engine::vcs::forge::{DocRef, Forge};
    let mut h = harness(true);
    h.state_mut().session.forge = Some(wordcraft_engine::cmd::versions::Remote {
        doc: DocRef { forge: Forge::github(), repo: "acme/docs".into(), branch: "ada-docs".into(), path: "guide.md".into() },
        sha: "1".into(),
        review: None,
        imported: Default::default(),
        threads: Default::default(),
        sent: Default::default(),
        workflow: Some(wordcraft_engine::cmd::workflow::Workflow { target: "main".into(), branch: "ada-docs".into(), fresh: false }),
    });
    let _ = h.state_mut().run("ui.dialog", serde_json::json!({"name": "saveVersion"}));
    h.run_steps(4);
    assert!(h.query_by_label("Request the merge into main").is_some());
    assert!(h.query_by_label("Saved on your branch ada-docs; main is locked.").is_some());
}

/// Insert › Markdown › Diagram; the diagram editor shows the source and its preview.
#[test]
fn the_diagram_editor_previews_the_source() {
    let mut h = harness(true);
    h.state_mut().ui.tab = "Insert".into();
    h.run_steps(4);
    assert!(h.query_by_label("Diagram").is_some());
    h.state_mut().run("markdown.diagram", serde_json::json!({"source": "flowchart LR\n    A --> B"})).unwrap();
    h.run_steps(4);
    assert!(h.state_mut().session.run("markdown.block", &serde_json::json!({})).unwrap().get("diagram").is_some(), "the caret is on the diagram");
    let _ = h.state_mut().run("ui.dialog", serde_json::json!({"name": "diagram"}));
    h.run_steps(4);
    assert!(h.query_by_label("Mermaid source").is_some() && h.query_by_label("Preview").is_some());
}
