//! PDF through a document template (Plume): the document's Markdown laid out by a LaTeX template
//! in pandoc's template language — Eisvogel by default — and compiled by a TeX engine installed on
//! the machine (Tectonic, or XeLaTeX from TeX Live, MacTeX or TinyTeX). No pandoc: the body is
//! written by [`wordcraft_formats::latex::export_body`] and the template filled by
//! [`wordcraft_formats::template`].
//!
//! A template is a folder: `template.latex` (the template), `metadata.yaml` (its default
//! settings; the document's front matter wins) and optionally `name.txt` (its name in lists).
//! Templates live in `<settings>/templates/<id>/`; the built-in Eisvogel is written there the
//! first time it's needed, so it can be edited like any other (and restored).
//!
//! Pictures a template setting names (`titlepage-background`, `page-background`, `logo`,
//! `titlepage-logo`, `backcover`) are looked for beside the document, then in the template's
//! folder, and copied next to the generated `.tex`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};
use wordcraft_doc::Document;
use wordcraft_formats::latex;
use wordcraft_formats::template::{Template, Value, parse_yaml};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The default template's id.
pub const DEFAULT_TEMPLATE: &str = "eisvogel";
const EISVOGEL_LATEX: &str = include_str!("../templates/eisvogel/template.latex");
const EISVOGEL_METADATA: &str = include_str!("../templates/eisvogel/metadata.yaml");
/// File names inside a template folder.
pub const TEMPLATE_FILE: &str = "template.latex";
pub const METADATA_FILE: &str = "metadata.yaml";
const NAME_FILE: &str = "name.txt";
/// Largest picture copied for a template setting (bytes).
const MAX_PICTURE: u64 = 64 << 20;
/// Longest a TeX run may take (seconds). Tectonic's first run downloads its packages.
#[cfg(not(target_arch = "wasm32"))]
const TEX_TIMEOUT_SECS: u64 = 600;

/// Settings that name a picture or PDF file.
const PATH_KEYS: &[&str] = &["titlepage-background", "page-background", "titlepage-logo", "logo", "backcover"];
/// Settings that are text (escaped for LaTeX unless they already hold LaTeX commands).
const TEXT_KEYS: &[&str] = &[
    "title",
    "subtitle",
    "author",
    "date",
    "subject",
    "keywords",
    "abstract",
    "thanks",
    "toc-title",
    "header-left",
    "header-center",
    "header-right",
    "footer-left",
    "footer-center",
    "footer-right",
    "backcover-text",
    "watermark",
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("pdfTemplate.list", "PDF Templates", "File › Export", |_s, _v| Ok(json!({"templates": list_json()?})))
            .params("{} → templates: [{id, name, builtin, dir}]")
            .pure(),
        CommandSpec::new("pdfTemplate.read", "Read PDF Template", "File › Export", |_s, v| {
            let dir = template_dir(id_param(v)?)?;
            Ok(json!({
                "template": read_text(&dir.join(TEMPLATE_FILE))?,
                "metadata": read_text(&dir.join(METADATA_FILE)).unwrap_or_default(),
                "name": template_name(&dir),
                "dir": dir.to_string_lossy(),
            }))
        })
        .params("{id} → template (LaTeX), metadata (YAML), name, dir")
        .pure(),
        CommandSpec::new("pdfTemplate.save", "Save PDF Template", "File › Export", |_s, v| {
            let dir = template_dir(id_param(v)?)?;
            if let Some(t) = p::str(v, "template") {
                Template::parse(t).map_err(|e| CmdError::Params(format!("the template can't be read: {e}")))?;
                write_text(&dir.join(TEMPLATE_FILE), t)?;
            }
            if let Some(m) = p::str(v, "metadata") {
                write_text(&dir.join(METADATA_FILE), m)?;
            }
            if let Some(n) = p::str(v, "name").map(str::trim).filter(|n| !n.is_empty()) {
                write_text(&dir.join(NAME_FILE), n)?;
            }
            Ok(json!({"saved": true}))
        })
        .params("{id, template?, metadata?, name?}: write the template's files (the template is checked first)")
        .pure(),
        CommandSpec::new("pdfTemplate.duplicate", "New PDF Template", "File › Export", |_s, v| {
            let from = template_dir(id_param(v)?)?;
            let name = p::str(v, "name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or("My template").to_string();
            let root = templates_root().ok_or_else(no_settings)?;
            let base = slug(&name);
            let id = (1..1000)
                .map(|k| if k == 1 { base.clone() } else { format!("{base}-{k}") })
                .find(|id| !root.join(id).exists())
                .ok_or_else(|| CmdError::Failed("too many templates with this name".into()))?;
            let dir = root.join(&id);
            fs::copy_dir(&from, &dir).map_err(failed)?;
            write_text(&dir.join(NAME_FILE), &name)?;
            Ok(json!({"id": id, "name": name, "dir": dir.to_string_lossy()}))
        })
        .params("{id, name}: copy a template under a new name → {id, name, dir}")
        .pure(),
        CommandSpec::new("pdfTemplate.reset", "Restore PDF Template", "File › Export", |_s, v| {
            if id_param(v)? != DEFAULT_TEMPLATE {
                return Err(CmdError::Params("only the built-in template can be restored".into()));
            }
            let dir = templates_root().ok_or_else(no_settings)?.join(DEFAULT_TEMPLATE);
            write_builtin(&dir)?;
            Ok(json!({"restored": true}))
        })
        .params("{id: \"eisvogel\"}: put the built-in template back as shipped")
        .pure(),
        CommandSpec::new("pdfTemplate.engine", "PDF Engine", "File › Export", |_s, _v| {
            Ok(match find_engine() {
                Some(e) => json!({"found": true, "kind": e.kind.name(), "path": e.path.to_string_lossy()}),
                None => json!({"found": false, "install": INSTALL_HINT}),
            })
        })
        .params("{} → found, kind (tectonic|xelatex), path; or install: how to get one")
        .pure(),
        CommandSpec::new("file.exportPdfTemplate", "Export PDF with a Template", "File › Export", |s, v| PdfJob::prepare(s, v)?.run())
            .params("{path, template?: id (default eisvogel), keepTex?: bool}: lay the document out with a LaTeX template and compile it to `path`")
            .pure(),
        CommandSpec::new("file.exportLatexTemplate", "Export LaTeX with a Template", "File › Export", |s, v| {
            let out = PathBuf::from(p::req_str(v, "path")?);
            let id = p::str(v, "template").unwrap_or(DEFAULT_TEMPLATE);
            let dir = out.parent().map(Path::to_path_buf).unwrap_or_default();
            let tex = build(&s.doc, s.path.as_deref(), &template_dir(id)?, &dir)?;
            write_text(&out, &tex)?;
            Ok(json!({"path": out.to_string_lossy()}))
        })
        .params("{path, template?}: the filled template (.tex) and its pictures, without compiling")
        .pure(),
    ]
}

/// How to get a TeX engine, for messages.
pub const INSTALL_HINT: &str = "install Tectonic (brew install tectonic) or MacTeX/TinyTeX, then export again";

fn no_settings() -> CmdError {
    CmdError::Failed("Plume's settings folder isn't known on this system".into())
}

fn id_param(v: &Json) -> Result<&str, CmdError> {
    let id = p::str(v, "id").or_else(|| p::str(v, "template")).unwrap_or(DEFAULT_TEMPLATE).trim();
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(CmdError::Params(format!("{id:?} is not a template id")));
    }
    Ok(id)
}

fn slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let s: String = s.chars().take(40).collect();
    if s.is_empty() { "template".into() } else { s }
}

use crate::io::templates as fs;

fn failed(e: String) -> CmdError {
    CmdError::Failed(e)
}

fn read_text(path: &Path) -> Result<String, CmdError> {
    fs::read_text(path, wordcraft_formats::template::MAX_SOURCE as u64).map_err(failed)
}

fn write_text(path: &Path, text: &str) -> Result<(), CmdError> {
    fs::write_text(path, text).map_err(failed)
}

/// `<settings>/templates`.
pub fn templates_root() -> Option<PathBuf> {
    // Tests (the hostile-parameters sweep runs every command) never touch the real settings.
    if cfg!(test) {
        return Some(std::env::temp_dir().join(format!("plume-test-templates-{}", std::process::id())));
    }
    wordcraft_control_key::settings_dir().map(|d| d.join("templates"))
}

/// Eisvogel 3 loads Source Sans as `sourcesans`, its name since TeX Live 2024; older
/// distributions — Tectonic's bundle among them — call it `sourcesanspro`, and some have neither.
/// Templates written that way (copies made before Plume fixed its own, the user's variants) load
/// whichever exists, else keep the distribution's default fonts, instead of stopping the PDF.
fn portable_fonts(src: &str) -> String {
    let mut out = src.to_string();
    if !out.contains("\\IfFileExists{sourcesans.sty}") {
        out = out.replace(
            "\\usepackage[default]{sourcesans}",
            "\\IfFileExists{sourcesans.sty}{\\usepackage[default]{sourcesans}}{\\IfFileExists{sourcesanspro.sty}{\\usepackage[default]{sourcesanspro}}{}}",
        );
    }
    if !out.contains("\\IfFileExists{sourcecodepro.sty}") {
        out = out.replace("\\usepackage{sourcecodepro}", "\\IfFileExists{sourcecodepro.sty}{\\usepackage{sourcecodepro}}{}");
    }
    out
}

fn write_builtin(dir: &Path) -> Result<(), CmdError> {
    write_text(&dir.join(TEMPLATE_FILE), EISVOGEL_LATEX)?;
    write_text(&dir.join(METADATA_FILE), EISVOGEL_METADATA)?;
    write_text(&dir.join(NAME_FILE), "Eisvogel")
}

/// A template's folder (the built-in one is written on first use).
pub fn template_dir(id: &str) -> Result<PathBuf, CmdError> {
    let dir = templates_root().ok_or_else(no_settings)?.join(id);
    if id == DEFAULT_TEMPLATE && !dir.join(TEMPLATE_FILE).is_file() {
        write_builtin(&dir)?;
    }
    if !dir.join(TEMPLATE_FILE).is_file() {
        return Err(CmdError::Params(format!("there's no template {id:?}")));
    }
    Ok(dir)
}

fn template_name(dir: &Path) -> String {
    fs::read_text(&dir.join(NAME_FILE), 4096)
        .ok()
        .map(|s| s.lines().next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

fn list_json() -> Result<Vec<Json>, CmdError> {
    let root = templates_root().ok_or_else(no_settings)?;
    template_dir(DEFAULT_TEMPLATE)?;
    let mut out = Vec::new();
    for dir in fs::subfolders(&root) {
        if !dir.join(TEMPLATE_FILE).is_file() {
            continue;
        }
        let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        out.push(json!({"id": id, "name": template_name(&dir), "builtin": id == DEFAULT_TEMPLATE, "dir": dir.to_string_lossy()}));
    }
    // The default first.
    out.sort_by_key(|t| t["builtin"] != json!(true));
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Filling the template

/// LaTeX holds a command (`\thepage`) or a group: written as is.
fn is_latex(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(2).any(|w| w.first() == Some(&b'\\') && w.get(1).is_some_and(|c| c.is_ascii_alphabetic()))
}

fn escape_value(v: Value) -> Value {
    match v {
        Value::Str(s) if !is_latex(&s) => Value::Str(latex::escape_text(&s)),
        Value::List(l) => Value::List(l.into_iter().map(escape_value).collect()),
        other => other,
    }
}

/// Babel's name for a BCP 47 language tag.
fn babel_lang(tag: &str) -> Option<&'static str> {
    let t = tag.to_ascii_lowercase();
    let (lang, region) = t.split_once(['-', '_']).unwrap_or((t.as_str(), ""));
    Some(match (lang, region) {
        ("en", "gb" | "uk") => "british",
        ("en", "us") | ("en", "") => "american",
        ("en", _) => "english",
        ("fr", "ca") => "canadien",
        ("fr", _) => "french",
        ("de", "at") => "naustrian",
        ("de", "ch") => "nswissgerman",
        ("de", _) => "ngerman",
        ("es", _) => "spanish",
        ("it", _) => "italian",
        ("pt", "br") => "brazilian",
        ("pt", _) => "portuguese",
        ("nl", _) => "dutch",
        ("ca", _) => "catalan",
        ("pl", _) => "polish",
        ("cs", _) => "czech",
        ("sv", _) => "swedish",
        ("da", _) => "danish",
        ("nb" | "no", _) => "norsk",
        ("fi", _) => "finnish",
        ("ru", _) => "russian",
        ("uk", _) => "ukrainian",
        ("el", _) => "greek",
        ("tr", _) => "turkish",
        ("hu", _) => "magyar",
        ("ro", _) => "romanian",
        ("hr", _) => "croatian",
        _ => return None,
    })
}

/// The template's variables for `doc`: the template's defaults, the document's front matter and
/// properties over them, then what the body needs. Pictures the settings name are copied to
/// `out_dir/assets`.
fn variables(doc: &Document, doc_dir: Option<&Path>, tpl_dir: &Path, body: &latex::Body, out_dir: &Path) -> Value {
    let mut vars: BTreeMap<String, Value> = read_text(&tpl_dir.join(METADATA_FILE)).map(|t| parse_yaml(&t)).unwrap_or_default();
    let flow_meta =
        doc.passthrough.get(wordcraft_formats::model::FRONT_MATTER_PART).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    vars.extend(parse_yaml(&flow_meta));
    let c = &doc.core;
    let split = |s: &str| Value::List(s.split(", ").map(str::trim).filter(|x| !x.is_empty()).map(|x| Value::Str(x.to_string())).collect());
    for (k, v) in [("title", &c.title), ("subject", &c.subject), ("date", &c.created)] {
        if !v.trim().is_empty() {
            vars.insert(k.into(), Value::Str(v.trim().to_string()));
        }
    }
    if !c.creator.trim().is_empty() {
        vars.insert("author".into(), split(&c.creator));
    }
    if !c.keywords.trim().is_empty() {
        vars.insert("keywords".into(), split(&c.keywords));
    }
    let plain = |v: Option<&Value>| v.map(Value::text).unwrap_or_default();
    let title_meta = plain(vars.get("title"));
    let author_meta = plain(vars.get("author"));
    for k in TEXT_KEYS {
        if let Some(v) = vars.remove(*k) {
            vars.insert((*k).to_string(), escape_value(v));
        }
    }
    if !vars.get("title").is_some_and(Value::truthy) && !body.title.is_empty() {
        vars.insert("title".into(), Value::Str(body.title.clone()));
    }
    vars.insert("title-meta".into(), Value::Str(latex::escape_text(&title_meta)));
    vars.insert("author-meta".into(), Value::Str(latex::escape_text(&author_meta)));
    for (k, on) in
        [("tables", body.tables), ("graphics", body.graphics), ("strikeout", body.strikeout), ("multirow", body.multirow), ("listings", true)]
    {
        if on {
            vars.insert(k.into(), Value::Bool(true));
        }
    }
    if let Some(lang) = vars.get("lang").map(Value::text)
        && let Some(b) = babel_lang(&lang)
    {
        vars.entry("babel-lang".into()).or_insert(Value::Str(b.into()));
    }
    // Pictures named by settings.
    for (n, key) in PATH_KEYS.iter().enumerate() {
        let Some(name) = vars.get(*key).map(Value::text).filter(|s| !s.trim().is_empty()) else { continue };
        let found = [doc_dir, Some(tpl_dir)]
            .into_iter()
            .flatten()
            .map(|d| d.join(name.trim()))
            .chain(std::iter::once(PathBuf::from(name.trim())))
            .find(|p| p.is_file());
        let Some(src) = found else {
            log::warn!("template setting {key}: {name:?} not found");
            vars.remove(*key);
            continue;
        };
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or_else(|| "pdf".into());
        let rel = format!("assets/setting{n}.{ext}");
        match fs::copy_into_work(&src, &out_dir.join(&rel), MAX_PICTURE) {
            Ok(()) => {
                vars.insert((*key).to_string(), Value::Str(rel));
            }
            Err(e) => {
                log::warn!("template setting {key}: {e}");
                vars.remove(*key);
            }
        }
    }
    // Pandoc's own defaults, which templates count on.
    vars.entry("toc-depth".into()).or_insert(Value::Str("3".into()));
    vars.insert("body".into(), Value::Str(body.latex.clone()));
    Value::Map(vars)
}

/// Write the filled template's pictures into `out_dir` and return the LaTeX source.
pub fn build(doc: &Document, doc_path: Option<&Path>, tpl_dir: &Path, out_dir: &Path) -> Result<String, CmdError> {
    let src = portable_fonts(&read_text(&tpl_dir.join(TEMPLATE_FILE))?);
    let tpl = Template::parse(&src).map_err(|e| CmdError::Failed(format!("the template can't be read: {e}")))?;
    let body = latex::export_body(doc);
    for (name, bytes) in &body.images {
        fs::write_work_file(&out_dir.join(name), bytes).map_err(failed)?;
    }
    let vars = variables(doc, doc_path.and_then(Path::parent), tpl_dir, &body, out_dir);
    Ok(tpl.render(&vars))
}

// ---------------------------------------------------------------------------------------------
// TeX engines

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngineKind {
    Tectonic,
    XeLaTeX,
}

impl EngineKind {
    pub fn name(self) -> &'static str {
        match self {
            EngineKind::Tectonic => "tectonic",
            EngineKind::XeLaTeX => "xelatex",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Engine {
    pub kind: EngineKind,
    pub path: PathBuf,
}

/// A TeX engine on this machine: `PLUME_TEX` (a path to `tectonic` or `xelatex`), Plume's own
/// `<settings>/engine/bin`, then the places installers put them (apps opened from the Finder
/// don't get the shell's `PATH`), then `PATH`.
pub fn find_engine() -> Option<Engine> {
    let kind_of = |p: &Path| {
        let n = p.file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if n.starts_with("tectonic") { EngineKind::Tectonic } else { EngineKind::XeLaTeX }
    };
    if let Some(p) = std::env::var_os("PLUME_TEX").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(Engine { kind: kind_of(&p), path: p });
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(s) = wordcraft_control_key::settings_dir() {
        dirs.push(s.join("engine/bin"));
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/Library/TeX/texbin", "/usr/bin"].map(PathBuf::from));
    if let Some(h) = &home {
        for d in ["Library/TinyTeX/bin/universal-darwin", ".TinyTeX/bin/x86_64-linux", ".TinyTeX/bin/aarch64-linux", ".cargo/bin", "bin"] {
            dirs.push(h.join(d));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    let exe = if cfg!(windows) { ".exe" } else { "" };
    for name in ["tectonic", "xelatex"] {
        for d in &dirs {
            let p = d.join(format!("{name}{exe}"));
            if p.is_file() {
                return Some(Engine { kind: kind_of(&p), path: p });
            }
        }
    }
    None
}

/// A template PDF export: prepared from the session (fast: the `.tex` and its pictures are
/// written to a work folder), then [`PdfJob::run`] compiles it, which can take a while (the UI
/// runs it on a worker thread).
pub struct PdfJob {
    work: PathBuf,
    out: PathBuf,
    engine: Option<Engine>,
    keep_tex: bool,
}

impl PdfJob {
    pub fn prepare(s: &Session, v: &Json) -> Result<PdfJob, CmdError> {
        let out = PathBuf::from(p::req_str(v, "path")?);
        let id = id_param(v)?;
        let tpl = template_dir(id)?;
        let work = std::env::temp_dir().join(format!("plume-pdf-{}-{}", std::process::id(), s.document_id()));
        fs::fresh_work_dir(&work).map_err(failed)?;
        let tex = build(&s.doc, s.path.as_deref(), &tpl, &work)?;
        fs::write_work_file(&work.join("document.tex"), tex.as_bytes()).map_err(failed)?;
        Ok(PdfJob { work, out, engine: find_engine(), keep_tex: p::bool(v, "keepTex").unwrap_or(false) })
    }

    /// Compile and copy the PDF to its place.
    pub fn run(self) -> CmdResult {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (&self.engine, self.keep_tex, &self.out, &self.work);
            Err(CmdError::Disabled("PDF templates need a TeX engine, which the web version can't run".into()))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let r = self.compile();
            if r.is_ok() && !self.keep_tex {
                fs::remove_work_dir(&self.work);
            }
            r
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(&self) -> CmdResult {
        let Some(engine) = &self.engine else {
            return Err(CmdError::Failed(format!("no TeX engine was found: {INSTALL_HINT}")));
        };
        let runs: &[&[&str]] = match engine.kind {
            EngineKind::Tectonic => &[&["--keep-logs", "document.tex"]],
            // Twice: the table of contents and page references need a second pass.
            EngineKind::XeLaTeX => {
                &[&["-interaction=nonstopmode", "-halt-on-error", "document.tex"], &["-interaction=nonstopmode", "-halt-on-error", "document.tex"]]
            }
        };
        for args in runs {
            let status = run_with_timeout(&engine.path, args, &self.work)?;
            if !status {
                return Err(CmdError::Failed(format!("{} stopped: {}", engine.kind.name(), tex_error(&self.work))));
            }
        }
        let pdf = self.work.join("document.pdf");
        if !pdf.is_file() {
            return Err(CmdError::Failed(format!("{} made no PDF: {}", engine.kind.name(), tex_error(&self.work))));
        }
        fs::copy_out_of_work(&pdf, &self.out).map_err(failed)?;
        Ok(
            json!({"path": self.out.to_string_lossy(), "engine": engine.kind.name(), "work": if self.keep_tex { Some(self.work.to_string_lossy()) } else { None }}),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_with_timeout(exe: &Path, args: &[&str], dir: &Path) -> Result<bool, CmdError> {
    fs::run_in(exe, args, dir, TEX_TIMEOUT_SECS).map_err(failed)
}

/// The first error a TeX run reported (`! …` and the line after), else the end of its output.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn tex_error(dir: &Path) -> String {
    let text = ["document.log", "engine-output.txt"].iter().filter_map(|f| fs::read_work_text(&dir.join(f))).collect::<Vec<_>>().join("\n");
    let lines: Vec<&str> = text.lines().collect();
    if let Some(k) = lines.iter().position(|l| l.starts_with("! ") || l.starts_with("error:")) {
        let mut msg: Vec<&str> = lines.iter().skip(k).take(3).map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        msg.truncate(2);
        return msg.join(" ").chars().take(400).collect();
    }
    lines.iter().rev().take(3).rev().copied().collect::<Vec<_>>().join(" ").chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tectonic's bundle has `sourcesanspro`, not `sourcesans`: a template asking for the latter
    /// loads whichever exists (and the built-in one already does), once.
    #[test]
    fn source_fonts_load_under_either_name() {
        let old = "\\usepackage[default]{sourcesans}\n\\usepackage{sourcecodepro}\n";
        let fixed = portable_fonts(old);
        assert!(fixed.contains("\\IfFileExists{sourcesanspro.sty}{\\usepackage[default]{sourcesanspro}}{}"), "{fixed}");
        assert!(fixed.contains("\\IfFileExists{sourcecodepro.sty}{\\usepackage{sourcecodepro}}{}"), "{fixed}");
        assert_eq!(portable_fonts(&fixed), fixed, "applied once");
        assert_eq!(portable_fonts(EISVOGEL_LATEX), EISVOGEL_LATEX, "the built-in template already falls back");
    }

    #[test]
    fn eisvogel_fills_with_a_document() {
        let dir = std::env::temp_dir().join(format!("plume-tpl-test-{}", std::process::id()));
        let tpl = dir.join("tpl");
        write_builtin(&tpl).unwrap();
        let doc = wordcraft_formats::markdown::import(
            "---\ntitle: \"Rapport & co\"\nauthor: [Ada]\ntoc: true\nheader-left: \"\\\\thepage\"\n---\n\n# Introduction\n\nDu **gras**, une note.[^1]\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```\n\n[^1]: La note.\n",
        );
        let tex = build(&doc, None, &tpl, &dir.join("out")).unwrap();
        assert!(tex.contains("\\section{Introduction}\\label{introduction}"), "{tex}");
        assert!(tex.contains("\\title{Rapport \\& co}"));
        assert!(tex.contains("\\begin{longtable}"));
        assert!(tex.contains("\\begin{lstlisting}"));
        assert!(tex.contains("\\footnote{La note.}"));
        assert!(tex.contains("\\tableofcontents"));
        assert!(tex.contains("  french,") && tex.contains("{babel}"), "babel from lang");
        assert!(tex.contains("\\thepage"), "raw LaTeX settings stay");
        assert!(!tex.contains("$title$") && !tex.contains("$endif$"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn template_ids_are_checked() {
        for bad in ["../x", "a/b", "", "é"] {
            assert!(id_param(&json!({"id": bad})).is_err(), "{bad}");
        }
        assert_eq!(slug("Mon modèle à moi!"), "mon-mod-le-moi");
        assert!(babel_lang("fr-FR") == Some("french") && babel_lang("xx").is_none());
    }
}
