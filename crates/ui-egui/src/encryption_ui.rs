//! Plume: whether the document is saved encrypted, shown apart from its classification (a label
//! encrypts nothing): a padlock in the status bar, closed or open, always with its words, and a
//! menu that says what the encryption hides and what it doesn't, and saves an encrypted copy.

use egui::{Color32, Ui};
use serde_json::json;
use wordcraft_engine::classification::{self, EncryptionAdvice};

use crate::WordApp;
use crate::file_dialogs::{AfterPick, AfterSave, FileDialogRequest};

/// How the document is saved encrypted (`aescrypt`, `zip-aes`, `word`), if it is.
pub fn method(app: &WordApp) -> Option<&'static str> {
    wordcraft_engine::cmd::file::encryption_of(&app.session)
}

/// Does the document's classification ask for it to be encrypted?
fn level_asks(app: &WordApp) -> bool {
    let labels = classification::read(&app.session.doc);
    classification::current_level(&app.session.prefs.classification, &labels)
        .is_some_and(|(_, l)| matches!(l.encryption, EncryptionAdvice::Required | EncryptionAdvice::WarnUnencrypted))
}

/// What an encrypted file still shows of itself.
fn exposure(method: &str) -> &'static str {
    match method {
        "zip-aes" => tl!("File names, sizes and dates inside the ZIP stay readable; only their contents are encrypted."),
        _ => tl!("The file's name and its approximate size stay visible; nothing inside does."),
    }
}

/// The method's name (names of formats, not translated).
fn method_name(method: &str) -> &'static str {
    match method {
        "aescrypt" => "AES Crypt (AES-256)",
        "zip-aes" => "ZIP AES-256",
        _ => "Office (AES)",
    }
}

/// The status bar's padlock (none in a build without encryption: nothing it offers would work).
pub fn status(app: &mut WordApp, ui: &mut Ui) {
    if !wordcraft_engine::io::ENCRYPTION {
        return;
    }
    let m = method(app);
    let (text, icon, color) = match m {
        Some(_) => (tl!("Encrypted"), "lock", Color32::from_rgb(0x2E, 0x7D, 0x32)),
        None if level_asks(app) => (tl!("Not encrypted"), "unlock", Color32::from_rgb(0xB2, 0x6A, 0x00)),
        None => (tl!("Not encrypted"), "unlock", Color32::from_rgb(0x9E, 0x9E, 0x9E)),
    };
    crate::classification_ui::chip(ui, app, text, Some((color, icon.to_string())), &format!("{}: {text}", tl!("Encryption")), menu);
}

/// The padlock's menu.
fn menu(ui: &mut Ui, app: &mut WordApp) {
    ui.set_max_width(320.0);
    match method(app) {
        Some(m) => {
            ui.label(egui::RichText::new(tl!("This document is saved encrypted.")).strong());
            ui.label(format!("{}: {}", tl!("Method"), method_name(m)));
            ui.label(egui::RichText::new(exposure(m)).small());
            ui.label(egui::RichText::new(tl!("Without the password nobody, Plume included, can open it.")).small().weak());
            ui.separator();
            if ui.button(tl!("Change Password…")).clicked() {
                let _ = app.run("file.encrypt", json!({}));
                ui.close();
            }
            if ui.button(tl!("Save an Unencrypted Copy…")).clicked() {
                ui.close();
                let name = format!("{}.md", app.title_stem());
                let _ = app.ask_file(FileDialogRequest::Save { name }, AfterPick::Export { ext: "md".into() });
            }
        }
        None => {
            ui.label(egui::RichText::new(tl!("Anyone who gets this file can read it.")).strong());
            if level_asks(app) {
                ui.label(egui::RichText::new(tl!("Its classification asks for encryption.")).color(Color32::from_rgb(0xB2, 0x6A, 0x00)));
            }
            ui.label(egui::RichText::new(tl!("A classification label doesn't encrypt the document.")).small().weak());
            ui.separator();
            if ui.button(tl!("Save an Encrypted Copy…")).clicked() {
                ui.close();
                save_encrypted_copy(app);
            }
            let path = app.session.path.as_ref().map(|p| p.to_string_lossy().to_string());
            if path.as_deref().is_some_and(wordcraft_engine::io::can_encrypt) && ui.button(tl!("Encrypt with Password…")).clicked() {
                let _ = app.run("file.encrypt", json!({}));
                ui.close();
            }
        }
    }
}

/// Save As an encrypted file: `<name>.md.aes`, or `<name>.textpack.aes` when the document has
/// pictures (a lone Markdown file can't hold them). Saving asks for the password.
pub fn save_encrypted_copy(app: &mut WordApp) {
    let pictures = !wordcraft_engine::io::markdown_bundle(&app.session.doc).assets.is_empty();
    let name = format!("{}.{}", app.title_stem(), if pictures { "textpack.aes" } else { "md.aes" });
    if app.services.download.is_some() {
        // The web: Save As's name and format.
        app.ui.backstage = true;
        app.ui.backstage_page = "saveAs".into();
        return;
    }
    let _ = app.ask_file(FileDialogRequest::Save { name }, AfterPick::SaveAs { after: AfterSave::Nothing, document: app.session.document_id() });
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "encryption")]
    use serde_json::json;

    use crate::{Services, WordApp};

    /// Without encryption, saving as `.md.aes` says so and writes nothing; no password is asked.
    #[cfg(not(feature = "encryption"))]
    #[test]
    fn without_encryption_nothing_is_encrypted() {
        let dir = std::env::temp_dir().join(format!("plume-noenc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Payroll"));
        s.native_markdown = true;
        let mut a = WordApp::new(s, Services::default());
        let path = dir.join("pay.md.aes").to_string_lossy().to_string();
        let e = a.run("file.saveAs", serde_json::json!({"path": path})).unwrap_err();
        assert!(e.contains("no encryption"), "{e}");
        assert!(a.dialog.is_none() && !std::path::Path::new(&path).exists());
        assert!(a.run("file.encrypt", serde_json::json!({"password": "pw"})).is_err());
        assert!(super::method(&a).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Saving as `.md.aes` without a password asks for one first, then saves encrypted; the
    /// padlock says so, and a C4 document says it is waiting for encryption until then.
    #[cfg(feature = "encryption")]
    #[test]
    fn saving_encrypted_asks_for_the_password_then_shows_the_padlock() {
        let dir = std::env::temp_dir().join(format!("plume-padlock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Payroll"));
        s.native_markdown = true;
        let mut a = WordApp::new(s, Services::default());
        a.run("classification.set", json!({"level": "C4"})).unwrap();
        assert!(super::method(&a).is_none() && super::level_asks(&a));
        let path = dir.join("pay.md.aes").to_string_lossy().to_string();
        let r = a.run("file.saveAs", json!({"path": path})).unwrap();
        assert_eq!(r["pending"], "password");
        assert_eq!(a.dialog.as_ref().map(|d| d.name()), Some("encryptPassword"));
        assert!(!std::path::Path::new(&path).exists(), "nothing written before the password");
        // What the dialog's OK does.
        a.run("file.encrypt", json!({"password": "pw"})).unwrap();
        let (id, params) = a.after_encrypt.take().unwrap();
        a.run(&id, params).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(wordcraft_crypt::detect(&bytes), wordcraft_crypt::Kind::AesCrypt(3));
        assert_eq!(super::method(&a), Some("aescrypt"));
        assert_eq!(a.title_stem(), "pay");
        assert_eq!(a.default_save_name(), "pay.md.aes");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
