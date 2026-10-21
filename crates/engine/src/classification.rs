//! Plume: document classification. A document carries labels, independent of each other, in its
//! front matter (`plume:`):
//!
//! ```yaml
//! plume:
//!   classification:
//!     reference: plume      # the referential the level belongs to
//!     version: "1.0"
//!     level: C2
//!   context: personal       # personal | professional | associative | other
//!   visibility: private     # private | shared | public
//!   importance: normal      # normal | important | strategic
//!   personal_data: none     # none | present | sensitive
//! ```
//!
//! A referential has a stable id, a version, and its levels: code, label, description, colour,
//! and its rules (publication, encryption advice). Plume ships `plume` (C0–C4) and TLP 2.0; the
//! user adds their own (File › Options › Document Classification). A referential Plume doesn't
//! know is kept as written, never rewritten. Changing a document's referential takes its new
//! level too: a document is never reclassified silently.
//!
//! The labels express a declared classification; they don't protect the file (that is
//! encryption's job, a separate thing).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_formats::front_matter as fm;

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// What a level asks before the document is published (saved to a forge, shared).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PublishRule {
    /// Nothing to say.
    #[default]
    Allow,
    /// A warning, shown once.
    Warn,
    /// An explicit decision: the publication waits for it.
    Confirm,
}

/// What a level advises about encryption when the document leaves its place (exports, sharing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EncryptionAdvice {
    #[default]
    Optional,
    /// Recommended for exchanges.
    Exchange,
    /// Recommended for exports and sharing.
    Export,
    /// A strong warning when exported unencrypted.
    WarnUnencrypted,
    /// Required for transport exports.
    Required,
}

/// One level of a referential.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Level {
    /// Stable code written in the front matter (`C2`, `TLP:AMBER`).
    pub code: String,
    pub label: String,
    pub description: String,
    /// `#RRGGBB`: the label's colour, always beside its text, never alone.
    pub color: String,
    /// The pictogram drawn before the label: `dot`, `shield` or `lock`.
    pub icon: String,
    pub publish: PublishRule,
    pub encryption: EncryptionAdvice,
}

/// A classification referential.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Referential {
    /// Stable identifier (`plume`, `tlp`, a custom one).
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub levels: Vec<Level>,
}

impl Referential {
    pub fn level(&self, code: &str) -> Option<&Level> {
        self.levels.iter().find(|l| l.code.eq_ignore_ascii_case(code))
    }
}

/// How the labels show in the window's title bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Presentation {
    /// One label per dimension when there's room, one compact label otherwise.
    #[default]
    Full,
    /// Always one compact label.
    Compact,
    /// Not in the title bar (the status bar still shows them).
    Hidden,
}

/// File › Options › Document Classification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The referential a document's first level is taken from.
    pub default_reference: String,
    /// The user's own referentials.
    pub custom: Vec<Referential>,
    pub presentation: Presentation,
    /// PDF, HTML and DOCX exports show the level (and the PDF template its footer).
    pub mark_exports: bool,
    /// Warn before publishing a document its level asks to keep (forges).
    pub warn_on_publish: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { default_reference: PLUME.into(), custom: Vec::new(), presentation: Presentation::Full, mark_exports: true, warn_on_publish: true }
    }
}

/// Plume's own referential's id.
pub const PLUME: &str = "plume";
/// TLP 2.0's id.
pub const TLP: &str = "tlp";

fn level(code: &str, label: &str, description: &str, color: &str, icon: &str, publish: PublishRule, encryption: EncryptionAdvice) -> Level {
    Level { code: code.into(), label: label.into(), description: description.into(), color: color.into(), icon: icon.into(), publish, encryption }
}

/// The referentials Plume ships: `plume` (C0–C4) and TLP 2.0 (FIRST's Traffic Light Protocol).
pub fn builtin() -> Vec<Referential> {
    use EncryptionAdvice as E;
    use PublishRule as P;
    vec![
        Referential {
            id: PLUME.into(),
            version: "1.0".into(),
            name: "Plume".into(),
            description: "Plume's five levels, from public to highly restricted.".into(),
            levels: vec![
                level("C0", "Public", "Anyone may read it.", "#2E7D32", "dot", P::Allow, E::Optional),
                level("C1", "Internal", "For the people of the organisation or group.", "#1565C0", "dot", P::Warn, E::Exchange),
                level("C2", "Confidential", "For the people who need it; not to be published.", "#B26A00", "shield", P::Confirm, E::Export),
                level(
                    "C3",
                    "Restricted",
                    "For named people only; disclosure would cause serious harm.",
                    "#C62828",
                    "shield",
                    P::Confirm,
                    E::WarnUnencrypted,
                ),
                level(
                    "C4",
                    "Highly restricted",
                    "For named people only, under strict control; never exported unencrypted.",
                    "#6A1B9A",
                    "lock",
                    P::Confirm,
                    E::Required,
                ),
            ],
        },
        Referential {
            id: TLP.into(),
            version: "2.0".into(),
            name: "TLP".into(),
            description: "FIRST's Traffic Light Protocol, version 2.0.".into(),
            levels: vec![
                level("TLP:CLEAR", "TLP:CLEAR", "Recipients may share it without limit.", "#FFFFFF", "dot", P::Allow, E::Optional),
                level(
                    "TLP:GREEN",
                    "TLP:GREEN",
                    "Recipients may share it within their community, not publicly.",
                    "#33FF00",
                    "dot",
                    P::Warn,
                    E::Exchange,
                ),
                level(
                    "TLP:AMBER",
                    "TLP:AMBER",
                    "Recipients may share it only within their organisation and its clients, on a need-to-know basis.",
                    "#FFC000",
                    "shield",
                    P::Confirm,
                    E::Export,
                ),
                level(
                    "TLP:AMBER+STRICT",
                    "TLP:AMBER+STRICT",
                    "Recipients may share it only within their organisation, on a need-to-know basis.",
                    "#FFC000",
                    "shield",
                    P::Confirm,
                    E::WarnUnencrypted,
                ),
                level("TLP:RED", "TLP:RED", "For the eyes and ears of the individual recipients only.", "#FF2B2B", "lock", P::Confirm, E::Required),
            ],
        },
    ]
}

/// Every referential: the built-in ones, then the user's.
pub fn all(settings: &Settings) -> Vec<Referential> {
    let mut v = builtin();
    v.extend(settings.custom.iter().filter(|c| !v.iter().any(|b| b.id == c.id)).cloned().collect::<Vec<_>>());
    v
}

/// The referential `id`, if Plume knows it.
pub fn referential(settings: &Settings, id: &str) -> Option<Referential> {
    all(settings).into_iter().find(|r| r.id.eq_ignore_ascii_case(id))
}

/// The documents' dimensions besides the level: (front matter key, command key, values).
pub const DIMENSIONS: [(&str, &str, &[&str]); 4] = [
    ("context", "context", &["personal", "professional", "associative", "other"]),
    ("visibility", "visibility", &["private", "shared", "public"]),
    ("importance", "importance", &["normal", "important", "strategic"]),
    ("personal_data", "personalData", &["none", "present", "sensitive"]),
];

/// A document's classification as its front matter has it (values as written).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Labels {
    pub reference: Option<String>,
    pub version: Option<String>,
    pub level: Option<String>,
    pub context: Option<String>,
    pub visibility: Option<String>,
    pub importance: Option<String>,
    pub personal_data: Option<String>,
}

const ROOT: &str = "plume";
const CLASS: &str = "classification";

/// The document's labels.
pub fn read(doc: &Document) -> Labels {
    let body = fm::stored(doc);
    let c = |k: &str| fm::get_path(&body, &[ROOT, CLASS, k]);
    let d = |k: &str| fm::get_path(&body, &[ROOT, k]);
    Labels {
        reference: c("reference"),
        version: c("version"),
        level: c("level"),
        context: d("context"),
        visibility: d("visibility"),
        importance: d("importance"),
        personal_data: d("personal_data"),
    }
}

fn write(doc: &mut Document, path: &[&str], value: &str) {
    let body = fm::set_path(&fm::stored(doc), path, value);
    fm::store(doc, &body);
}

/// The level the document has, in a referential Plume knows (`None` when unclassified or the
/// referential is unknown).
pub fn current_level(settings: &Settings, labels: &Labels) -> Option<(Referential, Level)> {
    let r = referential(settings, labels.reference.as_deref().unwrap_or(&settings.default_reference))?;
    let l = r.level(labels.level.as_deref()?)?.clone();
    Some((r, l))
}

/// The labels, as commands give them.
fn labels_json(settings: &Settings, l: &Labels) -> Value {
    let reference = l.reference.clone().or_else(|| l.level.as_ref().map(|_| settings.default_reference.clone()));
    let known = reference.as_deref().and_then(|r| referential(settings, r));
    let level = known.as_ref().zip(l.level.as_deref()).and_then(|(r, c)| r.level(c));
    json!({
        "reference": reference,
        "referenceName": known.as_ref().map(|r| r.name.clone()),
        "referenceKnown": known.is_some() || reference.is_none(),
        "version": l.version.clone().or_else(|| known.as_ref().filter(|_| l.level.is_some()).map(|r| r.version.clone())),
        "level": l.level,
        "levelLabel": level.map(|x| x.label.clone()),
        "color": level.map(|x| x.color.clone()),
        "icon": level.map(|x| x.icon.clone()),
        "context": l.context,
        "visibility": l.visibility,
        "importance": l.importance,
        "personalData": l.personal_data,
    })
}

/// What publishing (or exporting) the document asks, by its level: warnings, and whether an
/// explicit decision is needed.
pub fn check(settings: &Settings, labels: &Labels, action: &str) -> Value {
    let mut warnings: Vec<String> = Vec::new();
    let mut confirm = false;
    if let Some((r, l)) = current_level(settings, labels) {
        match action {
            "publish" | "share" if settings.warn_on_publish => match l.publish {
                PublishRule::Allow => {}
                PublishRule::Warn => warnings.push(format!("{} — {} ({}): check it may be shared there.", l.code, l.label, r.name)),
                PublishRule::Confirm => {
                    confirm = true;
                    warnings.push(format!("{} — {} ({}): {}", l.code, l.label, r.name, l.description));
                }
            },
            "export" => match l.encryption {
                EncryptionAdvice::WarnUnencrypted | EncryptionAdvice::Required => {
                    confirm = true;
                    warnings.push(format!("{} — {} ({}): an unencrypted copy leaves its protection behind.", l.code, l.label, r.name));
                }
                EncryptionAdvice::Export => warnings.push(format!("{} — {} ({}): encrypting exports is recommended.", l.code, l.label, r.name)),
                _ => {}
            },
            _ => {}
        }
    } else if labels.reference.as_deref().is_some_and(|r| referential(settings, r).is_none()) && matches!(action, "publish" | "share") {
        warnings.push(format!("the referential {:?} isn't known here: check its rules before publishing", labels.reference.as_deref().unwrap_or("")));
    }
    if labels.personal_data.as_deref() == Some("sensitive") && matches!(action, "publish" | "share") {
        confirm = true;
        warnings.push("the document holds sensitive personal data".into());
    }
    json!({"action": action, "warnings": warnings, "confirm": confirm})
}

/// The front matter keys a publication must be acknowledged with (`acknowledged: true`) when the
/// level asks for a decision; an error naming the warnings otherwise.
pub fn require_ack(s: &Session, v: &Value, action: &str) -> Result<(), CmdError> {
    let c = check(&s.prefs.classification, &read(&s.doc), action);
    if c["confirm"] == json!(true) && p::bool(v, "acknowledged") != Some(true) {
        let w: Vec<&str> = c["warnings"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        return Err(CmdError::Failed(format!("classification: {}; publish only after deciding to (`acknowledged: true`)", w.join("; "))));
    }
    Ok(())
}

/// The marking an export shows (`C2 — Confidential`), when the settings ask for one.
pub fn marking(settings: &Settings, doc: &Document) -> Option<String> {
    if !settings.mark_exports {
        return None;
    }
    let labels = read(doc);
    match current_level(settings, &labels) {
        Some((_, l)) if l.code == l.label => Some(l.code),
        Some((_, l)) => Some(format!("{} — {}", l.code, l.label)),
        None => labels.level,
    }
}

/// `doc` as an exported copy shows it (PDF, HTML): the marking as its first line, small and
/// bold, when the settings ask for one. The document itself never gets it.
pub fn marked<'a>(settings: &Settings, doc: &'a Document) -> std::borrow::Cow<'a, Document> {
    let Some(m) = marking(settings, doc) else { return std::borrow::Cow::Borrowed(doc) };
    let mut d = doc.clone();
    let props = wordcraft_doc::CharProps { bold: Some(true), size: Some(9.0), ..Default::default() };
    d.body.insert(0, wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(&m, props)));
    std::borrow::Cow::Owned(d)
}

/// `doc` for a PDF template: the marking in the page footer (`footer-center`), unless the
/// document's front matter sets that footer itself.
pub fn for_template<'a>(settings: &Settings, doc: &'a Document) -> std::borrow::Cow<'a, Document> {
    let body = fm::stored(doc);
    match marking(settings, doc) {
        Some(m) if fm::get(&body, "footer-center").is_none() => {
            let mut d = doc.clone();
            fm::store(&mut d, &fm::set(&body, "footer-center", &m));
            std::borrow::Cow::Owned(d)
        }
        _ => std::borrow::Cow::Borrowed(doc),
    }
}

fn str_or_null<'a>(v: &'a Value, k: &str) -> Result<Option<Option<&'a str>>, CmdError> {
    match v.get(k) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) if s.len() <= 64 => Ok(Some(Some(s.trim()).filter(|s| !s.is_empty()))),
        Some(_) => Err(CmdError::Params(format!("`{k}`: a short string, or null to remove it"))),
    }
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let settings = s.prefs.classification.clone();
    let labels = read(&s.doc);
    // Validate everything first: a refused value changes nothing.
    let mut writes: Vec<(Vec<&str>, String)> = Vec::new();
    for (key, param, values) in DIMENSIONS {
        if let Some(value) = str_or_null(v, param)? {
            if let Some(x) = value
                && !values.contains(&x)
            {
                return Err(CmdError::Params(format!("`{param}`: one of {}", values.join(", "))));
            }
            writes.push((vec![ROOT, key], value.unwrap_or("").to_string()));
        }
    }
    if let Some(level) = str_or_null(v, "level")? {
        match level {
            None => {
                writes.push((vec![ROOT, CLASS, "level"], String::new()));
            }
            Some(code) => {
                let reference = labels.reference.clone().unwrap_or_else(|| settings.default_reference.clone());
                match referential(&settings, &reference) {
                    Some(r) => {
                        let Some(l) = r.level(code) else {
                            let codes: Vec<&str> = r.levels.iter().map(|l| l.code.as_str()).collect();
                            return Err(CmdError::Params(format!("`level`: one of {} ({})", codes.join(", "), r.name)));
                        };
                        writes.push((vec![ROOT, CLASS, "reference"], r.id.clone()));
                        writes.push((vec![ROOT, CLASS, "version"], labels.version.clone().unwrap_or_else(|| r.version.clone())));
                        writes.push((vec![ROOT, CLASS, "level"], l.code.clone()));
                    }
                    // A referential Plume doesn't know: its codes are its own business.
                    None => writes.push((vec![ROOT, CLASS, "level"], code.to_string())),
                }
            }
        }
    }
    for (path, value) in &writes {
        write(&mut s.doc, path, value);
    }
    Ok(labels_json(&settings, &read(&s.doc)))
}

fn set_reference(s: &mut Session, v: &Value) -> CmdResult {
    let settings = s.prefs.classification.clone();
    let id = p::req_str(v, "reference")?;
    let r = referential(&settings, id).ok_or_else(|| CmdError::Params(format!("no referential {id:?} (classification.references lists them)")))?;
    // Never reclassified silently: the level in the new referential is given with it.
    let code =
        p::str(v, "level").ok_or_else(|| CmdError::Params(format!("`level`: the document's level in {}, given with the referential", r.name)))?;
    let l = r
        .level(code)
        .ok_or_else(|| CmdError::Params(format!("`level`: one of {}", r.levels.iter().map(|l| l.code.as_str()).collect::<Vec<_>>().join(", "))))?;
    write(&mut s.doc, &[ROOT, CLASS, "reference"], &r.id);
    write(&mut s.doc, &[ROOT, CLASS, "version"], &r.version);
    write(&mut s.doc, &[ROOT, CLASS, "level"], &l.code);
    Ok(labels_json(&settings, &read(&s.doc)))
}

fn settings_json(st: &Settings) -> Value {
    let mut v = serde_json::to_value(st).unwrap_or_default();
    v["references"] = serde_json::to_value(all(st)).unwrap_or_default();
    v
}

fn options(s: &mut Session, v: &Value) -> CmdResult {
    let st = &mut s.prefs.classification;
    if let Some(id) = p::str(v, "defaultReference") {
        let r = referential(st, id).ok_or_else(|| CmdError::Params(format!("no referential {id:?}")))?;
        st.default_reference = r.id;
    }
    if let Some(pr) = p::str(v, "presentation") {
        st.presentation = match pr {
            "full" => Presentation::Full,
            "compact" => Presentation::Compact,
            "hidden" => Presentation::Hidden,
            _ => return Err(CmdError::Params("`presentation`: full, compact or hidden".into())),
        };
    }
    if let Some(b) = p::bool(v, "markExports") {
        st.mark_exports = b;
    }
    if let Some(b) = p::bool(v, "warnOnPublish") {
        st.warn_on_publish = b;
    }
    Ok(settings_json(st))
}

/// Most custom referentials, and levels in one.
const MAX_CUSTOM: usize = 32;
const MAX_LEVELS: usize = 16;

fn save_reference(s: &mut Session, v: &Value) -> CmdResult {
    let r: Referential = serde_json::from_value(v.get("reference").cloned().unwrap_or(Value::Null))
        .map_err(|e| CmdError::Params(format!("`reference`: a referential ({e})")))?;
    let id_ok = !r.id.is_empty() && r.id.len() <= 32 && r.id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !id_ok || builtin().iter().any(|b| b.id.eq_ignore_ascii_case(&r.id)) {
        return Err(CmdError::Params("`reference.id`: 1–32 letters, digits, `-`, `_` or `.`, not a built-in referential's".into()));
    }
    if r.levels.is_empty() || r.levels.len() > MAX_LEVELS {
        return Err(CmdError::Params(format!("`reference.levels`: 1 to {MAX_LEVELS} levels")));
    }
    for (i, l) in r.levels.iter().enumerate() {
        let code_ok = !l.code.trim().is_empty() && l.code.len() <= 32 && !l.code.contains(['\n', ':', '#']) || l.code.starts_with("TLP:");
        if !code_ok || r.levels.iter().take(i).any(|o| o.code.eq_ignore_ascii_case(&l.code)) {
            return Err(CmdError::Params(format!("level {:?}: a unique code of 1–32 characters", l.code)));
        }
        if l.label.len() > 128 || l.description.len() > 2048 || !(l.color.is_empty() || is_color(&l.color)) {
            return Err(CmdError::Params(format!("level {:?}: label ≤ 128, description ≤ 2048 bytes, color #RRGGBB", l.code)));
        }
    }
    let st = &mut s.prefs.classification;
    st.custom.retain(|c| !c.id.eq_ignore_ascii_case(&r.id));
    if st.custom.len() >= MAX_CUSTOM {
        return Err(CmdError::Params(format!("at most {MAX_CUSTOM} referentials of your own")));
    }
    st.custom.push(r);
    Ok(settings_json(st))
}

fn is_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c.chars().skip(1).all(|x| x.is_ascii_hexdigit())
}

fn delete_reference(s: &mut Session, v: &Value) -> CmdResult {
    let id = p::req_str(v, "id")?;
    let st = &mut s.prefs.classification;
    let before = st.custom.len();
    st.custom.retain(|c| !c.id.eq_ignore_ascii_case(id));
    if st.custom.len() == before {
        return Err(CmdError::Params(format!("no referential of your own named {id:?}")));
    }
    if st.default_reference.eq_ignore_ascii_case(id) {
        st.default_reference = PLUME.into();
    }
    Ok(settings_json(st))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("classification.get", "Classification", "Review › Classification", |s, _| {
            Ok(labels_json(&s.prefs.classification, &read(&s.doc)))
        })
        .params("{} → {reference, referenceName, referenceKnown, version, level, levelLabel, color, icon, context, visibility, importance, personalData}")
        .pure(),
        CommandSpec::new("classification.set", "Set Classification", "Review › Classification", set).params(
            r#"{"level"?: code | null, "context"?: "personal|professional|associative|other" | null, "visibility"?: "private|shared|public" | null, "importance"?: "normal|important|strategic" | null, "personalData"?: "none|present|sensitive" | null}: each dimension on its own, written in the front matter (`plume:`)"#,
        ),
        CommandSpec::new("classification.setReference", "Classification Referential", "Review › Classification", set_reference)
            .params(r#"{"reference": id, "level": code in it}: the document's referential and its level in it, together (never reclassified silently)"#),
        CommandSpec::new("classification.references", "Classification Referentials", "Review › Classification", |s, _| {
            Ok(settings_json(&s.prefs.classification))
        })
        .params("{} → {defaultReference, presentation, markExports, warnOnPublish, custom, references: [{id, version, name, description, levels: [{code, label, description, color, icon, publish, encryption}]}]}")
        .pure(),
        CommandSpec::new("classification.check", "Check Classification", "Review › Classification", |s, v| {
            let action = p::str(v, "action").unwrap_or("publish");
            if !matches!(action, "publish" | "share" | "export") {
                return Err(CmdError::Params("`action`: publish, share or export".into()));
            }
            Ok(check(&s.prefs.classification, &read(&s.doc), action))
        })
        .params(r#"{"action"?: "publish|share|export"} → {warnings, confirm}: what the document's level asks before it leaves"#)
        .pure(),
        CommandSpec::new("classification.options", "Classification Options", "File › Options › Document Classification", options)
            .params(r#"{"defaultReference"?: id, "presentation"?: "full|compact|hidden", "markExports"?: bool, "warnOnPublish"?: bool}"#)
            .pure(),
        CommandSpec::new("classification.saveReference", "Save Classification Referential", "File › Options › Document Classification", save_reference)
            .params(r##"{"reference": {id, version, name, description, levels: [{code, label, description, color: "#RRGGBB", icon: "dot|shield|lock", publish: "allow|warn|confirm", encryption: "optional|exchange|export|warnUnencrypted|required"}]}}"##)
            .pure(),
        CommandSpec::new("classification.deleteReference", "Delete Classification Referential", "File › Options › Document Classification", delete_reference)
            .params(r#"{"id": id of one of your referentials}"#)
            .pure(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session() -> Session {
        Session::new(Document::from_text("Hello"))
    }

    #[test]
    fn dimensions_are_set_independently_and_written_in_the_front_matter() {
        let mut s = session();
        s.run("classification.set", &json!({"context": "personal", "visibility": "private"})).unwrap();
        s.run("classification.set", &json!({"level": "C2"})).unwrap();
        s.run("classification.set", &json!({"importance": "normal", "personalData": "none"})).unwrap();
        let body = fm::stored(&s.doc);
        assert!(body.contains("plume:\n  context: personal\n  visibility: private\n  classification:\n    reference: plume\n    version: \"1.0\"\n    level: C2\n  importance: normal\n  personal_data: none"), "{body}");
        let got = s.run("classification.get", &json!({})).unwrap();
        assert_eq!(
            (got["level"].as_str(), got["levelLabel"].as_str(), got["visibility"].as_str()),
            (Some("C2"), Some("Confidential"), Some("private"))
        );
        // One dimension changes alone, and can be removed.
        s.run("classification.set", &json!({"visibility": null})).unwrap();
        let got = s.run("classification.get", &json!({})).unwrap();
        assert_eq!((got["visibility"].clone(), got["context"].as_str(), got["level"].as_str()), (Value::Null, Some("personal"), Some("C2")));
        // Undo takes a change back.
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.run("classification.get", &json!({})).unwrap()["visibility"], "private");
        for bad in [json!({"context": "secret"}), json!({"level": "C9"}), json!({"visibility": 3})] {
            assert!(matches!(s.run("classification.set", &bad), Err(CmdError::Params(_))), "{bad}");
        }
    }

    #[test]
    fn a_new_referential_comes_with_its_level_and_unknown_ones_are_kept() {
        let mut s = session();
        s.run("classification.set", &json!({"level": "C3"})).unwrap();
        assert!(matches!(s.run("classification.setReference", &json!({"reference": "tlp"})), Err(CmdError::Params(_))), "no silent reclassification");
        s.run("classification.setReference", &json!({"reference": "tlp", "level": "TLP:AMBER"})).unwrap();
        let got = s.run("classification.get", &json!({})).unwrap();
        assert_eq!((got["reference"].as_str(), got["version"].as_str(), got["level"].as_str()), (Some("tlp"), Some("2.0"), Some("TLP:AMBER")));
        // A referential Plume doesn't know: kept as written, its level too.
        let mut s = session();
        let fmatter = "plume:\n  classification:\n    reference: acme-2024\n    version: \"3\"\n    level: SECRET-X\n    extra: kept";
        fm::store(&mut s.doc, fmatter);
        let got = s.run("classification.get", &json!({})).unwrap();
        assert_eq!((got["referenceKnown"].clone(), got["level"].as_str()), (json!(false), Some("SECRET-X")));
        s.run("classification.set", &json!({"context": "professional"})).unwrap();
        assert!(fm::stored(&s.doc).starts_with(fmatter), "{}", fm::stored(&s.doc));
    }

    #[test]
    fn publishing_a_confidential_document_needs_a_decision() {
        let mut s = session();
        assert_eq!(s.run("classification.check", &json!({})).unwrap()["confirm"], false);
        s.run("classification.set", &json!({"level": "C1"})).unwrap();
        let c = s.run("classification.check", &json!({"action": "publish"})).unwrap();
        assert!(c["confirm"] == false && !c["warnings"].as_array().unwrap().is_empty());
        s.run("classification.set", &json!({"level": "C3"})).unwrap();
        assert_eq!(s.run("classification.check", &json!({"action": "publish"})).unwrap()["confirm"], true);
        assert!(require_ack(&s, &json!({}), "publish").is_err());
        assert!(require_ack(&s, &json!({"acknowledged": true}), "publish").is_ok());
        assert_eq!(marking(&s.prefs.classification, &s.doc).as_deref(), Some("C3 — Restricted"));
        let copy = marked(&s.prefs.classification, &s.doc);
        assert!(copy.plain_text(wordcraft_doc::StoryRef::Body).starts_with("C3 — Restricted"));
        assert!(!s.doc.plain_text(wordcraft_doc::StoryRef::Body).contains("Restricted"), "the document itself is unchanged");
        assert_eq!(fm::get(&fm::stored(&for_template(&s.prefs.classification, &s.doc)), "footer-center").as_deref(), Some("C3 — Restricted"));
        s.prefs.classification.mark_exports = false;
        assert!(matches!(marked(&s.prefs.classification, &s.doc), std::borrow::Cow::Borrowed(_)));
    }

    /// Saved and opened again (Markdown, ZIP, TextPack): the same labels, the other front matter
    /// keys kept.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn labels_survive_saving_and_opening() {
        let dir = std::env::temp_dir().join(format!("plume-class-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for name in ["doc.md", "doc.zip", "doc.textpack"] {
            let path = dir.join(name);
            let mut s = session();
            s.native_markdown = true;
            fm::store(&mut s.doc, "title: \"Mon document\"\nunknown_key: kept");
            s.run(
                "classification.set",
                &json!({"level": "C2", "context": "personal", "visibility": "private", "importance": "normal", "personalData": "none"}),
            )
            .unwrap();
            s.run("file.save", &json!({"path": path.to_string_lossy()})).unwrap();
            let before = read(&s.doc);
            let mut t = Session::new(Document::new());
            t.native_markdown = true;
            t.run("file.open", &json!({"path": path.to_string_lossy()})).unwrap();
            assert_eq!(read(&t.doc), before, "{name}");
            assert_eq!(fm::get(&fm::stored(&t.doc), "unknown_key").as_deref(), Some("kept"), "{name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_referentials_are_kept_in_the_settings() {
        let mut s = session();
        let r = json!({"id": "acme", "version": "1", "name": "ACME", "levels": [
            {"code": "OPEN", "label": "Open", "color": "#00AA00", "publish": "allow"},
            {"code": "CLOSED", "label": "Closed", "color": "#AA0000", "publish": "confirm", "encryption": "required"}]});
        s.run("classification.saveReference", &json!({"reference": r})).unwrap();
        s.run("classification.options", &json!({"defaultReference": "acme"})).unwrap();
        s.run("classification.set", &json!({"level": "CLOSED"})).unwrap();
        assert_eq!(s.run("classification.check", &json!({"action": "export"})).unwrap()["confirm"], true);
        assert!(s.run("classification.saveReference", &json!({"reference": {"id": "plume", "levels": [{"code": "x"}]}})).is_err());
        assert!(s.run("classification.saveReference", &json!({"reference": {"id": "dup", "levels": [{"code": "A"}, {"code": "a"}]}})).is_err());
        s.run("classification.deleteReference", &json!({"id": "acme"})).unwrap();
        assert_eq!(s.prefs.classification.default_reference, PLUME);
        // The document keeps its level of a referential now unknown.
        assert_eq!(s.run("classification.get", &json!({})).unwrap()["level"], "CLOSED");
    }
}
