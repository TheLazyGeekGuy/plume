//! Plume: TextBundle and TextPack (textbundle.org, version 2). A TextBundle is a folder
//! (`name.textbundle/`) holding the text (`text.md`, any `text.*`), its pictures under `assets/`,
//! and `info.json`, its metadata. A TextPack (`name.textpack`) is the same in one ZIP file.
//!
//! Plume reads the Markdown and its pictures and writes them back the same way; the folder or
//! archive keeps everything else it held. As the format asks, other applications' metadata in
//! `info.json` (their own keys, the creator) is kept when Plume saves over a bundle; only the
//! version and the text's type are set. Hostile archives are bounded: entry count, each entry's
//! size and the total are capped, and paths climbing out of the bundle are ignored.

use std::io::Read;

use serde_json::{Map, Value, json};

/// The text's type a bundle declares by default (version 1 bundles have none).
pub const MARKDOWN_TYPE: &str = "net.daringfireball.markdown";
/// The text file Plume writes in a new bundle.
pub const TEXT_NAME: &str = "text.md";
/// Largest text file read.
pub(crate) const MAX_TEXT: u64 = 64 << 20;
/// Largest other entry read, and all entries together.
pub(crate) const MAX_ENTRY: u64 = 256 << 20;
pub(crate) const MAX_TOTAL: u64 = 1 << 30;
/// Most entries read from a TextPack.
pub(crate) const MAX_ENTRIES: usize = 10_000;
/// Largest `info.json` read.
pub const MAX_INFO: u64 = 1 << 20;

/// A bundle's contents: its metadata, its text, and its other files by path inside the bundle
/// (`assets/a.png`, other applications' files).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unpacked {
    pub info: Map<String, Value>,
    /// The text file's name (`text.md`, `text.markdown`…).
    pub text_name: String,
    pub text: Vec<u8>,
    pub files: Vec<(String, Vec<u8>)>,
}

impl Unpacked {
    /// A file of the bundle by path (`assets/a.png`, `./assets/a.png`, percent-encoded or not).
    pub fn file(&self, src: &str) -> Option<&[u8]> {
        let src = src.split(['#', '?']).next().unwrap_or("");
        let decoded = String::from_utf8(wordcraft_formats::model::percent_decode(src)).ok();
        // The name as written, else decoded (`b%20c.png` names `b c.png`).
        [clean(src), decoded.as_deref().and_then(clean)]
            .into_iter()
            .flatten()
            .find_map(|want| self.files.iter().find(|(p, _)| *p == want).map(|(_, b)| b.as_slice()))
    }

    /// Whether the text is Markdown (the type the bundle declares, Markdown by default).
    pub fn is_markdown(&self) -> bool {
        is_markdown_type(&self.info)
    }
}

/// Whether `info` declares Markdown text (or nothing: version 1 bundles are Markdown).
pub fn is_markdown_type(info: &Map<String, Value>) -> bool {
    match info.get("type").and_then(Value::as_str) {
        None => true,
        Some(t) => {
            let t = t.to_ascii_lowercase();
            t == MARKDOWN_TYPE || t.contains("markdown") || t.ends_with(".md")
        }
    }
}

/// Whether `name` is a bundle's text file: `text.<ext>`, any case.
pub fn is_text_name(name: &str) -> bool {
    name.len() > 5 && name.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("text.")) && !name.contains('/')
}

/// The text file among `names`: `text.md`, then `text.markdown`, then any other `text.*`.
pub fn pick_text<'a>(names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let all: Vec<&str> = names.filter(|n| is_text_name(n)).collect();
    for want in ["text.md", "text.markdown"] {
        if let Some(n) = all.iter().find(|n| n.eq_ignore_ascii_case(want)) {
            return Some(n);
        }
    }
    let mut rest = all;
    rest.sort_unstable();
    rest.first().copied()
}

/// A path inside the bundle, made relative and plain (`./a/../b` → `b`); `None` when it climbs
/// out of the bundle or is absolute.
pub(crate) fn clean(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if path.starts_with('/') || path.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The bundle inside a TextPack: at the root of the archive, or in its one top-level folder
/// (`name.textbundle/`), as applications write them either way.
fn root_of(names: &[String]) -> String {
    if names.iter().any(|n| n == "info.json" || is_text_name(n)) {
        return String::new();
    }
    let mut tops: Vec<&str> =
        names.iter().filter_map(|n| n.split_once('/').map(|(top, _)| top)).filter(|t| !t.is_empty() && *t != "__MACOSX").collect();
    tops.sort_unstable();
    tops.dedup();
    match tops.as_slice() {
        [one] => format!("{one}/"),
        _ => tops
            .iter()
            .find(|t| names.iter().any(|n| n.strip_prefix(&format!("{t}/")).is_some_and(|rest| rest == "info.json" || is_text_name(rest))))
            .map(|t| format!("{t}/"))
            .unwrap_or_default(),
    }
}

/// Read a TextPack.
pub fn unpack(bytes: &[u8]) -> Result<Unpacked, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("not a TextPack (ZIP) file: {e}"))?;
    if zip.len() > MAX_ENTRIES {
        return Err(format!("the TextPack holds more than {MAX_ENTRIES} files"));
    }
    let names: Vec<String> = (0..zip.len()).filter_map(|i| zip.by_index_raw(i).ok().map(|f| f.name().replace('\\', "/"))).collect();
    let root = root_of(&names);
    let text_name =
        pick_text(names.iter().filter_map(|n| n.strip_prefix(root.as_str()))).map(str::to_string).ok_or("the TextPack has no text file (text.md)")?;
    let mut out = Unpacked { text_name: text_name.clone(), ..Default::default() };
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        let Some(rel) = name.strip_prefix(root.as_str()).and_then(clean) else { continue };
        if rel.starts_with("__MACOSX/") || rel.ends_with(".DS_Store") {
            continue;
        }
        let limit = if rel == text_name {
            MAX_TEXT
        } else if rel == "info.json" {
            MAX_INFO
        } else {
            MAX_ENTRY
        };
        let limit = limit.min(MAX_TOTAL.saturating_sub(total));
        let mut data = Vec::new();
        // Bounded whatever size the archive claims.
        (&mut f).take(limit + 1).read_to_end(&mut data).map_err(|e| format!("{rel}: {e}"))?;
        if data.len() as u64 > limit {
            return Err(format!("{rel} is too large"));
        }
        total += data.len() as u64;
        if rel == text_name {
            out.text = data;
        } else if rel == "info.json" {
            out.info = parse_info(&data);
        } else {
            out.files.push((rel, data));
        }
    }
    Ok(out)
}

/// `info.json`'s keys; an unreadable file reads as none (the bundle still opens).
pub fn parse_info(bytes: &[u8]) -> Map<String, Value> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

/// The `info.json` written over `existing`: what other applications put there is kept (their
/// keys, the creator, the source); the version is 2 and the text Markdown. A new bundle says
/// Plume created it.
pub fn info_for_save(existing: Option<&Map<String, Value>>) -> Map<String, Value> {
    let mut m = existing.cloned().unwrap_or_default();
    m.insert("version".into(), json!(2));
    m.insert("type".into(), json!(MARKDOWN_TYPE));
    m.entry("transient").or_insert(json!(false));
    if !m.contains_key("creatorIdentifier") {
        m.insert("creatorIdentifier".into(), json!(wordcraft_brand::APP_ID));
    }
    m
}

/// `info.json`'s text.
pub fn info_bytes(info: &Map<String, Value>) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(&Value::Object(info.clone())).unwrap_or_else(|_| b"{\"version\": 2}".to_vec());
    b.push(b'\n');
    b
}

/// A TextPack: `info.json`, the text and the files, at the root of the archive. Text and
/// metadata are compressed; pictures, compressed already, are stored.
pub fn pack(info: &Map<String, Value>, text_name: &str, text: &[u8], files: &[(String, &[u8])]) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let err = |e: zip::result::ZipError| format!("zip: {e}");
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    z.start_file("info.json", deflate).map_err(err)?;
    z.write_all(&info_bytes(info)).map_err(|e| e.to_string())?;
    z.start_file(text_name, deflate).map_err(err)?;
    z.write_all(text).map_err(|e| e.to_string())?;
    let mut seen: Vec<String> = vec!["info.json".into(), text_name.to_string()];
    for (path, bytes) in files {
        let Some(p) = clean(path) else { continue };
        if seen.iter().any(|s| s.eq_ignore_ascii_case(&p)) {
            continue;
        }
        let picture = p
            .rsplit_once('.')
            .is_some_and(|(_, e)| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "heic"));
        z.start_file(p.as_str(), if picture { stored } else { deflate }).map_err(err)?;
        z.write_all(bytes).map_err(|e| e.to_string())?;
        seen.push(p);
    }
    Ok(z.finish().map_err(err)?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_reads_back_at_the_root_or_in_its_folder() {
        let info = info_for_save(None);
        assert_eq!(info["creatorIdentifier"], wordcraft_brand::APP_ID);
        let bytes = pack(
            &info,
            TEXT_NAME,
            b"# Hi\n\n![](assets/a.png)\n",
            &[("assets/a.png".into(), b"PNG".as_slice()), ("../evil".into(), b"x".as_slice())],
        )
        .unwrap();
        let u = unpack(&bytes).unwrap();
        assert_eq!((u.text_name.as_str(), u.text.as_slice()), (TEXT_NAME, b"# Hi\n\n![](assets/a.png)\n".as_slice()));
        assert_eq!(u.file("./assets/a.png"), Some(b"PNG".as_slice()));
        assert_eq!(u.file("assets/../../evil"), None);
        assert!(u.is_markdown());

        // A pack written by another application: the bundle in a folder, its own metadata.
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("Note.textbundle/info.json", o).unwrap();
        z.write_all(br#"{"version":2,"creatorIdentifier":"net.shinyfrog.bear","net.shinyfrog.bear":{"version":1}}"#).unwrap();
        z.start_file("Note.textbundle/text.markdown", o).unwrap();
        z.write_all(b"Body").unwrap();
        z.start_file("Note.textbundle/assets/b%20c.jpg", o).unwrap();
        z.write_all(b"JPG").unwrap();
        let other = unpack(&z.finish().unwrap().into_inner()).unwrap();
        assert_eq!((other.text_name.as_str(), other.text.as_slice()), ("text.markdown", b"Body".as_slice()));
        assert_eq!(other.file("assets/b%20c.jpg"), Some(b"JPG".as_slice()));
        let kept = info_for_save(Some(&other.info));
        assert_eq!(kept["creatorIdentifier"], "net.shinyfrog.bear");
        assert_eq!(kept["net.shinyfrog.bear"]["version"], 1);
        assert_eq!(kept["type"], MARKDOWN_TYPE);
    }

    #[test]
    fn junk_is_an_error_not_a_crash() {
        assert!(unpack(b"not a zip").is_err());
        assert!(unpack(&pack(&Map::new(), "notes.txt", b"x", &[]).unwrap()).is_err(), "no text.* file");
        assert!(parse_info(b"[1,2]").is_empty());
        assert!(!is_text_name("text.") && is_text_name("TEXT.MD") && !is_text_name("a/text.md"));
        let mut plain = Map::new();
        plain.insert("type".into(), json!("public.plain-text"));
        assert!(!is_markdown_type(&plain));
    }
}
