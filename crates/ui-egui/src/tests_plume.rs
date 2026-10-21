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

/// Insert › Markdown › Callout shows each kind as on the page; clicking one makes the
/// paragraph that callout. Diagram shows a picture of each kind of its first group (one of
/// them is drawn wider than a texture may be: it is scaled down, never refused).
#[test]
fn the_callout_and_diagram_menus_are_galleries() {
    let mut h = harness(true);
    h.state_mut().ui.tab = "Insert".into();
    h.run_steps(4);
    h.get_by_label("Callout").click();
    h.run_steps(4);
    for kind in ["Note", "Tip", "Important", "Warning", "Caution", "Info", "Danger", "Details"] {
        assert!(h.query_by_label(kind).is_some(), "{kind}");
    }
    h.get_by_label("Tip").click();
    h.run_steps(4);
    let block = h.state_mut().session.run("markdown.block", &serde_json::json!({})).unwrap();
    assert_eq!(block["callout"]["kind"], "tip", "{block}");
    h.get_by_label("Diagram").click();
    h.run_steps(4);
    assert!(h.query_by_label("Flowchart").is_some() && h.query_by_label("Sequence Diagram").is_some());
    h.get_by_label("Structure").click();
    h.run_steps(4);
    assert!(h.query_by_label("Flowchart").is_none(), "one group at a time");
}

/// The web version has no Zotero tab (it can't reach the Zotero app); the desktop app keeps it,
/// in both profiles.
#[test]
fn the_web_version_has_no_zotero_tab() {
    use crate::widgets::tab_hidden_in;
    assert!(tab_hidden_in("Zotero", false, true) && tab_hidden_in("Zotero", true, true));
    assert!(!tab_hidden_in("Zotero", false, false) && !tab_hidden_in("Zotero", true, false));
    assert!(!tab_hidden_in("Insert", true, true), "other tabs stay");
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

/// Insert › Markdown › Diagram; a flowchart opens on the canvas of shapes and connectors, with
/// its preview, never as Mermaid text.
#[test]
fn the_diagram_editor_draws_shapes_and_connectors() {
    let mut h = harness(true);
    h.state_mut().ui.tab = "Insert".into();
    h.run_steps(4);
    assert!(h.query_by_label("Diagram").is_some());
    h.state_mut().run("markdown.diagram", serde_json::json!({"source": "flowchart LR\n    A --> B"})).unwrap();
    h.run_steps(4);
    assert!(h.state_mut().session.run("markdown.block", &serde_json::json!({})).unwrap().get("diagram").is_some(), "the caret is on the diagram");
    let _ = h.state_mut().run("ui.dialog", serde_json::json!({"name": "diagram"}));
    h.run_steps(4);
    assert!(h.query_by_label("Flowchart").is_some() && h.query_by_label("Preview").is_some());
    assert!(h.query_by_label("Arrange").is_some() && h.query_by_label("Mermaid source").is_none());
    let Some(crate::dialogs::Dialog::Diagram(e)) = &h.state().dialog else { panic!("the diagram editor is open") };
    let Some(wordcraft_engine::diagram::codec::Diagram { kind, model: wordcraft_engine::diagram::codec::Model::Graph(g), .. }) = &e.diagram else {
        panic!("a flowchart on the canvas")
    };
    assert_eq!((*kind, g.nodes.len(), g.edges.len()), (wordcraft_engine::diagram::codec::Kind::Flowchart, 2, 1));
}

/// View › Markdown Source shows the document as its file is written; edited and applied, the
/// document follows.
#[test]
fn the_markdown_source_pane_edits_the_document() {
    let mut h = harness(true);
    let _ = h.state_mut().run("view.markdownSource", serde_json::json!({"value": true}));
    h.run_steps(4);
    assert!(h.query_by_label("Markdown Source").is_some() && h.query_by_label("Apply").is_some());
    let text = h.state().md_source.as_ref().map(|(_, t, _)| t.clone()).unwrap();
    assert!(text.contains("# The Open Studio Handbook"), "{text}");
    h.state_mut().md_source = Some((0, text.replace("The Open Studio Handbook", "Le manuel"), true));
    let t = h.state().md_source.as_ref().unwrap().1.clone();
    h.state_mut().run("markdown.setSource", serde_json::json!({"text": t})).unwrap();
    assert!(h.state().session.doc.plain_text(wordcraft_doc::StoryRef::Body).contains("Le manuel"));
}

/// Markdown has no fonts or sizes: the profile draws neither box, nor the Text Effects menu;
/// without the profile they are back.
#[test]
fn the_profile_has_no_font_or_size_boxes() {
    // The boxes show the caret's font and size (the sample's title: Aptos Display, 28 pt).
    let shows = |h: &egui_kittest::Harness<'static, WordApp>, v: &str| h.query_all_by_value(v).next().is_some();
    let h = harness(true);
    assert!(!shows(&h, "Aptos Display") && !shows(&h, "28"), "no font or size box");
    assert!(h.query_by_label("Text Effects and Typography").is_none());
    assert!(h.query_by_label("Clear All Formatting").is_some(), "Clear Formatting stays");
    let h = harness(false);
    assert!(shows(&h, "Aptos Display") && shows(&h, "28"));
    assert!(h.query_by_label("Text Effects and Typography").is_some());
}

/// View › Markdown: Page, Side by Side and Source; the command steps through them, and is only
/// for a Markdown document.
#[test]
fn the_markdown_view_steps_through_page_split_and_source() {
    let mut s = wordcraft_engine::Session::new(wordcraft_engine::sample::sample_document());
    s.native_markdown = true;
    let mut a = WordApp::new(s, Services::default());
    let value = |a: &mut WordApp, p| a.run("view.markdownView", p).unwrap()["value"].as_str().unwrap_or("").to_string();
    assert_eq!(value(&mut a, serde_json::json!({})), "split");
    assert!(a.session.view.markdown_source && !a.session.view.markdown_source_only);
    assert_eq!(value(&mut a, serde_json::json!({})), "source");
    assert!(!a.session.view.markdown_source && a.session.view.markdown_source_only);
    assert_eq!(value(&mut a, serde_json::json!({})), "page");
    assert_eq!(value(&mut a, serde_json::json!({"value": "source"})), "source");
    assert!(a.run("view.markdownView", serde_json::json!({"value": "sideways"})).is_err());
    let mut docx = WordApp::new(wordcraft_engine::Session::new(wordcraft_engine::sample::sample_document()), Services::default());
    assert!(docx.run("view.markdownView", serde_json::json!({})).is_err(), "only for Markdown");
}

/// Source typed but not applied yet (the pause hasn't passed) is part of what is saved: any
/// command that saves or replaces the document applies it first.
#[test]
fn typed_markdown_source_is_applied_before_a_save() {
    let mut s = wordcraft_engine::Session::new(wordcraft_engine::sample::sample_document());
    s.native_markdown = true;
    let mut a = WordApp::new(s, Services::default());
    a.md_source = Some((a.session.rev(), "# Typed heading\n\nTyped body.\n".into(), true));
    a.md_source_typed = 0.0;
    let path = std::env::temp_dir().join(format!("plume-src-{}.md", std::process::id()));
    a.run("file.saveAs", serde_json::json!({"path": path.to_string_lossy()})).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    assert!(saved.contains("# Typed heading") && saved.contains("Typed body."), "{saved}");
    assert!(a.md_source.as_ref().is_some_and(|(_, _, edited)| !edited));
}

/// The title bar takes the level's colour with ink that reads on it; a colour too close to the
/// plain bar (black in the dark theme, white in the light one) gets the striped edge.
#[test]
fn the_title_bar_tint_stays_readable() {
    use crate::classification_ui::tint;
    use egui::Color32;
    let (dark_bar, light_bar) = (Color32::from_rgb(0x1F, 0x1F, 0x1F), Color32::from_rgb(0xF0, 0xF0, 0xF0));
    let black = tint(Color32::BLACK, dark_bar);
    assert!(black.stripes && black.ink == Color32::WHITE, "black over the dark theme is striped");
    assert!(!tint(Color32::BLACK, light_bar).stripes);
    let red = tint(Color32::from_rgb(0xC6, 0x28, 0x28), dark_bar);
    assert!(!red.stripes && red.ink == Color32::WHITE);
    let amber = tint(Color32::from_rgb(0xFF, 0xC0, 0x00), light_bar);
    assert!(amber.ink != Color32::WHITE, "dark ink on a pale level");
    assert!(tint(Color32::WHITE, light_bar).stripes, "TLP:CLEAR over the light theme is striped");
}

/// C4 is black, and the title bar shows it only while the option is on.
#[test]
fn a_highly_restricted_document_tints_the_title_bar_black() {
    let mut s = wordcraft_engine::Session::new(wordcraft_engine::sample::sample_document());
    s.native_markdown = true;
    let mut a = WordApp::new(s, Services::default());
    let plain = egui::Color32::from_rgb(0x1F, 0x1F, 0x1F);
    assert!(crate::classification_ui::title_tint(&a, plain).is_none(), "unclassified: plain bar");
    a.run("classification.set", serde_json::json!({"level": "C4"})).unwrap();
    let t = crate::classification_ui::title_tint(&a, plain).unwrap();
    assert_eq!(t.fill, egui::Color32::BLACK);
    assert!(t.stripes);
    a.run("classification.options", serde_json::json!({"colorTitleBar": false})).unwrap();
    assert!(crate::classification_ui::title_tint(&a, plain).is_none());
}
