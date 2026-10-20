//! Plume: a Markdown document in an ordinary ZIP file, the way people send one: a Markdown file
//! under its own name (at the root or in a folder) and its pictures at the paths it names them
//! by (`images/photo.jpg`), beside whatever else the archive holds.
//!
//! Plume reads the Markdown and the pictures it names, and saving writes the archive again: the
//! Markdown in place under its name, new pictures beside it, and every other file copied as it
//! was (not even recompressed). Hostile archives are bounded as TextPacks are: entry count, each
//! entry's size and the total are capped, and paths climbing out of the archive are ignored.

use std::io::{Read, Write};

use super::textbundle::{MAX_ENTRIES, MAX_ENTRY, MAX_TEXT, MAX_TOTAL, clean};

/// A Markdown ZIP's contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unzipped {
    /// The Markdown file's path in the archive (`report.md`, `project/guide.md`).
    pub text_path: String,
    pub text: Vec<u8>,
    /// The other files, by path in the archive.
    pub files: Vec<(String, Vec<u8>)>,
}

impl Unzipped {
    /// A file the Markdown names (`images/a.png`, `./images/a%20b.png`, `../logo.png`), relative to
    /// the Markdown file's folder and within the archive.
    pub fn file(&self, src: &str) -> Option<&[u8]> {
        if src.contains("://") {
            return None;
        }
        let src = src.split(['#', '?']).next().unwrap_or("");
        let decoded = String::from_utf8(wordcraft_formats::model::percent_decode(src)).ok();
        let dir = folder(&self.text_path);
        [Some(src.to_string()), decoded].into_iter().flatten().find_map(|rel| {
            let want = clean(&format!("{dir}{rel}"))?;
            self.files.iter().find(|(p, _)| *p == want).map(|(_, b)| b.as_slice())
        })
    }
}

/// The folder of a path in the archive, with its slash (`docs/` for `docs/a.md`, `` at the root).
fn folder(path: &str) -> &str {
    path.rfind('/').and_then(|i| path.get(..=i)).unwrap_or("")
}

fn is_markdown_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".md") || l.ends_with(".markdown")
}

/// Files a ZIP made on macOS or Windows carries that are nobody's document.
fn is_junk(path: &str) -> bool {
    path.starts_with("__MACOSX/") || path.split('/').any(|s| s == ".DS_Store" || s == "Thumbs.db")
}

/// The archive's document among `names`: the Markdown file nearest the root, a `README` or
/// `index` first among equals, then by name.
pub fn pick_text<'a>(names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    names.filter(|n| is_markdown_name(n) && !is_junk(n)).min_by_key(|n| {
        let file = n.rsplit('/').next().unwrap_or(n).to_ascii_lowercase();
        let index = file.starts_with("readme.") || file.starts_with("index.");
        (n.matches('/').count(), !index, n.to_ascii_lowercase())
    })
}

/// The cleaned names of an archive's files, by index (`None` for folders and unusable names).
fn names<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>) -> Vec<Option<String>> {
    (0..zip.len()).map(|i| zip.by_index_raw(i).ok().filter(|f| !f.is_dir()).and_then(|f| clean(&f.name().replace('\\', "/")))).collect()
}

/// Read a Markdown ZIP.
pub fn unpack(bytes: &[u8]) -> Result<Unzipped, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("not a ZIP file: {e}"))?;
    if zip.len() > MAX_ENTRIES {
        return Err(format!("the ZIP holds more than {MAX_ENTRIES} files"));
    }
    let names = names(&mut zip);
    let text_path = pick_text(names.iter().flatten().map(String::as_str)).map(str::to_string).ok_or("the ZIP holds no Markdown file (.md)")?;
    let mut out = Unzipped { text_path: text_path.clone(), ..Default::default() };
    let mut total = 0u64;
    for (i, name) in names.iter().enumerate() {
        let Some(name) = name.as_ref().filter(|n| !is_junk(n)) else { continue };
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let limit = if *name == text_path { MAX_TEXT } else { MAX_ENTRY }.min(MAX_TOTAL.saturating_sub(total));
        let mut data = Vec::new();
        // Bounded whatever size the archive claims.
        (&mut f).take(limit + 1).read_to_end(&mut data).map_err(|e| format!("{name}: {e}"))?;
        if data.len() as u64 > limit {
            return Err(format!("{name} is too large"));
        }
        total += data.len() as u64;
        if *name == text_path {
            out.text = data;
        } else {
            out.files.push((name.clone(), data));
        }
    }
    Ok(out)
}

fn is_picture(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, e)| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "heic"))
}

/// The archive written again with `text` as its Markdown and `assets` (paths relative to the
/// Markdown file) beside it. Over `existing`, the Markdown keeps its path, a picture already there
/// is left as it is, and every other file is copied untouched; a new archive holds `new_name`.
pub fn pack(existing: Option<&[u8]>, new_name: &str, text: &[u8], assets: &[(String, &[u8])]) -> Result<Vec<u8>, String> {
    let err = |e: zip::result::ZipError| format!("zip: {e}");
    let mut old = existing.and_then(|b| zip::ZipArchive::new(std::io::Cursor::new(b)).ok()).filter(|z| z.len() <= MAX_ENTRIES);
    let old_names = old.as_mut().map(names).unwrap_or_default();
    let text_path = pick_text(old_names.iter().flatten().map(String::as_str))
        .map(str::to_string)
        .or_else(|| clean(new_name))
        .ok_or_else(|| format!("{new_name}: not a file name"))?;
    let dir = folder(&text_path).to_string();
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let mut written: Vec<String> = Vec::new();
    let mut text_written = false;
    if let Some(zip) = old.as_mut() {
        for (i, name) in old_names.iter().enumerate() {
            match name {
                Some(n) if *n == text_path => {
                    z.start_file(n.as_str(), deflate).map_err(err)?;
                    z.write_all(text).map_err(|e| e.to_string())?;
                    text_written = true;
                }
                Some(n) if written.iter().any(|w| w.eq_ignore_ascii_case(n)) => continue,
                Some(_) => z.raw_copy_file(zip.by_index_raw(i).map_err(err)?).map_err(err)?,
                // A folder entry, kept so empty folders stay.
                None => match zip.by_index_raw(i) {
                    Ok(f) if f.is_dir() && clean(f.name()).is_some() => z.raw_copy_file(f).map_err(err)?,
                    _ => continue,
                },
            }
            if let Some(n) = name {
                written.push(n.clone());
            }
        }
    }
    if !text_written {
        z.start_file(text_path.as_str(), deflate).map_err(err)?;
        z.write_all(text).map_err(|e| e.to_string())?;
        written.push(text_path.clone());
    }
    for (rel, bytes) in assets {
        let Some(p) = clean(&format!("{dir}{rel}")) else { continue };
        if written.iter().any(|w| w.eq_ignore_ascii_case(&p)) {
            continue;
        }
        // Pictures are compressed already.
        z.start_file(p.as_str(), if is_picture(&p) { stored } else { deflate }).map_err(err)?;
        z.write_all(bytes).map_err(|e| e.to_string())?;
        written.push(p);
    }
    Ok(z.finish().map_err(err)?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, data) in entries {
            z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap().into_inner()
    }

    #[test]
    fn the_markdown_nearest_the_root_is_the_document() {
        let names = ["project/notes/b.md", "project/guide.md", "project/README.md", "__MACOSX/x.md", "project/a.png"];
        assert_eq!(pick_text(names.iter().copied()), Some("project/README.md"));
        assert_eq!(pick_text(["z.md", "a.markdown", "d/a.md"].into_iter()), Some("a.markdown"));
        assert_eq!(pick_text(["a.png", "b.txt"].into_iter()), None);
    }

    #[test]
    fn pictures_resolve_from_the_markdown_folder() {
        let bytes = zip_of(&[
            ("Report/report.md", b"# R\n\n![](images/my%20photo.png)\n"),
            ("Report/images/my photo.png", b"PNG"),
            ("Report/logo.png", b"LOGO"),
            ("../evil.png", b"x"),
        ]);
        let u = unpack(&bytes).unwrap();
        assert_eq!(u.text_path, "Report/report.md");
        assert_eq!(u.file("images/my%20photo.png"), Some(b"PNG".as_slice()));
        assert_eq!(u.file("./images/my photo.png?raw=1"), Some(b"PNG".as_slice()));
        assert_eq!(u.file("images/../logo.png"), Some(b"LOGO".as_slice()));
        assert_eq!(u.file("../../evil.png"), None);
        assert_eq!(u.file("https://example.com/a.png"), None);
    }

    /// Saving over an archive keeps its other files byte for byte and the Markdown's path.
    #[test]
    fn packing_over_an_archive_keeps_everything_else() {
        let old = zip_of(&[("Report/report.md", b"old"), ("Report/images/a.png", b"PNG"), ("Report/data.csv", b"1,2"), ("LICENSE", b"CC0")]);
        let new =
            pack(Some(&old), "ignored.md", b"new", &[("images/a.png".into(), b"PNG".as_slice()), ("assets/img-1.png".into(), b"NEW".as_slice())])
                .unwrap();
        let u = unpack(&new).unwrap();
        assert_eq!((u.text_path.as_str(), u.text.as_slice()), ("Report/report.md", b"new".as_slice()));
        let files: Vec<(&str, &[u8])> = u.files.iter().map(|(p, b)| (p.as_str(), b.as_slice())).collect();
        assert_eq!(
            files,
            vec![("Report/images/a.png", b"PNG".as_slice()), ("Report/data.csv", b"1,2"), ("LICENSE", b"CC0"), ("Report/assets/img-1.png", b"NEW")]
        );

        // A new archive: the Markdown under its name, pictures beside it.
        let fresh = unpack(&pack(None, "Notes.md", b"text", &[("assets/b.png".into(), b"B".as_slice())]).unwrap()).unwrap();
        assert_eq!(fresh.text_path, "Notes.md");
        assert_eq!(fresh.file("assets/b.png"), Some(b"B".as_slice()));
    }

    #[test]
    fn junk_is_an_error_not_a_crash() {
        assert!(unpack(b"not a zip").is_err());
        assert!(unpack(&zip_of(&[("a.txt", b"x")])).is_err(), "no Markdown file");
        assert!(pack(Some(b"junk"), "a.md", b"x", &[]).is_ok(), "an unreadable archive is replaced");
        assert!(pack(None, "../", b"x", &[]).is_err());
    }
}
