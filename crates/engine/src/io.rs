//! File format dispatch by extension. Format crates plug in here.

use wordcraft_doc::Document;

#[cfg(not(target_arch = "wasm32"))]
pub mod folders;
pub mod mdzip;
#[cfg(not(target_arch = "wasm32"))]
mod safe_save;
pub mod templates;
pub mod textbundle;

/// Formats WordCraft opens.
pub const OPEN_EXTS: &[&str] = &[
    "docx",
    "docm",
    "dotx",
    "dotm",
    "doc",
    "dot",
    "txt",
    "md",
    "markdown",
    "textbundle",
    "textpack",
    "zip",
    "html",
    "htm",
    "rtf",
    "odt",
    "tex",
    "latex",
    "ltx",
    "xml",
    "wcraft.json",
    "json",
];
/// Formats WordCraft saves (Save As).
pub const SAVE_EXTS: &[&str] =
    &["docx", "docm", "dotx", "dotm", "pdf", "txt", "md", "textbundle", "textpack", "zip", "html", "rtf", "odt", "tex", "png", "json", "xml"];

/// Magic number for ZIP package signatures (used by .docx/.docm/.dotx/.dotm).
const ZIP_SIG: &[u8] = b"PK\x03\x04";
/// Magic number for OLE compound document signatures (used by .doc/.dot).
const OLE_SIG: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The extension of a file name or path, lower-case; empty when the file name has none (a dot
/// in a folder name doesn't count).
fn ext_of(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    file.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default()
}

/// Plume: is `ext` a Markdown document's own format: a Markdown file (`.md`, `.markdown`), a
/// TextBundle folder (`.textbundle`), a TextPack (`.textpack`) or a ZIP holding a Markdown file
/// and its pictures (`.zip`)?
pub fn is_markdown_ext(ext: &str) -> bool {
    matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown" | "textbundle" | "textpack" | "zip")
}

/// [`is_markdown_ext`] for a file name or path.
pub fn is_markdown_name(name: &str) -> bool {
    is_markdown_ext(&ext_of(name))
}

/// A TextBundle's or TextPack's text as a document: Markdown with its pictures (`resolve` gives
/// a picture's bytes by its path in the bundle), or plain text when the bundle says so.
fn bundle_document(
    text: &[u8],
    markdown: bool,
    page: &wordcraft_doc::SectionProps,
    resolve: &dyn Fn(&str) -> Option<std::sync::Arc<Vec<u8>>>,
) -> Document {
    let mut doc = if markdown {
        let text = wordcraft_formats::txt::decode(text);
        let mut doc = wordcraft_formats::markdown::import_with(&text, page, resolve);
        wordcraft_formats::markdown::apply_theme(&mut doc, Default::default());
        doc
    } else {
        wordcraft_formats::txt::import_on(text, page)
    };
    fit_pictures(&mut doc);
    doc.ensure_nonempty();
    doc
}

/// Plume: `doc` as its Markdown keeps it, written as the native copy and read back. An imported
/// document then shows what saving it keeps: its styles become Markdown blocks (a caption or a
/// subtitle plain text, the title a level-1 heading), its fonts, colours and page layout give
/// way to the theme. Its pictures come along from the copy's assets.
pub fn as_markdown(doc: &Document, page: &wordcraft_doc::SectionProps) -> Document {
    let bundle = wordcraft_formats::markdown::export_native(doc, &Default::default());
    let assets: std::collections::BTreeMap<&str, std::sync::Arc<Vec<u8>>> = bundle.assets.iter().map(|(p, b)| (p.as_str(), b.clone())).collect();
    let mut out = wordcraft_formats::markdown::import_with(&bundle.text, page, &|src| assets.get(src).cloned());
    wordcraft_formats::markdown::apply_theme(&mut out, Default::default());
    // Markdown carries no language: the document keeps its own.
    out.styles.default_chr.lang = doc.styles.default_chr.lang.clone();
    fit_pictures(&mut out);
    out.ensure_nonempty();
    out
}

/// A Markdown ZIP's document ([`mdzip`]).
pub fn open_zip(bytes: &[u8], page: &wordcraft_doc::SectionProps) -> Result<Document, String> {
    let u = mdzip::unpack(bytes)?;
    Ok(bundle_document(&u.text, true, page, &|src| u.file(src).map(|b| std::sync::Arc::new(b.to_vec()))))
}

/// A Markdown ZIP of `doc` over the archive `existing` when there is one (see [`mdzip::pack`]);
/// a new archive holds `<name>.md`, `name` being the archive's file name.
pub fn zip_bytes(name: &str, doc: &Document, existing: Option<&[u8]>) -> Result<(Vec<u8>, wordcraft_formats::markdown::Report), String> {
    let bundle = wordcraft_formats::markdown::export_native(&markdown_ready(doc), &wordcraft_formats::markdown::NativeOptions::default());
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let stem = if stem.trim().is_empty() { "document" } else { stem };
    let assets: Vec<(String, &[u8])> = bundle.assets.iter().map(|(p, b)| (p.clone(), b.as_slice())).collect();
    Ok((mdzip::pack(existing, &format!("{stem}.md"), bundle.text.as_bytes(), &assets)?, bundle.report))
}

/// Save `doc` as a Markdown ZIP at `path`, over the one there.
#[cfg(not(target_arch = "wasm32"))]
fn save_zip(path: &std::path::Path, doc: &Document) -> Result<wordcraft_formats::markdown::Report, String> {
    let existing = if path.is_file() { read_regular_file(path, 2 << 30) } else { None };
    let (bytes, report) = zip_bytes(&path.to_string_lossy(), doc, existing.as_deref())?;
    safe_save::write(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(report)
}

/// A TextPack's document.
fn open_textpack(bytes: &[u8], page: &wordcraft_doc::SectionProps) -> Result<Document, String> {
    let u = textbundle::unpack(bytes)?;
    Ok(bundle_document(&u.text, u.is_markdown(), page, &|src| u.file(src).map(|b| std::sync::Arc::new(b.to_vec()))))
}

/// A TextBundle folder's document: its `info.json` and text file, its pictures read from the
/// folder (within it only, with the limits of [`LocalImages`]).
#[cfg(not(target_arch = "wasm32"))]
fn open_textbundle(dir: &std::path::Path, page: &wordcraft_doc::SectionProps) -> Result<Document, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let names: Vec<String> = entries.flatten().filter(|e| e.path().is_file()).filter_map(|e| e.file_name().into_string().ok()).take(10_000).collect();
    let text_name = textbundle::pick_text(names.iter().map(String::as_str))
        .ok_or_else(|| format!("{}: the TextBundle has no text file (text.md)", dir.display()))?;
    let text = read_regular_file(&dir.join(text_name), 64 << 20).ok_or_else(|| format!("{}: can't read {text_name}", dir.display()))?;
    let info = read_regular_file(&dir.join("info.json"), textbundle::MAX_INFO).map(|b| textbundle::parse_info(&b)).unwrap_or_default();
    let images = LocalImages::new(dir);
    Ok(bundle_document(&text, textbundle::is_markdown_type(&info), page, &|src| images.load(src)))
}

/// Is `ext` one of Word's package formats (.docx/.docm/.dotx/.dotm), which save without loss?
pub fn is_word_package(ext: &str) -> bool {
    wordcraft_docx::Flavor::from_ext(ext).is_some()
}

/// How a copy in another format is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExportOptions {
    /// Print Markup: PDFs show tracked changes marked up and, with `comments`, each page scaled
    /// down beside a markup area with the comments in balloons (page images get the area too);
    /// HTML, Markdown and text copies end with the comments.
    pub markup: bool,
    /// With `markup`, show the comments (Track Changes Options › Comments).
    pub comments: bool,
    /// An `.xml` copy is a Word 2003 XML Document (WordprocessingML 2003) instead of a Word XML
    /// Document (Flat OPC): `format: "wordml2003"` on the save commands.
    pub word2003: bool,
}

/// A document password: wiped from memory when dropped, never printed (`Debug` shows `***`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Password(zeroize::Zeroizing<String>);

impl Password {
    pub fn new(s: &str) -> Password {
        Password(zeroize::Zeroizing::new(s.to_string()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// The text, for a password box to edit in place.
    pub fn as_mut_string(&mut self) -> &mut String {
        &mut self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(***)")
    }
}

/// Does this open error mean the file is password-protected and no password was given?
pub fn needs_password(err: &str) -> bool {
    err.contains(wordcraft_docx::PASSWORD_REQUIRED)
}

/// Does this open error mean the password given was wrong?
pub fn wrong_password(err: &str) -> bool {
    err.contains(wordcraft_docx::WRONG_PASSWORD)
}

/// Is the file at `path` empty, zero bytes long? File Explorer's New › Microsoft Word Document
/// makes one, and `file.open` opens it as a new blank document (#1384). False for a file that
/// can't be read: opening it says why.
#[cfg(not(target_arch = "wasm32"))]
pub fn is_empty_file(path: &std::path::Path) -> bool {
    crate::file_access::check_read(&path.to_string_lossy()).is_ok() && std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == 0)
}

/// [`is_empty_file`]: on the web files come through the browser, never by path.
#[cfg(target_arch = "wasm32")]
pub fn is_empty_file(_path: &std::path::Path) -> bool {
    false
}

/// Can files with this extension be saved with a password (Word's packages)?
pub fn can_encrypt(name: &str) -> bool {
    is_word_package(&ext_of(name))
}

/// Finish a document just read from a file: never leave it empty, and bring its unlocked
/// `DATE`/`TIME` fields up to now the way Word does on open (see
/// [`crate::cmd::references::refresh_date_fields`]).
fn opened(doc: &mut Document) {
    doc.ensure_nonempty();
    crate::cmd::references::refresh_date_fields(doc);
}

/// Parse a document from bytes; `name` gives the format by extension.
pub fn open_bytes(name: &str, bytes: &[u8]) -> Result<Document, String> {
    open_bytes_with(name, bytes, None).map(|(doc, _)| doc)
}

/// Open a Word 97-2003 binary file, decrypting it with `password` when it is password-protected
/// (XOR, RC4, RC4 CryptoAPI). The errors for a missing or wrong password are the same texts the
/// `.docx` reader gives, so [`needs_password`] and [`wrong_password`] recognise them.
fn open_binary_doc(bytes: &[u8], password: Option<&str>) -> Result<(Document, bool), String> {
    use wordcraft_docbin::DocbinError;
    let encrypted = wordcraft_docbin::is_encrypted(bytes);
    match wordcraft_docbin::read_with_password(bytes, password) {
        Ok(doc) => Ok((doc, encrypted)),
        Err(DocbinError::PasswordRequired) => Err(wordcraft_docx::PASSWORD_REQUIRED.to_string()),
        Err(DocbinError::WrongPassword) => Err(wordcraft_docx::WRONG_PASSWORD.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// [`open_bytes`] for a file that may be password-protected: a Word document encrypted with a
/// password ([MS-OFFCRYPTO] agile encryption, or the Word 97-2003 schemes) is decrypted with
/// `password` first. Also says
/// whether the file was encrypted. Without the password needed the error satisfies
/// [`needs_password`]; with a wrong one, [`wrong_password`].
pub fn open_bytes_with(name: &str, bytes: &[u8], password: Option<&str>) -> Result<(Document, bool), String> {
    open_bytes_in(name, bytes, password, &wordcraft_geom::Region::default())
}

/// The page a file that gives no page size (plain text, Markdown, HTML) opens on: `region`'s
/// paper, as a new blank document (#838, #1381), with the default margins.
pub fn region_page(region: &wordcraft_geom::Region) -> wordcraft_doc::SectionProps {
    let (_, w, h) = region.paper();
    wordcraft_doc::SectionProps { page_w: w, page_h: h, ..Default::default() }
}

/// [`open_bytes_with`] in `region`: a file that gives no page size opens on the region's paper
/// (see [`region_page`]); a document with its own page size keeps it.
pub fn open_bytes_in(name: &str, bytes: &[u8], password: Option<&str>, region: &wordcraft_geom::Region) -> Result<(Document, bool), String> {
    crate::diagrams::register();
    let page = region_page(region);
    let (doc, encrypted) = read_bytes(name, bytes, password, &page)?;
    load_embedded_fonts(&doc);
    Ok((doc, encrypted))
}

/// Load the fonts `doc` embeds (#1171) for this run, in memory only: text set in a font that isn't
/// installed then lays out and exports with it instead of a fallback. Faces already available
/// stay as they are.
fn load_embedded_fonts(doc: &Document) {
    let files: Vec<(Vec<u8>, String)> = wordcraft_docx::embedded_fonts(doc)
        .into_iter()
        .filter_map(|f| wordcraft_fonts::decode_font_file(&f.data).ok().map(|data| (data, f.name)))
        .collect();
    if !files.is_empty() {
        wordcraft_fonts::FontDb::global().add_document_fonts(files);
    }
}

fn read_bytes(name: &str, bytes: &[u8], password: Option<&str>, page: &wordcraft_doc::SectionProps) -> Result<(Document, bool), String> {
    let ext = ext_of(name);
    if matches!(ext.as_str(), "docx" | "docm" | "dotx" | "dotm" | "doc" | "dot") && wordcraft_docx::is_encrypted(bytes) {
        let mut doc = wordcraft_docx::read_with_password(bytes, password).map_err(|e| e.to_string())?;
        opened(&mut doc);
        return Ok((doc, true));
    }
    if ext.is_empty() || signature_contradicts(&ext, bytes) {
        return open_by_content(name, &ext, bytes, password, page);
    }
    let mut doc = match ext.as_str() {
        "doc" | "dot" => {
            let (mut doc, encrypted) = open_binary_doc(bytes, password)?;
            opened(&mut doc);
            return Ok((doc, encrypted));
        }
        // Plain text goes through the formats importer: BOMs, unmarked UTF-16, Windows-1252.
        "txt" | "text" => wordcraft_formats::txt::import_on(bytes, page),
        "json" => {
            let mut d = serde_json::from_slice::<Document>(bytes).map_err(|e| format!("{name}: {e}"))?;
            d.normalize_runs();
            d
        }
        "textpack" => open_textpack(bytes, page).map_err(|e| format!("{name}: {e}"))?,
        "zip" => open_zip(bytes, page).map_err(|e| format!("{name}: {e}"))?,
        "textbundle" => return Err(format!("{name}: a TextBundle is a folder; open the folder, or a .textpack file")),
        other => match crate::io_ext::open(other, bytes, page) {
            Some(r) => r?,
            None => return open_by_content(name, other, bytes, password, page),
        },
    };
    // Markdown looks as the site it's read on shows it.
    if matches!(ext.as_str(), "md" | "markdown") {
        wordcraft_formats::markdown::apply_theme(&mut doc, Default::default());
    }
    opened(&mut doc);
    Ok((doc, false))
}

/// The narrowest text width of the document's sections, points: what a picture must fit.
pub(crate) fn text_width(doc: &Document) -> f32 {
    doc.sections().iter().map(|(_, sp)| sp.text_width()).fold(f32::INFINITY, f32::min).clamp(36.0, 4000.0)
}

/// Shrink inline pictures wider than `max_w` to it, keeping their proportions. Layout keeps a
/// picture's size (as Word does), so formats whose pictures come at their pixel size (HTML,
/// Markdown, ODT, RTF) are fitted when they are read, as Insert › Pictures does.
pub(crate) fn fit_block_pictures(b: &mut wordcraft_doc::Block, max_w: f32) {
    crate::cmd::paste::each_para(b, 0, &mut |p| {
        let mut changed = false;
        for o in &mut p.objects {
            if let wordcraft_doc::InlineObject::Image { w, h, float, .. } = o
                && float.wrap == wordcraft_doc::para::Wrap::Inline
                && w.is_finite()
                && *w > max_w
            {
                let k = max_w / *w;
                *w = max_w;
                *h = (*h * k).max(1.0);
                changed = true;
            }
        }
        if changed {
            p.touch();
        }
    });
}

/// [`fit_block_pictures`] for a whole document read from a file, against its own text width.
pub(crate) fn fit_pictures(doc: &mut Document) {
    let max_w = text_width(doc);
    for b in &mut doc.body {
        fit_block_pictures(std::sync::Arc::make_mut(b), max_w);
    }
}

/// Does the file hold a different document format than its Word or RTF extension says? An RTF
/// file saved as `.doc` (as accounting systems and report generators often save their
/// documents), a `.docx` package renamed `.doc`, a Word 97-2003 file renamed `.docx`, a Word XML
/// Document (2003 or Flat OPC) or a web page saved as `.doc` (#2853): Word opens
/// each by what it holds, so they go through [`open_by_content`] instead of failing.
fn signature_contradicts(ext: &str, bytes: &[u8]) -> bool {
    let expected = match ext {
        "docx" | "docm" | "dotx" | "dotm" => "zip",
        "doc" | "dot" => "ole",
        "rtf" => "rtf",
        _ => return false,
    };
    let held = if bytes.starts_with(ZIP_SIG) {
        "zip"
    } else if bytes.starts_with(OLE_SIG) {
        "ole"
    } else if bytes.starts_with(b"{\\rtf") {
        "rtf"
    } else if is_word_xml(bytes) {
        "xml"
    } else if looks_like_web_page(bytes) {
        "html"
    } else {
        return false;
    };
    held != expected
}

/// The bytes after a UTF-8 byte-order mark and leading white space.
fn after_bom(bytes: &[u8]) -> &[u8] {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    let start = b.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(b.len());
    b.get(start..).unwrap_or_default()
}

/// Is this a Word XML Document: Word 2003 XML (`w:wordDocument`) or Flat OPC (`pkg:package`)?
/// Only text that starts with markup is parsed (UTF-16 has its byte-order mark).
fn is_word_xml(bytes: &[u8]) -> bool {
    let utf16 = bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]);
    (utf16 || after_bom(bytes).starts_with(b"<")) && !matches!(wordcraft_docx::sniff_xml(bytes), wordcraft_docx::XmlKind::Other)
}

/// Does the file start like a web page (`<!DOCTYPE html …>` or `<html …>`, after an XML
/// declaration and comments)? Word's Save as Web Page and many report generators write HTML
/// under a `.doc` name, and Word opens it as a web page.
fn looks_like_web_page(bytes: &[u8]) -> bool {
    let head = wordcraft_formats::txt::decode(bytes.get(..bytes.len().min(4096)).unwrap_or(bytes)).to_ascii_lowercase();
    let mut t = head.trim_start_matches(['\u{FEFF}', ' ', '\t', '\r', '\n']);
    for _ in 0..64 {
        let skip = if t.starts_with("<!--") {
            t.find("-->").map(|i| i + 3)
        } else if t.starts_with("<?") {
            t.find("?>").map(|i| i + 2)
        } else {
            break;
        };
        let Some(rest) = skip.and_then(|i| t.get(i..)) else { return false };
        t = rest.trim_start();
    }
    // The tag name ends there: `<htmlish>` is some other XML, not a web page.
    let tag = |name: &str| {
        t.strip_prefix(name).is_some_and(|rest| rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/'))
    };
    tag("<!doctype html") || tag("<html")
}

/// A file whose extension names no format WordCraft knows (`.m3u`, `.log`, `.ini`, none…),
/// opened by what it holds, as Word does: a Word document or RTF file by its signature, a Word
/// XML Document by its root element, a web page under a Word or RTF name as one
/// (#2853), any other text through the plain-text import with encoding detection (#569).
/// Anything else is refused with a clear error rather than shown as garbage.
fn open_by_content(
    name: &str,
    ext: &str,
    bytes: &[u8],
    password: Option<&str>,
    page: &wordcraft_doc::SectionProps,
) -> Result<(Document, bool), String> {
    let (mut doc, encrypted) = if bytes.starts_with(OLE_SIG) && wordcraft_docx::is_encrypted(bytes) {
        (wordcraft_docx::read_with_password(bytes, password).map_err(|e| e.to_string())?, true)
    } else if bytes.starts_with(ZIP_SIG) {
        (wordcraft_docx::read(bytes).map_err(|e| format!("{name}: not a document {} can open ({e})", wordcraft_brand::APP_NAME))?, false)
    } else if bytes.starts_with(OLE_SIG) {
        open_binary_doc(bytes, password).map_err(|e| {
            if needs_password(&e) || wrong_password(&e) { e } else { format!("{name}: not a document {} can open ({e})", wordcraft_brand::APP_NAME) }
        })?
    } else if bytes.starts_with(b"{\\rtf") {
        (wordcraft_formats::rtf::import(bytes)?, false)
    } else if is_word_xml(bytes) {
        (wordcraft_docx::read_xml(bytes).map_err(|e| format!("{name}: {e}"))?, false)
    } else if matches!(ext, "doc" | "dot" | "docx" | "docm" | "dotx" | "dotm" | "rtf") && looks_like_web_page(bytes) {
        let opened = crate::io_ext::open("html", bytes, page).unwrap_or_else(|| Err(format!("{name}: can't open web pages")));
        (opened?, false)
    } else if wordcraft_formats::txt::is_text(bytes) {
        (wordcraft_formats::txt::import_on(bytes, page), false)
    } else if ext.is_empty() {
        return Err(format!("{name}: not a document or text file {} can open", wordcraft_brand::APP_NAME));
    } else {
        return Err(format!("{name}: unsupported format `.{ext}` (not a document or text file)"));
    };
    opened(&mut doc);
    Ok((doc, encrypted))
}

/// Serialise a document; `name` gives the format by extension.
pub fn save_bytes(name: &str, doc: &Document) -> Result<Vec<u8>, String> {
    save_bytes_opts(name, doc, &ExportOptions::default())
}

/// [`save_bytes`] with export options (Print Markup).
pub fn save_bytes_opts(name: &str, doc: &Document, opts: &ExportOptions) -> Result<Vec<u8>, String> {
    let ext = ext_of(name);
    match ext.as_str() {
        // List paragraphs get their label and a tab, as in Word's plain-text save.
        "txt" | "text" => {
            let bytes = wordcraft_formats::txt::export(doc);
            Ok(if opts.markup && opts.comments { wordcraft_formats::comments::append("txt", doc, bytes) } else { bytes })
        }
        "json" => serde_json::to_vec_pretty(doc).map_err(|e| e.to_string()),
        "textpack" => textpack_bytes(doc, None).map(|(bytes, _)| bytes),
        "zip" => zip_bytes(name, doc, None).map(|(bytes, _)| bytes),
        "textbundle" => Err(format!("{name}: a TextBundle is a folder; save a .textpack to download it")),
        other => match crate::io_ext::save(other, doc, opts) {
            Some(r) => r,
            None => Err(format!("{name}: can't save as `.{other}`")),
        },
    }
}

/// [`save_bytes`], encrypted with `password` when there is one and the format is a Word package
/// (other formats are copies and are written as they are).
pub fn save_bytes_with(name: &str, doc: &Document, password: Option<&str>, opts: &ExportOptions) -> Result<Vec<u8>, String> {
    let bytes = save_bytes_opts(name, doc, opts)?;
    match password {
        Some(pw) if can_encrypt(name) => wordcraft_docx::encrypt(&bytes, pw).map_err(|e| e.to_string()),
        _ => Ok(bytes),
    }
}

pub fn open_path(path: &std::path::Path) -> Result<Document, String> {
    open_path_with(path, None).map(|(doc, _)| doc)
}

/// [`open_path`] for a file that may be password-protected (see [`open_bytes_with`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn open_path_with(path: &std::path::Path, password: Option<&str>) -> Result<(Document, bool), String> {
    open_path_in(path, password, &wordcraft_geom::Region::default())
}

/// [`open_path_with`] in `region` (see [`open_bytes_in`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn open_path_in(path: &std::path::Path, password: Option<&str>, region: &wordcraft_geom::Region) -> Result<(Document, bool), String> {
    crate::diagrams::register();
    crate::file_access::check_read(&path.to_string_lossy())?;
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // A TextBundle is a folder.
    if meta.is_dir() && ext_of(&path.to_string_lossy()) == "textbundle" {
        let mut doc = open_textbundle(path, &region_page(region))?;
        opened(&mut doc);
        return Ok((doc, false));
    }
    if meta.len() > 2 << 30 {
        return Err(format!("{}: file is larger than 2 GB", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.to_string_lossy();
    if matches!(ext_of(&name).as_str(), "html" | "htm" | "xhtml") {
        // Pictures referenced by a relative path load from beside the HTML file.
        let images = LocalImages::new(path.parent().unwrap_or(std::path::Path::new("")));
        let html = wordcraft_formats::html::decode(&bytes);
        let mut doc = wordcraft_formats::html::import_on(&html, &|src| images.load(src), &region_page(region));
        fit_pictures(&mut doc);
        opened(&mut doc);
        return Ok((doc, false));
    }
    if is_markdown(&ext_of(&name)) {
        // Pictures named by a relative path (the `assets` folder a native save writes) load from
        // beside the Markdown file, with the same limits as HTML.
        let images = LocalImages::new(path.parent().unwrap_or(std::path::Path::new("")));
        let text = wordcraft_formats::txt::decode(&bytes);
        let mut doc = wordcraft_formats::markdown::import_with(&text, &region_page(region), &|src| images.load(src));
        wordcraft_formats::markdown::apply_theme(&mut doc, Default::default());
        // The folder's publication profile: page, margins, fonts, theme.
        if let Some((profile, _)) = crate::publication::Profile::for_document(path) {
            profile.apply(&mut doc);
        }
        fit_pictures(&mut doc);
        doc.ensure_nonempty();
        return Ok((doc, false));
    }
    open_bytes_in(&name, &bytes, password, region)
}

/// The pictures an HTML file names by a path relative to its folder.
///
/// Only regular files inside the folder (after resolving `..` and symlinks) are read. The folder is
/// the HTML file's own, which `open_path_with` has already checked against the automation read root. URLs
/// (`http:`, `file:`…) and absolute paths are not followed. Each file is read at most once and
/// at most `max_file` bytes of it; every reference, cached or not, counts against a budget for
/// the whole document, so one large picture can't be multiplied by referencing it many times.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct LocalImages {
    /// The folder, canonical; `None` when it can't be resolved (nothing loads then).
    dir: Option<std::path::PathBuf>,
    max_file: u64,
    /// Bytes the document may still load.
    left: std::cell::Cell<u64>,
    cache: std::cell::RefCell<Vec<(std::path::PathBuf, std::sync::Arc<Vec<u8>>)>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalImages {
    /// Largest picture file read.
    const MAX_FILE: u64 = 256 << 20;
    /// Picture bytes one document may load, counting every reference.
    const BUDGET: u64 = 1 << 30;
    /// Files whose bytes are kept for further references.
    const CACHE: usize = 64;

    pub(crate) fn new(dir: &std::path::Path) -> LocalImages {
        LocalImages::with_limits(dir, LocalImages::MAX_FILE, LocalImages::BUDGET)
    }

    pub(crate) fn with_limits(dir: &std::path::Path, max_file: u64, budget: u64) -> LocalImages {
        let dir = if dir.as_os_str().is_empty() { std::path::Path::new(".") } else { dir };
        LocalImages { dir: std::fs::canonicalize(dir).ok(), max_file, left: std::cell::Cell::new(budget), cache: Default::default() }
    }

    /// Bytes of the file `src` names, or `None` when it isn't a regular file inside the folder,
    /// is too large, or the budget is spent.
    pub(crate) fn load(&self, src: &str) -> Option<std::sync::Arc<Vec<u8>>> {
        let path = self.resolve(src)?;
        let cached = self.cache.borrow().iter().find(|(p, _)| *p == path).map(|(_, d)| d.clone());
        if let Some(data) = cached {
            self.charge(data.len())?;
            return Some(data);
        }
        let limit = self.max_file.min(self.left.get());
        let bytes = read_regular_file(&path, limit)?;
        // What was read is spent even when it's over the limit.
        self.charge(bytes.len())?;
        if bytes.len() as u64 > limit {
            return None;
        }
        let data = std::sync::Arc::new(bytes);
        let mut cache = self.cache.borrow_mut();
        if cache.len() < LocalImages::CACHE {
            cache.push((path, data.clone()));
        }
        Some(data)
    }

    /// Take `n` bytes from the budget; `None` (and the budget emptied) when there aren't enough.
    fn charge(&self, n: usize) -> Option<()> {
        let left = self.left.get().checked_sub(n as u64);
        self.left.set(left.unwrap_or(0));
        left.map(|_| ())
    }

    /// The canonical path `src` names, if it's inside the folder.
    fn resolve(&self, src: &str) -> Option<std::path::PathBuf> {
        let src = src.split(['#', '?']).next().unwrap_or("");
        if src.is_empty() || src.starts_with("//") || src.split_once(':').is_some_and(|(scheme, _)| !scheme.contains('/')) {
            return None;
        }
        let rel = String::from_utf8(wordcraft_formats::model::percent_decode(src)).ok()?;
        let rel = std::path::Path::new(&rel);
        if rel.components().any(|c| matches!(c, std::path::Component::Prefix(_) | std::path::Component::RootDir)) {
            return None;
        }
        let dir = self.dir.as_ref()?;
        // Canonical paths have `..` and symlinks resolved, so this keeps both inside the folder.
        let path = std::fs::canonicalize(dir.join(rel)).ok()?;
        path.starts_with(dir).then_some(path)
    }
}

/// Up to `limit + 1` bytes of the regular file at `path` (more than `limit` means too large), or
/// `None` for anything else. A named pipe blocks when opened and a device reports a length of 0
/// and may never end, so the path is checked before opening, the opened handle is checked again,
/// and the read is bounded whatever length the file reports.
#[cfg(not(target_arch = "wasm32"))]
fn read_regular_file(path: &std::path::Path, limit: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut out = Vec::with_capacity(usize::try_from(meta.len().min(limit)).unwrap_or(0));
    file.take(limit.saturating_add(1)).read_to_end(&mut out).ok()?;
    Some(out)
}

/// The bytes of the file at `path`, for commands that take a path (pictures, mail-merge data).
/// Every such read goes through here so the automation read root applies ([`crate::file_access`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn read_file(path: &str) -> Result<Vec<u8>, String> {
    crate::file_access::check_read(path)?;
    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
}

#[cfg(target_arch = "wasm32")]
pub fn read_file(path: &str) -> Result<Vec<u8>, String> {
    Err(format!("{path}: files are opened through the browser on the web"))
}

/// [`read_file`] for a file that must be a regular file of at most `max` bytes, checked before
/// anything is read (a font named by the user, which could be any size).
#[cfg(not(target_arch = "wasm32"))]
pub fn read_file_capped(path: &str, max: u64, what: &str) -> Result<Vec<u8>, String> {
    crate::file_access::check_read(path)?;
    let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{path}: not a file"));
    }
    if meta.len() > max {
        return Err(format!("{path}: the {what} is larger than {} MB", max >> 20));
    }
    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
}

/// The first `max` bytes of the file at `path` (fewer when it is shorter), inside the automation
/// read root when there is one: enough to tell a file's format by its start.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_file_head(path: &str, max: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    crate::file_access::check_read(path)?;
    let f = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut head = Vec::new();
    f.take(max).read_to_end(&mut head).map_err(|e| format!("{path}: {e}"))?;
    Ok(head)
}

/// Rename without replacing another file, respecting both automation access roots.
#[cfg(not(target_arch = "wasm32"))]
pub fn rename_file(source: &std::path::Path, destination: &std::path::Path) -> Result<(), String> {
    crate::file_access::check_read(&source.to_string_lossy())?;
    crate::file_access::check_write(&source.to_string_lossy())?;
    crate::file_access::check_write(&destination.to_string_lossy())?;
    // Creating a sibling hard link refuses collisions atomically and keeps the original bytes.
    if let Err(e) = std::fs::hard_link(source, destination) {
        if e.kind() == std::io::ErrorKind::AlreadyExists && only_case_differs(source, destination) {
            return std::fs::rename(source, destination).map_err(|e| format!("Could not rename document: {e}"));
        }
        return Err(format!("Could not rename document: {e}"));
    }
    if let Err(e) = std::fs::remove_file(source) {
        let _ = std::fs::remove_file(destination);
        return Err(format!("Could not rename document: {e}"));
    }
    Ok(())
}

/// Whether `destination` names `source` in the same directory spelled with different case, and
/// the directory holds no entry spelled exactly as `destination`.
#[cfg(not(target_arch = "wasm32"))]
fn only_case_differs(source: &std::path::Path, destination: &std::path::Path) -> bool {
    let (Some(dir), Some(from), Some(to)) = (source.parent(), source.file_name(), destination.file_name()) else {
        return false;
    };
    if destination.parent() != Some(dir) || from == to || from.to_string_lossy().to_lowercase() != to.to_string_lossy().to_lowercase() {
        return false;
    }
    let dir = if dir.as_os_str().is_empty() { std::path::Path::new(".") } else { dir };
    std::fs::read_dir(dir).is_ok_and(|entries| entries.filter_map(Result::ok).all(|entry| entry.file_name() != to))
}

/// Write `bytes` to the file at `path`, inside the automation write root when there is one.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    crate::file_access::check_write(path)?;
    // Like a document save: the old file is replaced only once the new bytes are fully written.
    safe_save::write(std::path::Path::new(path), bytes).map_err(|e| format!("{path}: {e}"))
}

#[cfg(target_arch = "wasm32")]
pub fn write_file(path: &str, _bytes: &[u8]) -> Result<(), String> {
    Err(format!("{path}: files are saved through the browser on the web"))
}

#[cfg(target_arch = "wasm32")]
pub fn open_path_with(path: &std::path::Path, _password: Option<&str>) -> Result<(Document, bool), String> {
    Err(format!("{}: files are opened through the browser on the web", path.display()))
}

#[cfg(target_arch = "wasm32")]
pub fn open_path_in(path: &std::path::Path, password: Option<&str>, _region: &wordcraft_geom::Region) -> Result<(Document, bool), String> {
    open_path_with(path, password)
}

pub fn save_path(path: &std::path::Path, doc: &Document) -> Result<(), String> {
    save_path_with(path, doc, None, &ExportOptions::default())
}

/// [`save_path`], encrypted with `password` as [`save_bytes_with`] does.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_path_with(path: &std::path::Path, doc: &Document, password: Option<&str>, opts: &ExportOptions) -> Result<(), String> {
    crate::file_access::check_write(&path.to_string_lossy())?;
    if is_markdown_name(&path.to_string_lossy()) {
        return save_markdown(path, doc).map(|_| ());
    }
    let bytes = save_bytes_with(&path.to_string_lossy(), doc, password, opts)?;
    safe_save::write(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(not(target_arch = "wasm32"))]
fn is_markdown(ext: &str) -> bool {
    matches!(ext, "md" | "markdown")
}

/// A TextPack of `doc` (its Markdown, pictures and `info.json`), over the TextPack `existing`
/// when there is one: its other files and other applications' metadata are kept.
pub fn textpack_bytes(doc: &Document, existing: Option<&[u8]>) -> Result<(Vec<u8>, wordcraft_formats::markdown::Report), String> {
    let old = existing.and_then(|b| textbundle::unpack(b).ok());
    let bundle = wordcraft_formats::markdown::export_native(&markdown_ready(doc), &wordcraft_formats::markdown::NativeOptions::default());
    let info = textbundle::info_for_save(old.as_ref().map(|o| &o.info));
    let text_name = old.as_ref().map_or(textbundle::TEXT_NAME, |o| o.text_name.as_str());
    let mut files: Vec<(String, &[u8])> = bundle.assets.iter().map(|(p, b)| (p.clone(), b.as_slice())).collect();
    if let Some(o) = &old {
        files.extend(o.files.iter().map(|(p, b)| (p.clone(), b.as_slice())));
    }
    Ok((textbundle::pack(&info, text_name, bundle.text.as_bytes(), &files)?, bundle.report))
}

/// Save `doc` as a TextBundle folder: its text file (`text.md`, or the one the folder has), its
/// pictures under `assets/`, `info.json` with other applications' metadata kept. Nothing else in
/// the folder is touched.
#[cfg(not(target_arch = "wasm32"))]
fn save_textbundle(dir: &std::path::Path, doc: &Document) -> Result<wordcraft_formats::markdown::Report, String> {
    if dir.is_file() {
        return Err(format!("{}: a file is there, not a TextBundle folder", dir.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .take(10_000)
        .collect();
    let text_name = textbundle::pick_text(names.iter().map(String::as_str)).unwrap_or(textbundle::TEXT_NAME).to_string();
    let old = read_regular_file(&dir.join("info.json"), textbundle::MAX_INFO).map(|b| textbundle::parse_info(&b));
    let bundle = wordcraft_formats::markdown::export_native(&markdown_ready(doc), &wordcraft_formats::markdown::NativeOptions::default());
    for (rel, bytes) in &bundle.assets {
        let target = dir.join(rel);
        if target.is_file() {
            continue;
        }
        crate::file_access::check_write(&target.to_string_lossy())?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        safe_save::write(&target, bytes).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    let text = dir.join(&text_name);
    safe_save::write(&text, bundle.text.as_bytes()).map_err(|e| format!("{}: {e}", text.display()))?;
    let info = dir.join("info.json");
    safe_save::write(&info, &textbundle::info_bytes(&textbundle::info_for_save(old.as_ref()))).map_err(|e| format!("{}: {e}", info.display()))?;
    Ok(bundle.report)
}

/// Save `doc` as a TextPack at `path`, over the one there (see [`textpack_bytes`]).
#[cfg(not(target_arch = "wasm32"))]
fn save_textpack(path: &std::path::Path, doc: &Document) -> Result<wordcraft_formats::markdown::Report, String> {
    let existing = if path.is_file() { read_regular_file(path, 2 << 30) } else { None };
    let (bytes, report) = textpack_bytes(doc, existing.as_deref())?;
    safe_save::write(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(report)
}

/// Save `doc` as a Markdown-native file ([`wordcraft_formats::markdown::export_native`]): the
/// text at `path`, its pictures in an `assets` folder beside it (an asset already there with the
/// same name has the same bytes, so it isn't written again). Returns what Markdown couldn't hold.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_markdown(path: &std::path::Path, doc: &Document) -> Result<wordcraft_formats::markdown::Report, String> {
    save_markdown_files(path, doc).map(|(report, _)| report)
}

/// [`save_markdown`], also giving the asset files the Markdown references (what a version of the
/// document must hold besides the Markdown itself).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_markdown_files(path: &std::path::Path, doc: &Document) -> Result<(wordcraft_formats::markdown::Report, Vec<std::path::PathBuf>), String> {
    crate::file_access::check_write(&path.to_string_lossy())?;
    // TextBundle, TextPack and ZIP hold their pictures themselves.
    match ext_of(&path.to_string_lossy()).as_str() {
        "textbundle" => return save_textbundle(path, doc).map(|r| (r, Vec::new())),
        "textpack" => return save_textpack(path, doc).map(|r| (r, Vec::new())),
        "zip" => return save_zip(path, doc).map(|r| (r, Vec::new())),
        _ => {}
    }
    let doc = markdown_ready(doc);
    let bundle = wordcraft_formats::markdown::export_native(&doc, &wordcraft_formats::markdown::NativeOptions::default());
    let dir = path.parent().unwrap_or(std::path::Path::new(""));
    for (rel, bytes) in &bundle.assets {
        let target = dir.join(rel);
        if target.is_file() {
            continue;
        }
        crate::file_access::check_write(&target.to_string_lossy())?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        safe_save::write(&target, bytes).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    safe_save::write(path, bundle.text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    let assets = bundle.assets.iter().map(|(rel, _)| dir.join(rel)).collect();
    Ok((bundle.report, assets))
}

#[cfg(target_arch = "wasm32")]
pub fn save_path_with(path: &std::path::Path, _doc: &Document, _password: Option<&str>, _opts: &ExportOptions) -> Result<(), String> {
    Err(format!("{}: files are saved through the browser on the web", path.display()))
}

/// The download for a Markdown-native save in the browser: `name` and the Markdown when the
/// document has no pictures, else `<name>.zip` holding the Markdown and its `assets/` folder. A
/// ZIP is written over `archive`, the one the document was opened from, when there is one.
pub fn markdown_download(name: &str, doc: &Document, archive: Option<&[u8]>) -> Result<(String, Vec<u8>), String> {
    // A TextPack downloads as itself, a TextBundle (a folder) as the TextPack holding it.
    match ext_of(name).as_str() {
        "textpack" => return textpack_bytes(doc, None).map(|(b, _)| (name.to_string(), b)),
        "zip" => return zip_bytes(name, doc, archive).map(|(b, _)| (name.to_string(), b)),
        "textbundle" => {
            let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
            return textpack_bytes(doc, None).map(|(b, _)| (format!("{stem}.textpack"), b));
        }
        _ => {}
    }
    let doc = markdown_ready(doc);
    let bundle = wordcraft_formats::markdown::export_native(&doc, &wordcraft_formats::markdown::NativeOptions::default());
    if bundle.assets.is_empty() {
        return Ok((name.to_string(), bundle.text.into_bytes()));
    }
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let zip = wordcraft_formats::markdown::bundle_zip(file, &bundle)?;
    Ok((format!("{}.zip", name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name)), zip))
}

/// `doc` with its metafile pictures (WMF, EMF) drawn as PNG, for copies read on the web
/// (Markdown): browsers and Markdown renderers can't show metafiles. Borrowed when there are none.
/// Plume: the Markdown `doc` is saved as (pictures as the web shows them, Word properties in
/// the front matter, charts and SmartArt as diagrams).
pub fn markdown_text(doc: &Document) -> String {
    markdown_bundle(doc).text
}

/// [`markdown_text`] with the pictures it names (`assets/…`), not yet on disk perhaps.
pub fn markdown_bundle(doc: &Document) -> wordcraft_formats::markdown::Bundle {
    wordcraft_formats::markdown::export_native(&markdown_ready(doc), &wordcraft_formats::markdown::NativeOptions::default())
}

/// `doc` as its Markdown copy is written: pictures the web shows ([`web_pictures`]), and a
/// document converted from Word with its Word properties in the front matter
/// ([`wordcraft_formats::front_matter::adopt_office`]).
fn markdown_ready(doc: &Document) -> std::borrow::Cow<'_, Document> {
    let doc = web_pictures(doc);
    if doc.passthrough.contains_key(wordcraft_formats::model::FRONT_MATTER_PART) {
        return doc;
    }
    let mut doc = doc.into_owned();
    wordcraft_formats::front_matter::adopt_office(&mut doc);
    std::borrow::Cow::Owned(doc)
}

pub fn web_pictures(doc: &Document) -> std::borrow::Cow<'_, Document> {
    let metafiles: Vec<(String, f32, f32)> =
        doc.media.iter().filter_map(|(k, b)| wordcraft_render::metafile_size(b).map(|(w, h, _)| (k.clone(), w, h))).collect();
    let graphics = has_graphics(&doc.body, 0);
    if metafiles.is_empty() && !graphics {
        return std::borrow::Cow::Borrowed(doc);
    }
    let mut out = doc.clone();
    if graphics {
        // Charts, SmartArt and diagrams: drawn as they show on the page, kept as pictures.
        let mut drawn = 0usize;
        let mut body = std::mem::take(&mut out.body);
        for b in &mut body {
            graphics_as_pictures(doc, &mut out, std::sync::Arc::make_mut(b), &mut drawn, 0);
        }
        out.body = body;
    }
    for (key, w, h) in metafiles {
        // Two pixels per point, at most 2048 pixels on the longer side.
        let scale = (2048.0 / w.max(h).max(1.0)).min(2.0);
        let item = wordcraft_layout::display::Draw::Image {
            rect: wordcraft_geom::Rect { x: 0.0, y: 0.0, w, h },
            media: key.clone(),
            crop: [0.0; 4],
            alpha: 1.0,
            alt: String::new(),
        };
        let png = wordcraft_render::render_items(doc, &[item], 0.0, 0.0, w, h, scale, &Default::default()).to_png();
        if !png.is_empty() {
            out.media.insert(key, std::sync::Arc::new(png));
        }
    }
    std::borrow::Cow::Owned(out)
}

/// Most charts and diagrams drawn as pictures for one copy.
const MAX_GRAPHICS: usize = 200;

fn has_graphics(blocks: &wordcraft_doc::Blocks, depth: usize) -> bool {
    depth < 8
        && blocks.iter().any(|b| match &**b {
            wordcraft_doc::Block::Para(p) => p.objects.iter().any(|o| match o {
                wordcraft_doc::InlineObject::Graphic { .. } => true,
                // A drawing of shapes joined by connectors (Plume: becomes a Mermaid flowchart).
                wordcraft_doc::InlineObject::Group { children, .. } => children
                    .iter()
                    .any(|c| matches!(&c.obj, wordcraft_doc::InlineObject::Shape { extra, .. } if extra.start.is_some() && extra.end.is_some())),
                _ => false,
            }),
            wordcraft_doc::Block::Table(t) => t.rows.iter().flat_map(|r| &r.cells).any(|c| has_graphics(&c.blocks, depth + 1)),
        })
}

/// Replace the graphics in `b` (charts, SmartArt…) by pictures of them, drawn from `src`.
fn graphics_as_pictures(src: &Document, out: &mut Document, b: &mut wordcraft_doc::Block, drawn: &mut usize, depth: usize) {
    match b {
        wordcraft_doc::Block::Para(p) => {
            // Plume: a chart or SmartArt graphic alone in its paragraph becomes a diagram in
            // text (Mermaid), drawn; the Markdown gets its source.
            if *drawn < MAX_GRAPHICS
                && let Some((source, pic)) = graphic_as_mermaid(src, p)
            {
                *drawn += 1;
                let media = out.add_media(pic.png.to_vec(), "png");
                let k = if pic.width > 450.0 { 450.0 / pic.width } else { 1.0 };
                for o in &mut p.objects {
                    let alt = match o {
                        wordcraft_doc::InlineObject::Graphic { alt, .. } => Some(alt.clone()),
                        wordcraft_doc::InlineObject::Group { .. } => Some(String::new()),
                        _ => None,
                    };
                    if let Some(alt) = alt {
                        *o = wordcraft_doc::InlineObject::Image {
                            media: media.clone(),
                            w: pic.width * k,
                            h: pic.height * k,
                            alt,
                            float: Default::default(),
                            crop: [0.0; 4],
                            ole: None,
                        };
                    }
                }
                let md = p.props.md.get_or_insert_with(Default::default);
                md.code = Some(wordcraft_doc::props::MdCode { id: 0, info: "mermaid".into() });
                md.diagram = Some(source);
                p.touch();
                return;
            }
            let mut changed = false;
            for o in &mut p.objects {
                let wordcraft_doc::InlineObject::Graphic { w, h, alt, .. } = o else { continue };
                if *drawn >= MAX_GRAPHICS || !(w.is_finite() && h.is_finite() && *w >= 1.0 && *h >= 1.0) {
                    continue;
                }
                let (w, h, alt) = (w.min(1584.0), h.min(1584.0), alt.clone());
                let Some(png) = draw_object(src, o, w, h) else { continue };
                *drawn += 1;
                let media = out.add_media(png, "png");
                *o = wordcraft_doc::InlineObject::Image { media, w, h, alt, float: Default::default(), crop: [0.0; 4], ole: None };
                changed = true;
            }
            if changed {
                p.touch();
            }
        }
        wordcraft_doc::Block::Table(t) if depth < 8 => {
            for c in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                for cb in &mut c.blocks {
                    graphics_as_pictures(src, out, std::sync::Arc::make_mut(cb), drawn, depth + 1);
                }
            }
        }
        wordcraft_doc::Block::Table(_) => {}
    }
}

/// Plume: a paragraph holding one chart, SmartArt graphic or drawing of shapes joined by
/// connectors and no text, as Mermaid source and its picture. The graphic's model when Plume
/// made it, else what its parts say.
fn graphic_as_mermaid(src: &Document, p: &wordcraft_doc::Paragraph) -> Option<(String, wordcraft_diagram::Picture)> {
    if !p.plain_text().trim().is_empty() || p.objects.iter().filter(|o| !o.is_marker()).count() != 1 {
        return None;
    }
    if let Some(children) = p.objects.iter().find_map(|o| match o {
        wordcraft_doc::InlineObject::Group { children, .. } => Some(children),
        _ => None,
    }) {
        let text = |id: u32| src.plain_text(wordcraft_doc::StoryRef::Part(id)).split_whitespace().collect::<Vec<_>>().join(" ");
        let source = wordcraft_diagram::sketch::Sketch::from_drawing(children, &text)?.to_mermaid();
        let pic = wordcraft_diagram::draw(&source).ok()?;
        return Some((source, pic));
    }
    let g = p.objects.iter().find_map(|o| match o {
        wordcraft_doc::InlineObject::Graphic { graphic, .. } => Some(graphic.clone()),
        _ => None,
    })?;
    let part = |kind: &str| {
        g.source.as_ref()?.refs.iter().find(|r| !r.external && r.kind.ends_with(kind)).and_then(|r| src.passthrough.get(&r.target)).cloned()
    };
    let source = if let Some(spec) = &g.smart_art {
        wordcraft_diagram::mermaid::from_smart_art(spec)?
    } else if let Some(spec) = &g.chart {
        wordcraft_diagram::mermaid::from_chart(spec)?
    } else if let Some(data) = part("/diagramData") {
        wordcraft_diagram::mermaid::from_smart_art(&wordcraft_docx::smart_art_outline(&data)?)?
    } else {
        wordcraft_diagram::mermaid::from_chart(&wordcraft_docx::chart_outline(&part("/chart")?)?)?
    };
    let pic = wordcraft_diagram::draw(&source).ok()?;
    Some((source, pic))
}

/// A PNG of one inline object `w`×`h` points: laid out alone, inline, on a page its size.
fn draw_object(src: &Document, obj: &wordcraft_doc::InlineObject, w: f32, h: f32) -> Option<Vec<u8>> {
    const M: f32 = 18.0;
    let mut obj = obj.clone();
    if let wordcraft_doc::InlineObject::Graphic { float, .. } = &mut obj {
        *float = Default::default();
    }
    let mut tmp = src.clone();
    tmp.parts.retain(|_, p| !matches!(p.kind, wordcraft_doc::PartKind::Header | wordcraft_doc::PartKind::Footer));
    let mut para = wordcraft_doc::Paragraph::new();
    para.props.space_before = Some(0.0);
    para.props.space_after = Some(0.0);
    para.props.line_spacing = Some(Default::default());
    para.insert_object(0, obj, &Default::default()).ok()?;
    tmp.body = vec![wordcraft_doc::para_block(para)];
    tmp.last_section = wordcraft_doc::SectionProps {
        page_w: w + 2.0 * M,
        page_h: h + 2.0 * M + 24.0,
        margin_top: M,
        margin_bottom: M,
        margin_left: M,
        margin_right: M,
        header: 0.0,
        footer: 0.0,
        ..Default::default()
    };
    let l = wordcraft_layout::layout(&tmp, &mut wordcraft_layout::LayoutCache::new(), &Default::default());
    let page = l.pages.first()?;
    let scale = (2048.0 / w.max(h).max(1.0)).min(2.0);
    let png = wordcraft_render::render_area(&tmp, page, M, M, w, h, scale, &Default::default()).to_png();
    (!png.is_empty()).then_some(png)
}

/// Most documents listed from one folder tree.
pub const MAX_DOCUMENTS: usize = 2_000;

/// The Markdown documents under `root` (folders in name order, at most a few levels deep;
/// hidden folders, `assets` and build folders skipped), as paths relative to `root`.
#[cfg(not(target_arch = "wasm32"))]
pub fn markdown_documents(root: &std::path::Path) -> Result<Vec<String>, String> {
    crate::file_access::check_read(&root.to_string_lossy())?;
    fn walk(dir: &std::path::Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        if depth > 6 || out.len() >= MAX_DOCUMENTS {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<(String, bool)> =
            rd.filter_map(Result::ok).filter_map(|e| Some((e.file_name().into_string().ok()?, e.file_type().ok()?.is_dir()))).collect();
        entries.sort();
        for (name, is_dir) in entries {
            if out.len() >= MAX_DOCUMENTS {
                return;
            }
            let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if is_dir {
                if !name.starts_with('.') && !matches!(name.as_str(), "assets" | "node_modules" | "target" | "dist" | "build") {
                    walk(&dir.join(&name), &path, depth + 1, out);
                }
            } else if name.rsplit_once('.').is_some_and(|(_, e)| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown")) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, "", 0, &mut out);
    Ok(out)
}

/// Decode text bytes: UTF-8 (with or without BOM), UTF-16 LE/BE (BOM or sniffed), else the
/// legacy encoding they look like — GBK, Big5, Shift_JIS, Windows-1252… (see
/// [`wordcraft_formats::txt::decode`]).
pub fn decode_text(b: &[u8]) -> String {
    wordcraft_formats::txt::decode(b)
}
