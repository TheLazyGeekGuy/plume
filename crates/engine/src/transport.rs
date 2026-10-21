//! Plume: moving a Markdown document between its transport forms, and protecting it.
//!
//! A document travels as a **package**: its Markdown file and the files beside it (pictures,
//! attachments, anything else the archive held), by path. Every form holds one:
//!
//! - **Markdown + assets**: `report.md` in a folder, with the local files it links to;
//! - **ZIP**: the same in one archive, the Markdown at the root or in a folder ([`crate::io::mdzip`]);
//! - **TextPack** (`.textpack`) and **TextBundle** (`.textbundle`, a folder): `text.md`,
//!   `info.json` and the files under `assets/` ([`crate::io::textbundle`]).
//!
//! Converting reads the source into a package and writes it in the target form, bytes for bytes:
//! the Markdown is not rewritten by the editor, and files nothing links to are kept. TextBundle
//! wants files under `assets/`: those elsewhere are moved there, the links to them updated, and
//! the original paths recorded in `info.json` (`net.plume.transport`) so converting back restores
//! the package exactly. Remote links (`https://…`) are listed, never downloaded.
//!
//! A package can then be **protected**: a whole file in AES Crypt (`.aes`, any form inside), or
//! a ZIP whose entries are encrypted with AES-256 ([`wordcraft_crypt`]). The result is read back
//! and compared with the package (decrypted with the same password) before it is written; a
//! file at the destination is never overwritten without `overwrite`.
//!
//! Passwords come in commands' parameters (or an environment variable they name), are never
//! written into the document, its front matter, a log or a message, and aren't kept.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use wordcraft_crypt as crypt;
use zeroize::Zeroizing;

use crate::io::{mdzip, textbundle};
use crate::{CmdError, CmdResult, CommandSpec, p};

/// The key of `info.json` where a TextBundle Plume wrote records the package's original paths.
pub const INFO_KEY: &str = "net.plume.transport";

/// A document and its files, by path from the package's root.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Package {
    /// The Markdown file's path (`report.md`, `docs/guide.md`).
    pub text_path: String,
    pub text: Vec<u8>,
    /// Every other file, by path, in the order they were read.
    pub files: Vec<(String, Vec<u8>)>,
    /// A TextBundle's `info.json` (other applications' keys are kept through conversions).
    pub info: Option<Map<String, Value>>,
}

/// The forms a package travels in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Markdown,
    Zip,
    TextPack,
    TextBundle,
}

impl Form {
    pub fn name(self) -> &'static str {
        match self {
            Form::Markdown => "markdown",
            Form::Zip => "zip",
            Form::TextPack => "textpack",
            Form::TextBundle => "textbundle",
        }
    }
}

/// How a written package is protected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection {
    None,
    /// The whole file in AES Crypt (`.aes`).
    AesCrypt,
    /// A ZIP whose entries are encrypted with WinZip AES-256.
    ZipAes,
}

impl Protection {
    pub fn name(self) -> &'static str {
        match self {
            Protection::None => "none",
            Protection::AesCrypt => "aescrypt",
            Protection::ZipAes => "zip-aes",
        }
    }
}

/// A file name's form and protection: `x.textpack.aes` is a TextPack in AES Crypt, `x.zip` a
/// ZIP (encrypted when a password is given), `x.md` Markdown + assets.
pub fn form_of(name: &str, password: bool) -> Option<(Form, Protection)> {
    let lower = name.trim_end_matches(['/', '\\']).to_ascii_lowercase();
    let (inner, aes) = match lower.strip_suffix(".aes") {
        Some(inner) => (inner.to_string(), true),
        None => (lower, false),
    };
    let ext = inner.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let form = match ext {
        "md" | "markdown" => Form::Markdown,
        "zip" => Form::Zip,
        "textpack" => Form::TextPack,
        "textbundle" => Form::TextBundle,
        _ => return None,
    };
    let protection = if aes {
        Protection::AesCrypt
    } else if password && form == Form::Zip {
        Protection::ZipAes
    } else {
        Protection::None
    };
    Some((form, protection))
}

/// What a package's Markdown links to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Links {
    /// Local files it links to that the package holds.
    pub found: Vec<String>,
    /// Local files it links to that the package lacks.
    pub missing: Vec<String>,
    /// Links climbing out of the package's folder (`../logo.png`): not carried.
    pub outside: Vec<String>,
    /// Remote resources (`https://…`): listed, never downloaded.
    pub remote: Vec<String>,
    /// Files of the package nothing links to (kept all the same).
    pub unused: Vec<String>,
}

/// The link targets written in Markdown `text`: inline links and images, reference
/// definitions, and HTML `src`/`href` attributes. Duplicates are kept once, in order.
pub fn link_targets(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |t: &str| {
        let t = t.trim().trim_start_matches('<').trim_end_matches('>');
        if !t.is_empty() && !t.starts_with('#') && !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    };
    // `](target "title")` and `](<target with spaces>)`.
    let mut rest = text;
    while let Some(i) = rest.find("](") {
        rest = rest.get(i + 2..).unwrap_or("");
        let end = if rest.starts_with('<') { rest.find('>').map(|e| e + 1) } else { rest.find([')', ' ', '\n', '\t']) };
        if let Some(e) = end {
            push(rest.get(..e).unwrap_or(""));
        }
    }
    for line in text.lines() {
        // `[label]: target "title"`
        let l = line.trim_start();
        if l.starts_with('[')
            && let Some((label, def)) = l.split_once("]:")
            && !label.contains("](")
        {
            push(def.split_whitespace().next().unwrap_or(""));
        }
        // `src="…"`, `href='…'`
        for attr in ["src=", "href="] {
            let mut rest = line;
            while let Some(i) = rest.find(attr) {
                rest = rest.get(i + attr.len()..).unwrap_or("");
                let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
                let body = rest.get(1..).unwrap_or("");
                if let Some(e) = body.find(q) {
                    push(body.get(..e).unwrap_or(""));
                }
            }
        }
    }
    out
}

fn is_remote(target: &str) -> bool {
    let l = target.to_ascii_lowercase();
    l.contains("://") || l.starts_with("mailto:") || l.starts_with("data:") || l.starts_with("tel:")
}

/// The folder of a path, with its slash (`docs/` for `docs/a.md`, `` at the root).
fn folder(path: &str) -> &str {
    path.rfind('/').and_then(|i| path.get(..=i)).unwrap_or("")
}

/// A link target as a path in the package: relative to the Markdown's folder, query and fragment
/// dropped, percent-decoded when the written form names no file. `Err` when it climbs out.
fn resolve(text_path: &str, target: &str, has: impl Fn(&str) -> bool) -> Result<String, ()> {
    let target = target.split(['#', '?']).next().unwrap_or("");
    let dir = folder(text_path);
    let raw = textbundle::clean(&format!("{dir}{target}"));
    let decoded = String::from_utf8(wordcraft_formats::model::percent_decode(target)).ok().and_then(|t| textbundle::clean(&format!("{dir}{t}")));
    match (raw, decoded) {
        (Some(r), _) if has(&r) => Ok(r),
        (_, Some(d)) if has(&d) => Ok(d),
        (Some(r), _) => Ok(r),
        (None, Some(d)) => Ok(d),
        (None, None) => Err(()),
    }
}

impl Package {
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.iter().find(|(p, _)| p == path).map(|(_, b)| b.as_slice())
    }

    /// What the Markdown links to, and which files nothing links to.
    pub fn links(&self) -> Links {
        let text = String::from_utf8_lossy(&self.text);
        let mut l = Links::default();
        for t in link_targets(&text) {
            if is_remote(&t) {
                l.remote.push(t);
                continue;
            }
            match resolve(&self.text_path, &t, |p| self.file(p).is_some()) {
                Ok(p) if self.file(&p).is_some() => {
                    if !l.found.contains(&p) {
                        l.found.push(p);
                    }
                }
                Ok(_) => l.missing.push(t),
                Err(()) => l.outside.push(t),
            }
        }
        l.unused = self.files.iter().map(|(p, _)| p.clone()).filter(|p| !l.found.contains(p)).collect();
        l
    }

    /// Total size, bytes.
    pub fn size(&self) -> usize {
        self.text.len() + self.files.iter().map(|(_, b)| b.len()).sum::<usize>()
    }
}

// ---------------------------------------------------------------------------------------------
// Reading

/// The package in archive bytes: a ZIP or a TextPack (`form`), decrypted first when needed.
pub fn from_bytes(form: Form, bytes: &[u8], password: Option<&str>) -> Result<Package, String> {
    let plain;
    let bytes = match crypt::detect(bytes) {
        crypt::Kind::AesCrypt(_) => {
            let pw = password.ok_or_else(|| crypt::Error::PasswordRequired.to_string())?;
            plain = crypt::aescrypt::decrypt(bytes, pw).map_err(|e| e.to_string())?.to_vec();
            plain.as_slice()
        }
        crypt::Kind::ZipAes | crypt::Kind::ZipLegacy => {
            let pw = password.ok_or_else(|| crypt::Error::PasswordRequired.to_string())?;
            plain = crypt::zipaes::decrypt(bytes, pw).map_err(|e| e.to_string())?;
            plain.as_slice()
        }
        crypt::Kind::Age | crypt::Kind::SevenZip => {
            return Err(format!("{} files can't be opened yet; decrypt them with their own tool first", crypt::detect(bytes).name()));
        }
        _ => bytes,
    };
    match form {
        Form::Zip => {
            let u = mdzip::unpack(bytes)?;
            Ok(Package { text_path: u.text_path, text: u.text, files: u.files, info: None })
        }
        Form::TextPack => {
            let u = textbundle::unpack(bytes)?;
            Ok(from_bundle(u.info, u.text_name, u.text, u.files))
        }
        Form::Markdown | Form::TextBundle => Err(format!("a {} is read from a folder, not from bytes", form.name())),
    }
}

/// A TextBundle's contents as a package: the paths recorded in `info.json` restored (see
/// [`INFO_KEY`]), the links to them too.
fn from_bundle(mut info: Map<String, Value>, text_name: String, text: Vec<u8>, files: Vec<(String, Vec<u8>)>) -> Package {
    let record = info.remove(INFO_KEY);
    let original_text = record.as_ref().and_then(|r| r.get("text")).and_then(Value::as_str).and_then(textbundle::clean);
    let moved: BTreeMap<String, String> = record
        .as_ref()
        .and_then(|r| r.get("moved"))
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), textbundle::clean(v.as_str()?)?))).collect())
        .unwrap_or_default();
    let text_path = original_text.unwrap_or(text_name.clone());
    // Links are rewritten only if the text is still the one recorded (no other editor changed
    // its links since): otherwise moved files stay under `assets/` for its links to keep working.
    let restorable = !moved.is_empty()
        && std::str::from_utf8(&text).is_ok_and(|t| {
            let back = relink(t, &links_map(&text_name, &text_path, &moved));
            relink(&back, &links_map(&text_path, &text_name, &invert(&moved))) == t
        });
    let text = match std::str::from_utf8(&text) {
        Ok(t) if restorable => relink(t, &links_map(&text_name, &text_path, &moved)).into_bytes(),
        _ => text,
    };
    let files = files.into_iter().map(|(p, b)| (if restorable { moved.get(&p).cloned().unwrap_or(p) } else { p }, b)).collect();
    let info = (!info.is_empty()).then_some(info);
    Package { text_path: if restorable || moved.is_empty() { text_path } else { text_name }, text, files, info }
}

fn invert(m: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    m.iter().map(|(k, v)| (v.clone(), k.clone())).collect()
}

/// The link rewrites for files moved by `moved` (old path → new path), as written from each
/// text's folder: `(from link, to link)`.
fn links_map(from_text: &str, to_text: &str, moved: &BTreeMap<String, String>) -> Vec<(String, String)> {
    moved.iter().map(|(from, to)| (relative(folder(from_text), from), relative(folder(to_text), to))).collect()
}

/// `path` as written from folder `dir` (both from the package root): `images/a.png` from ``,
/// `../images/a.png` from `docs/`.
fn relative(dir: &str, path: &str) -> String {
    let d: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let p: Vec<&str> = path.split('/').collect();
    let common = d.iter().zip(&p).take_while(|(a, b)| a == b).count();
    let mut out: Vec<&str> = std::iter::repeat_n("..", d.len().saturating_sub(common)).collect();
    out.extend(p.iter().skip(common));
    out.join("/")
}

/// `text` with each link target `from` replaced by `to`, written plain, as `./from`, or
/// percent-encoded (spaces as `%20`), where a link's target stands: after `(`, `<`, a quote,
/// or a reference definition's `]: `, and before `)`, `>`, a quote, a space, `#`, `?` or the
/// line's end.
pub fn relink(text: &str, map: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (from, to) in map {
        if from == to {
            continue;
        }
        for (f, t) in [(from.clone(), to.clone()), (from.replace(' ', "%20"), to.replace(' ', "%20"))] {
            out = replace_targets(&out, &f, &t);
        }
    }
    out
}

fn replace_targets(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in text.match_indices(from) {
        if i < last {
            continue;
        }
        let before = text.get(..i).unwrap_or("");
        let before = before.strip_suffix("./").unwrap_or(before);
        let opens = before.ends_with(['(', '<', '"', '\'']) || before.ends_with("]: ") || before.ends_with("]:");
        let after = text.get(i + from.len()..).unwrap_or("");
        let closes = after.is_empty() || after.starts_with([')', '>', '"', '\'', ' ', '\t', '\n', '\r', '#', '?']);
        if opens && closes {
            out.push_str(text.get(last..i).unwrap_or(""));
            out.push_str(to);
            last = i + from.len();
        }
    }
    out.push_str(text.get(last..).unwrap_or(""));
    out
}

/// Read a package from disk: a Markdown file and the local files it links to (within its
/// folder), a ZIP, a TextPack or a TextBundle folder, decrypted with `password` if needed.
#[cfg(not(target_arch = "wasm32"))]
pub fn read(path: &std::path::Path, password: Option<&str>) -> Result<Package, String> {
    let name = path.to_string_lossy().to_string();
    let (form, _) = form_of(&name, false).ok_or_else(|| format!("{name}: not a Markdown document, ZIP, TextPack or TextBundle"))?;
    match form {
        Form::Zip | Form::TextPack => {
            let bytes = crate::io::read_file_capped(&name, crypt::MAX_SIZE as u64, "archive")?;
            from_bytes(form, &bytes, password).map_err(|e| format!("{name}: {e}"))
        }
        Form::Markdown if name.to_ascii_lowercase().ends_with(".aes") => {
            let bytes = crate::io::read_file_capped(&name, crypt::MAX_SIZE as u64, "file")?;
            let pw = password.ok_or_else(|| crypt::Error::PasswordRequired.to_string())?;
            let text = crypt::aescrypt::decrypt(&bytes, pw).map_err(|e| format!("{name}: {e}"))?.to_vec();
            let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            Ok(Package { text_path: file.strip_suffix(".aes").unwrap_or(&file).to_string(), text, files: Vec::new(), info: None })
        }
        Form::Markdown => read_markdown(path),
        Form::TextBundle => read_textbundle(path),
    }
}

/// A Markdown file and the files it links to, inside its folder.
#[cfg(not(target_arch = "wasm32"))]
fn read_markdown(path: &std::path::Path) -> Result<Package, String> {
    let name = path.to_string_lossy().to_string();
    let text = crate::io::read_file_capped(&name, textbundle::MAX_TEXT, "Markdown file")?;
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).ok_or_else(|| format!("{name}: not a file"))?;
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    let mut pkg = Package { text_path: file, text, files: Vec::new(), info: None };
    let mut total = 0u64;
    for t in link_targets(&String::from_utf8_lossy(&pkg.text)) {
        if is_remote(&t) {
            continue;
        }
        let Ok(rel) = resolve(&pkg.text_path, &t, |p| dir.join(p).is_file()) else { continue };
        if pkg.file(&rel).is_some() || rel == pkg.text_path {
            continue;
        }
        let full = dir.join(&rel);
        if !full.is_file() {
            continue;
        }
        let bytes = crate::io::read_file_capped(&full.to_string_lossy(), textbundle::MAX_ENTRY, "file")?;
        total += bytes.len() as u64;
        if total > textbundle::MAX_TOTAL {
            return Err(format!("{name}: its files are larger than {} MB together", textbundle::MAX_TOTAL >> 20));
        }
        pkg.files.push((rel, bytes));
    }
    Ok(pkg)
}

/// A TextBundle folder: `info.json`, the text and every other file in it.
#[cfg(not(target_arch = "wasm32"))]
fn read_textbundle(dir: &std::path::Path) -> Result<Package, String> {
    let name = dir.to_string_lossy().to_string();
    let mut files = crate::io::folders::files_under(dir)?;
    let text_name =
        textbundle::pick_text(files.iter().map(|(p, _)| p.as_str())).map(str::to_string).ok_or_else(|| format!("{name}: no text file (text.md)"))?;
    let info = files.iter().find(|(p, _)| p == "info.json").map(|(_, b)| textbundle::parse_info(b)).unwrap_or_default();
    let text = files.iter().find(|(p, _)| *p == text_name).map(|(_, b)| b.clone()).unwrap_or_default();
    files.retain(|(p, _)| *p != text_name && p != "info.json");
    Ok(from_bundle(info, text_name, text, files))
}

// ---------------------------------------------------------------------------------------------
// Writing

/// The package as a TextBundle's contents: `(info, text name, text, files)`. Files outside
/// `assets/` move into it (`images/a.png` → `assets/images/a.png`) with the links to them, if
/// the move can be undone exactly; the original paths are recorded under [`INFO_KEY`].
fn to_bundle(pkg: &Package) -> (Map<String, Value>, String, Vec<u8>, Vec<(String, Vec<u8>)>) {
    let mut info = textbundle::info_for_save(pkg.info.as_ref());
    let ext = pkg.text_path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).filter(|e| e == "markdown").unwrap_or_else(|| "md".into());
    let text_name = format!("text.{ext}");
    // Bundle path → package path, for the files outside `assets/` (none may land on another).
    let moved: BTreeMap<String, String> =
        pkg.files.iter().filter(|(p, _)| !p.starts_with("assets/")).map(|(p, _)| (format!("assets/{p}"), p.clone())).collect();
    let clash = moved.keys().any(|n| pkg.files.iter().any(|(q, _)| q == n));
    let relinked = match std::str::from_utf8(&pkg.text) {
        Ok(t) if !clash => {
            let n = relink(t, &links_map(&pkg.text_path, &text_name, &invert(&moved)));
            (relink(&n, &links_map(&text_name, &pkg.text_path, &moved)) == t).then_some(n)
        }
        _ => None,
    };
    let (text, files, moved) = match relinked {
        Some(n) => (
            n.into_bytes(),
            pkg.files.iter().map(|(p, b)| (if p.starts_with("assets/") { p.clone() } else { format!("assets/{p}") }, b.clone())).collect(),
            moved,
        ),
        // Not undoable exactly (or not UTF-8): files stay where the links find them.
        None => (pkg.text.clone(), pkg.files.clone(), BTreeMap::new()),
    };
    if moved.is_empty() && pkg.text_path == text_name {
        info.remove(INFO_KEY);
    } else {
        info.insert(INFO_KEY.into(), json!({"text": pkg.text_path, "moved": moved}));
    }
    (info, text_name, text, files)
}

/// The package as a ZIP: the Markdown and its files at their paths.
fn zip_bytes(pkg: &Package) -> Result<Vec<u8>, String> {
    let files: Vec<(String, &[u8])> = pkg.files.iter().map(|(p, b)| (relative(folder(&pkg.text_path), p), b.as_slice())).collect();
    mdzip::pack(None, &pkg.text_path, &pkg.text, &files)
}

/// The package written as one file in `form` (a ZIP or a TextPack), protected as asked. The
/// result is read back, decrypted, and compared with the package before it is returned.
pub fn to_bytes(pkg: &Package, form: Form, protection: Protection, password: Option<&str>) -> Result<Vec<u8>, String> {
    let plain = match form {
        Form::Zip => zip_bytes(pkg)?,
        Form::TextPack => {
            let (info, name, text, files) = to_bundle(pkg);
            let files: Vec<(String, &[u8])> = files.iter().map(|(p, b)| (p.clone(), b.as_slice())).collect();
            textbundle::pack(&info, &name, &text, &files)?
        }
        Form::Markdown if protection == Protection::AesCrypt => {
            if !pkg.files.is_empty() {
                return Err(
                    "a Markdown file with pictures or attachments is protected inside a ZIP or a TextPack (.zip.aes, .textpack.aes), not alone"
                        .into(),
                );
            }
            pkg.text.clone()
        }
        Form::Markdown | Form::TextBundle => return Err(format!("a {} is a folder: write it with `write`", form.name())),
    };
    let pw = || password.filter(|p| !p.is_empty()).ok_or_else(|| "a password is required to protect the file".to_string());
    let out = match protection {
        Protection::None => plain,
        Protection::AesCrypt => crypt::aescrypt::encrypt(&plain, pw()?).map_err(|e| e.to_string())?,
        Protection::ZipAes if form == Form::Zip => crypt::zipaes::encrypt(&plain, pw()?).map_err(|e| e.to_string())?,
        Protection::ZipAes => return Err("ZIP AES-256 protects a .zip; protect a TextPack with AES Crypt (.textpack.aes)".into()),
    };
    // Verify: what was written gives the package back.
    let back = if form == Form::Markdown {
        let text = crypt::aescrypt::decrypt(&out, password.unwrap_or("")).map_err(|e| format!("verification failed: {e}"))?;
        Package { text: text.to_vec(), ..pkg.clone() }
    } else {
        from_bytes(form, &out, password).map_err(|e| format!("verification failed: {e}"))?
    };
    if !same_package(&back, pkg) {
        return Err("verification failed: the written file doesn't give the document back as it was".into());
    }
    Ok(out)
}

/// Do two packages hold the same text and files (paths and bytes; order and `info` aside)?
pub fn same_package(a: &Package, b: &Package) -> bool {
    fn sorted(p: &Package) -> Vec<(&str, &[u8])> {
        let mut f: Vec<(&str, &[u8])> = p.files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        f.sort();
        f
    }
    a.text_path == b.text_path && a.text == b.text && sorted(a) == sorted(b)
}

/// Write the package as Markdown + assets into `dir` (the Markdown as `text_name` when given),
/// or as a TextBundle folder at `dir`. Every path is checked, and existing files listed, before
/// anything is written: without `overwrite`, an existing file stops the write.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_folder(
    pkg: &Package,
    form: Form,
    dir: &std::path::Path,
    text_name: Option<&str>,
    overwrite: bool,
) -> Result<Vec<std::path::PathBuf>, String> {
    let entries: Vec<(String, Vec<u8>)> = match form {
        Form::Markdown => {
            let name = text_name.and_then(textbundle::clean).unwrap_or_else(|| pkg.text_path.clone());
            std::iter::once((name, pkg.text.clone())).chain(pkg.files.iter().cloned()).collect()
        }
        Form::TextBundle => {
            let (info, name, text, files) = to_bundle(pkg);
            [("info.json".to_string(), textbundle::info_bytes(&info)), (name, text)].into_iter().chain(files).collect()
        }
        _ => return Err(format!("a {} is one file: write it with `to_bytes`", form.name())),
    };
    // Validate everything first.
    let mut targets = Vec::with_capacity(entries.len());
    let mut existing = Vec::new();
    for (rel, _) in &entries {
        let clean = textbundle::clean(rel).filter(|c| c == rel).ok_or_else(|| format!("{rel}: not a path inside the destination"))?;
        let path = dir.join(&clean);
        crate::file_access::check_write(&path.to_string_lossy())?;
        if crate::io::folders::exists(&path) {
            existing.push(clean.clone());
        }
        targets.push(path);
    }
    if !existing.is_empty() && !overwrite {
        return Err(format!("these files exist already (nothing was written): {}", existing.join(", ")));
    }
    let writes: Vec<(std::path::PathBuf, &[u8])> = targets.iter().cloned().zip(entries.iter().map(|(_, b)| b.as_slice())).collect();
    crate::io::folders::write_all(&writes)?;
    Ok(targets)
}

// ---------------------------------------------------------------------------------------------
// Commands

/// The password of a command: `password`, or the environment variable `passwordEnv` names (so
/// it needn't appear on a command line). Never echoed.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn password(v: &Value, key: &str, env_key: &str) -> Result<Option<Zeroizing<String>>, CmdError> {
    if let Some(pw) = p::str(v, key) {
        return Ok(Some(Zeroizing::new(pw.to_string())));
    }
    match p::str(v, env_key) {
        Some(var) => std::env::var(var)
            .map(|s| Some(Zeroizing::new(s)))
            .map_err(|_| CmdError::Params(format!("`{env_key}`: the environment variable {var} isn't set"))),
        None => Ok(None),
    }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn links_json(l: &Links) -> Value {
    json!({"found": l.found, "missing": l.missing, "outside": l.outside, "remote": l.remote, "unused": l.unused})
}

/// What protecting with each method leaves visible, for the user to be told.
pub fn exposure(protection: Protection, form: Form) -> Vec<&'static str> {
    match (protection, form) {
        (Protection::None, _) => vec!["Everything: the file isn't encrypted."],
        (Protection::ZipAes, _) => vec!["File names, sizes and dates inside the ZIP stay readable; only their contents are encrypted."],
        (Protection::AesCrypt, _) => vec!["The file's name and its approximate size stay visible; nothing inside does."],
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn convert(_s: &mut crate::Session, v: &Value) -> CmdResult {
    let from = p::req_str(v, "from")?;
    let to = p::req_str(v, "to")?;
    let source_pw = password(v, "sourcePassword", "sourcePasswordEnv")?;
    let pw = password(v, "password", "passwordEnv")?;
    let overwrite = p::bool(v, "overwrite").unwrap_or(false);
    let (form, protection) = form_of(to, pw.is_some())
        .ok_or_else(|| CmdError::Params("`to`: a .md, .zip, .textpack or .textbundle path, with .aes to protect it".into()))?;
    if protection != Protection::None && pw.is_none() {
        return Err(CmdError::Params("`password` (or `passwordEnv`) is required to protect the file".into()));
    }
    let pkg = read(std::path::Path::new(from), source_pw.as_deref().map(String::as_str)).map_err(CmdError::Failed)?;
    let to_path = std::path::Path::new(to);
    let written: Vec<String> = match form {
        Form::Markdown if protection == Protection::None => {
            let dir = to_path.parent().unwrap_or(std::path::Path::new("."));
            let name = to_path.file_name().map(|f| f.to_string_lossy().to_string());
            write_folder(&pkg, form, dir, name.as_deref(), overwrite)
                .map_err(CmdError::Failed)?
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect()
        }
        Form::TextBundle => {
            if protection != Protection::None {
                return Err(CmdError::Params("a TextBundle is a folder; protect a .textpack.aes instead".into()));
            }
            write_folder(&pkg, form, to_path, None, overwrite).map_err(CmdError::Failed)?.iter().map(|p| p.to_string_lossy().to_string()).collect()
        }
        _ => {
            let bytes = to_bytes(&pkg, form, protection, pw.as_deref().map(String::as_str)).map_err(CmdError::Failed)?;
            if !overwrite && crate::io::folders::exists(to_path) {
                return Err(CmdError::Failed(format!("{to} exists already (nothing was written); pass \"overwrite\": true to replace it")));
            }
            crate::io::write_file(to, &bytes).map_err(CmdError::Failed)?;
            vec![to.to_string()]
        }
    };
    Ok(json!({
        "form": form.name(),
        "protection": protection.name(),
        "written": written,
        "files": pkg.files.len(),
        "links": links_json(&pkg.links()),
        "visible": exposure(protection, form),
    }))
}

#[cfg(target_arch = "wasm32")]
fn convert(_s: &mut crate::Session, _v: &Value) -> CmdResult {
    Err(CmdError::Failed("converting files by path isn't available in the browser".into()))
}

/// `crypto.encryptFile`: any file in AES Crypt.
#[cfg(not(target_arch = "wasm32"))]
fn encrypt_file(_s: &mut crate::Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let pw = password(v, "password", "passwordEnv")?.ok_or_else(|| CmdError::Params("`password` (or `passwordEnv`) is required".into()))?;
    let to = p::str(v, "to").map_or_else(|| format!("{path}.aes"), str::to_string);
    if !p::bool(v, "overwrite").unwrap_or(false) && crate::io::folders::exists(std::path::Path::new(&to)) {
        return Err(CmdError::Failed(format!("{to} exists already (nothing was written)")));
    }
    let plain = Zeroizing::new(crate::io::read_file_capped(path, crypt::MAX_SIZE as u64, "file").map_err(CmdError::Failed)?);
    let out = crypt::aescrypt::encrypt(&plain, &pw).map_err(|e| CmdError::Failed(e.to_string()))?;
    let back = crypt::aescrypt::decrypt(&out, &pw).map_err(|e| CmdError::Failed(format!("verification failed: {e}")))?;
    if back.as_slice() != plain.as_slice() {
        return Err(CmdError::Failed("verification failed".into()));
    }
    crate::io::write_file(&to, &out).map_err(CmdError::Failed)?;
    Ok(json!({"written": to, "method": "aescrypt", "visible": exposure(Protection::AesCrypt, Form::Zip)}))
}

/// `crypto.decryptFile`: an AES Crypt file, or a ZIP with encrypted entries (written as an
/// ordinary ZIP).
#[cfg(not(target_arch = "wasm32"))]
fn decrypt_file(_s: &mut crate::Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let pw = password(v, "password", "passwordEnv")?.ok_or_else(|| CmdError::Params("`password` (or `passwordEnv`) is required".into()))?;
    let bytes = crate::io::read_file_capped(path, crypt::MAX_SIZE as u64 + 4096, "file").map_err(CmdError::Failed)?;
    let kind = crypt::detect(&bytes);
    let (plain, default_to) = match kind {
        crypt::Kind::AesCrypt(_) => {
            let plain = crypt::aescrypt::decrypt(&bytes, &pw).map_err(|e| CmdError::Failed(e.to_string()))?;
            (plain.to_vec(), path.strip_suffix(".aes").filter(|s| !s.is_empty()).map_or_else(|| format!("{path}.decrypted"), str::to_string))
        }
        crypt::Kind::ZipAes | crypt::Kind::ZipLegacy => {
            let plain = crypt::zipaes::decrypt(&bytes, &pw).map_err(|e| CmdError::Failed(e.to_string()))?;
            let stem = path.strip_suffix(".zip").unwrap_or(path);
            (plain, format!("{stem}-decrypted.zip"))
        }
        k => return Err(CmdError::Failed(format!("{path}: {} files can't be decrypted here", k.name()))),
    };
    let plain = Zeroizing::new(plain);
    let to = p::str(v, "to").map_or(default_to, str::to_string);
    if !p::bool(v, "overwrite").unwrap_or(false) && crate::io::folders::exists(std::path::Path::new(&to)) {
        return Err(CmdError::Failed(format!("{to} exists already (nothing was written)")));
    }
    crate::io::write_file(&to, &plain).map_err(CmdError::Failed)?;
    Ok(json!({"written": to, "method": kind.name()}))
}

#[cfg(not(target_arch = "wasm32"))]
fn inspect(_s: &mut crate::Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let head = crate::io::read_file_capped(path, crypt::MAX_SIZE as u64 + 4096, "file").map_err(CmdError::Failed)?;
    Ok(kind_json(crypt::detect(&head)))
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn kind_json(k: crypt::Kind) -> Value {
    json!({
        "kind": k.name(),
        "encrypted": k.is_encrypted(),
        "namesVisible": matches!(k, crypt::Kind::ZipAes | crypt::Kind::ZipLegacy),
        "weak": k == crypt::Kind::ZipLegacy,
        "version": match k { crypt::Kind::AesCrypt(v) => json!(v), _ => Value::Null },
    })
}

#[cfg(target_arch = "wasm32")]
fn encrypt_file(_s: &mut crate::Session, _v: &Value) -> CmdResult {
    Err(CmdError::Failed("encrypting files by path isn't available in the browser".into()))
}

#[cfg(target_arch = "wasm32")]
fn decrypt_file(_s: &mut crate::Session, _v: &Value) -> CmdResult {
    Err(CmdError::Failed("decrypting files by path isn't available in the browser".into()))
}

#[cfg(target_arch = "wasm32")]
fn inspect(_s: &mut crate::Session, _v: &Value) -> CmdResult {
    Err(CmdError::Failed("inspecting files by path isn't available in the browser".into()))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("transport.convert", "Convert Document Package", "File › Export › Transport", convert)
            .params(r#"{"from": path (.md, .zip, .textpack, .textbundle, + .aes), "to": path (same forms; .aes or a .zip with a password protects it), "password"? | "passwordEnv"?: protects the result, "sourcePassword"? | "sourcePasswordEnv"?: opens an encrypted source, "overwrite"?: bool} → {form, protection, written, files, links: {found, missing, outside, remote, unused}, visible}: the Markdown and every file, byte for byte; remote resources listed, never downloaded"#)
            .pure(),
        CommandSpec::new("crypto.encryptFile", "Encrypt File", "File › Info › Protect", encrypt_file)
            .params(r#"{"path": any file, "to"?: path (default `path.aes`), "password" | "passwordEnv": name of an environment variable, "overwrite"?: bool} → {written, method, visible}: AES Crypt (format 3: AES-256, PBKDF2-HMAC-SHA512), verified before it is written"#)
            .pure(),
        CommandSpec::new("crypto.decryptFile", "Decrypt File", "File › Info › Protect", decrypt_file)
            .params(r#"{"path": an .aes file or an encrypted .zip, "to"?: path, "password" | "passwordEnv", "overwrite"?: bool} → {written, method}: integrity checked first; nothing is written on a wrong password or an altered file"#)
            .pure(),
        CommandSpec::new("crypto.inspect", "Inspect Encryption", "File › Info › Protect", inspect)
            .params(r#"{"path"} → {kind, encrypted, namesVisible, weak, version}: how a file is protected, without a password"#)
            .pure(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg() -> Package {
        Package {
            text_path: "report.md".into(),
            text: b"# Report\n\n![Chart](images/chart.png)\n\nSee [the data](data/figures%201.csv \"Figures\") and <img src=\"./images/logo.png\">.\n\n[ref]: images/chart.png\n\n![](https://example.org/remote.png)\n".to_vec(),
            files: vec![
                ("images/chart.png".into(), b"\x89PNG chart".to_vec()),
                ("data/figures 1.csv".into(), b"a,b\n1,2\n".to_vec()),
                ("images/logo.png".into(), b"\x89PNG logo".to_vec()),
                ("notes/unused.txt".into(), b"kept anyway".to_vec()),
            ],
            info: None,
        }
    }

    #[test]
    fn links_are_found_listed_and_never_fetched() {
        let l = pkg().links();
        assert_eq!(l.found, ["images/chart.png", "data/figures 1.csv", "images/logo.png"]);
        assert_eq!(l.remote, ["https://example.org/remote.png"]);
        assert_eq!(l.unused, ["notes/unused.txt"]);
        let mut p = pkg();
        p.text.extend_from_slice(b"![](../outside.png) ![](missing.png)\n");
        let l = p.links();
        assert_eq!((l.outside, l.missing), (vec!["../outside.png".to_string()], vec!["missing.png".to_string()]));
    }

    #[test]
    fn relative_paths_and_relinking() {
        assert_eq!(relative("", "images/a.png"), "images/a.png");
        assert_eq!(relative("docs/", "images/a.png"), "../images/a.png");
        assert_eq!(relative("docs/", "docs/a.png"), "a.png");
        let map = vec![("images/a.png".to_string(), "assets/images/a.png".to_string())];
        assert_eq!(
            relink("![x](images/a.png) ![y](./images/a.png \"t\") <img src=\"images/a.png\">", &map),
            "![x](assets/images/a.png) ![y](./assets/images/a.png \"t\") <img src=\"assets/images/a.png\">"
        );
        // Not a link target: left alone.
        assert_eq!(relink("the file images/a.png is here; ![](old/images/a.png)", &map), "the file images/a.png is here; ![](old/images/a.png)");
    }

    /// md+assets → TextPack → ZIP → md+assets gives the very same package back.
    #[test]
    fn markdown_textpack_zip_markdown_is_lossless() {
        let original = pkg();
        let textpack = to_bytes(&original, Form::TextPack, Protection::None, None).unwrap();
        // Inside, a proper TextPack: text.md, info.json, everything under assets/ and linked there.
        let u = textbundle::unpack(&textpack).unwrap();
        assert_eq!(u.text_name, "text.md");
        assert!(u.files.iter().all(|(p, _)| p.starts_with("assets/")), "{:?}", u.files.iter().map(|f| &f.0).collect::<Vec<_>>());
        let t = String::from_utf8(u.text.clone()).unwrap();
        assert!(
            t.contains("](assets/images/chart.png)") && t.contains("](assets/data/figures%201.csv") && t.contains("src=\"./assets/images/logo.png\""),
            "{t}"
        );
        assert!(u.file("assets/images/chart.png").is_some());
        let from_pack = from_bytes(Form::TextPack, &textpack, None).unwrap();
        assert!(same_package(&from_pack, &original));
        let zip = to_bytes(&from_pack, Form::Zip, Protection::None, None).unwrap();
        let back = from_bytes(Form::Zip, &zip, None).unwrap();
        assert!(same_package(&back, &original), "{back:?}");
    }

    /// md+assets → ZIP → encrypt → decrypt → extract: the same files, byte for byte.
    #[test]
    fn zip_encrypted_and_decrypted_is_lossless() {
        let original = pkg();
        for protection in [Protection::ZipAes, Protection::AesCrypt] {
            let enc = to_bytes(&original, Form::Zip, protection, Some("pass phrase")).unwrap();
            assert!(crypt::detect(&enc).is_encrypted());
            assert!(from_bytes(Form::Zip, &enc, None).unwrap_err().contains("password"));
            assert!(from_bytes(Form::Zip, &enc, Some("wrong")).is_err());
            assert!(same_package(&from_bytes(Form::Zip, &enc, Some("pass phrase")).unwrap(), &original));
        }
        let pack = to_bytes(&original, Form::TextPack, Protection::AesCrypt, Some("pw")).unwrap();
        assert!(same_package(&from_bytes(Form::TextPack, &pack, Some("pw")).unwrap(), &original));
        assert!(to_bytes(&original, Form::TextPack, Protection::ZipAes, Some("pw")).is_err());
        assert!(to_bytes(&original, Form::Zip, Protection::ZipAes, None).is_err());
    }

    #[test]
    fn a_textpack_from_another_app_keeps_its_layout_and_metadata() {
        let mut info = textbundle::info_for_save(None);
        info.insert("com.example.app".into(), json!({"theme": "dark"}));
        let bytes = textbundle::pack(&info, "text.md", b"# T\n\n![](assets/a.png)\n", &[("assets/a.png".into(), b"png".as_slice())]).unwrap();
        let p = from_bytes(Form::TextPack, &bytes, None).unwrap();
        assert_eq!((p.text_path.as_str(), p.files[0].0.as_str()), ("text.md", "assets/a.png"));
        let again = to_bytes(&p, Form::TextPack, Protection::None, None).unwrap();
        let u = textbundle::unpack(&again).unwrap();
        assert_eq!(u.info["com.example.app"]["theme"], "dark");
        assert!(!u.info.contains_key(INFO_KEY));
        assert_eq!(u.text, b"# T\n\n![](assets/a.png)\n");
    }

    #[test]
    fn forms_and_protections_from_names() {
        assert_eq!(form_of("a.md", false), Some((Form::Markdown, Protection::None)));
        assert_eq!(form_of("a.zip", true), Some((Form::Zip, Protection::ZipAes)));
        assert_eq!(form_of("a.zip", false), Some((Form::Zip, Protection::None)));
        assert_eq!(form_of("a.textpack.aes", false), Some((Form::TextPack, Protection::AesCrypt)));
        assert_eq!(form_of("A.TextBundle/", false), Some((Form::TextBundle, Protection::None)));
        assert_eq!(form_of("a.docx", true), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn convert_on_disk_round_trips_and_never_overwrites_silently() {
        let dir = std::env::temp_dir().join(format!("plume-transport-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src");
        let original = pkg();
        write_folder(&original, Form::Markdown, &src, None, false).unwrap();
        let mut s = crate::Session::new(wordcraft_doc::Document::new());
        let md = src.join("report.md").to_string_lossy().to_string();
        let zip = dir.join("report.zip").to_string_lossy().to_string();
        let r = s.run("transport.convert", &json!({"from": md, "to": zip, "password": "pw"})).unwrap();
        assert_eq!(r["protection"], "zip-aes");
        assert_eq!(r["links"]["remote"][0], "https://example.org/remote.png");
        // The source folder held an unlinked file: it isn't the document's, so it isn't carried.
        assert_eq!(r["files"], 3);
        let again = s.run("transport.convert", &json!({"from": md, "to": zip, "password": "pw"}));
        assert!(again.unwrap_err().to_string().contains("exists already"));
        let tb = dir.join("report.textbundle").to_string_lossy().to_string();
        s.run("transport.convert", &json!({"from": zip, "to": tb, "sourcePassword": "pw"})).unwrap();
        let out = dir.join("out").join("report.md").to_string_lossy().to_string();
        s.run("transport.convert", &json!({"from": tb, "to": out})).unwrap();
        let back = read(std::path::Path::new(&out), None).unwrap();
        let mut linked = original.clone();
        linked.files.retain(|(p, _)| p != "notes/unused.txt");
        assert!(same_package(&back, &linked), "{back:?}");
        // Any file, encrypted and decrypted; a wrong password writes nothing.
        let enc = s.run("crypto.encryptFile", &json!({"path": md, "password": "x"})).unwrap();
        let aes = enc["written"].as_str().unwrap().to_string();
        assert_eq!(s.run("crypto.inspect", &json!({"path": aes})).unwrap()["kind"], "AES Crypt");
        let plain = dir.join("plain.md").to_string_lossy().to_string();
        assert!(s.run("crypto.decryptFile", &json!({"path": aes, "to": plain, "password": "y"})).is_err());
        assert!(!std::path::Path::new(&plain).exists());
        s.run("crypto.decryptFile", &json!({"path": aes, "to": plain, "password": "x"})).unwrap();
        assert_eq!(std::fs::read(&plain).unwrap(), original.text);
        // No password lands in the session or its document.
        assert!(!serde_json::to_string(&s.doc).unwrap().contains("pw\""));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
