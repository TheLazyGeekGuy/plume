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
