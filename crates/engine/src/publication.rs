//! Publication profiles (Plume): how a folder's Markdown documents are laid out on paper —
//! page size, orientation, margins, fonts, and the look they're edited in — kept beside them in
//! `publication/default.toml`, never in the Markdown itself:
//!
//! ```toml
//! theme = "gfm"            # or "vitepress": how documents look while edited
//!
//! [page]
//! format = "A4"            # A3, A4, A5, Letter, Legal
//! orientation = "portrait" # or "landscape"
//!
//! [margins]                # millimetres
//! top = 20
//! bottom = 20
//! left = 25
//! right = 25
//!
//! [typography]
//! body_font = "Liberation Serif"
//! body_size = 11           # points
//! heading_font = "Liberation Sans"
//! ```
//!
//! The reader takes this small TOML subset (sections, `key = value` with strings, numbers and
//! booleans, `#` comments) and ignores anything else, so a hand-edited file never stops a
//! document from opening.

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use wordcraft_doc::Document;

/// Where a folder's profile lives, relative to the folder.
pub const PROFILE_PATH: &str = "publication/default.toml";
/// Largest profile read.
#[cfg(not(target_arch = "wasm32"))]
const MAX_PROFILE: usize = 64 * 1024;
/// How many folders up from a document the profile is looked for.
#[cfg(not(target_arch = "wasm32"))]
const MAX_LEVELS: usize = 8;

/// A publication profile. `None` fields leave the document as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub theme: Option<wordcraft_formats::markdown::Theme>,
    /// Page width and height, points (portrait).
    pub page: Option<(f32, f32)>,
    pub landscape: Option<bool>,
    /// Top, bottom, left, right, points.
    pub margins: [Option<f32>; 4],
    pub body_font: Option<String>,
    pub body_size: Option<f32>,
    pub heading_font: Option<String>,
}

fn unquote(v: &str) -> Option<String> {
    let v = v.trim();
    let inner = v.strip_prefix('"').and_then(|r| r.strip_suffix('"')).or_else(|| v.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))?;
    let s: String = inner.replace("\\\"", "\"").replace("\\\\", "\\").chars().filter(|c| !c.is_control()).take(200).collect();
    Some(s)
}

fn number(v: &str) -> Option<f32> {
    v.trim().replace('_', "").parse::<f32>().ok().filter(|n| n.is_finite())
}

/// Paper sizes by name, points (portrait).
fn paper(name: &str) -> Option<(f32, f32)> {
    let mm = |w: f32, h: f32| (w * 72.0 / 25.4, h * 72.0 / 25.4);
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "a3" => mm(297.0, 420.0),
        "a4" => mm(210.0, 297.0),
        "a5" => mm(148.0, 210.0),
        "letter" => (612.0, 792.0),
        "legal" => (612.0, 1008.0),
        _ => return None,
    })
}

impl Profile {
    /// Read a profile's text (the TOML subset above).
    pub fn parse(text: &str) -> Profile {
        let mut p = Profile::default();
        let mut section = String::new();
        for line in text.lines().take(2_000) {
            // A `#` outside quotes starts a comment.
            let mut in_str = false;
            let cut = line.char_indices().find(|(_, c)| {
                if *c == '"' {
                    in_str = !in_str;
                }
                *c == '#' && !in_str
            });
            let line = cut.map(|(k, _)| line.get(..k).unwrap_or("")).unwrap_or(line).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                section = name.trim().to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let key = key.trim().to_ascii_lowercase();
            let mm = |v: &str| number(v).map(|n| (n.clamp(0.0, 200.0)) * 72.0 / 25.4);
            match (section.as_str(), key.as_str()) {
                ("", "theme") => p.theme = unquote(value).and_then(|t| wordcraft_formats::markdown::Theme::from_name(&t)),
                ("page", "format") => p.page = unquote(value).and_then(|f| paper(&f)),
                ("page", "orientation") => p.landscape = unquote(value).map(|o| o.eq_ignore_ascii_case("landscape")),
                ("margins", "top") => p.margins[0] = mm(value),
                ("margins", "bottom") => p.margins[1] = mm(value),
                ("margins", "left") => p.margins[2] = mm(value),
                ("margins", "right") => p.margins[3] = mm(value),
                ("typography", "body_font") => p.body_font = unquote(value).filter(|f| !f.is_empty()),
                ("typography", "body_size") => p.body_size = number(value).map(|n| n.clamp(4.0, 72.0)),
                ("typography", "heading_font") => p.heading_font = unquote(value).filter(|f| !f.is_empty()),
                _ => {}
            }
        }
        p
    }

    /// The profile for a document at `path`: the nearest `publication/default.toml` in its folder
    /// or the folders above (at most a few levels, and not above a Git repository's root).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn for_document(path: &Path) -> Option<(Profile, PathBuf)> {
        let mut dir = path.parent().map(Path::to_path_buf)?;
        if dir.as_os_str().is_empty() {
            dir = PathBuf::from(".");
        }
        for _ in 0..MAX_LEVELS {
            let file = dir.join(PROFILE_PATH);
            // Through `io`, so the automation read root applies.
            if file.is_file()
                && let Ok(bytes) = crate::io::read_file_capped(&file.to_string_lossy(), MAX_PROFILE as u64, "publication profile")
            {
                return Some((Profile::parse(&String::from_utf8_lossy(&bytes)), file));
            }
            if dir.join(".git").exists() {
                break;
            }
            match dir.parent() {
                Some(p) if !p.as_os_str().is_empty() => dir = p.to_path_buf(),
                _ => break,
            }
        }
        None
    }

    /// Lay `doc` out as the profile says: theme, page, margins, fonts.
    pub fn apply(&self, doc: &mut Document) {
        if let Some(t) = self.theme {
            wordcraft_formats::markdown::apply_theme(doc, t);
        }
        let mut sections: Vec<&mut wordcraft_doc::SectionProps> = vec![&mut doc.last_section];
        let mut inner: Vec<&mut wordcraft_doc::SectionProps> = Vec::new();
        for b in &mut doc.body {
            if let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(b)
                && let Some(sp) = p.section.as_deref_mut()
            {
                inner.push(sp);
            }
        }
        sections.extend(inner);
        for sp in sections {
            if let Some((w, h)) = self.page {
                let landscape = self.landscape.unwrap_or(sp.page_w > sp.page_h);
                (sp.page_w, sp.page_h) = if landscape { (h, w) } else { (w, h) };
            } else if let Some(l) = self.landscape
                && l != (sp.page_w > sp.page_h)
            {
                std::mem::swap(&mut sp.page_w, &mut sp.page_h);
            }
            let [t, b, l, r] = self.margins;
            sp.margin_top = t.unwrap_or(sp.margin_top);
            sp.margin_bottom = b.unwrap_or(sp.margin_bottom);
            sp.margin_left = l.unwrap_or(sp.margin_left);
            sp.margin_right = r.unwrap_or(sp.margin_right);
        }
        let st = &mut doc.styles;
        if let Some(f) = &self.body_font {
            st.default_chr.font = Some(f.clone());
            for s in st.styles.iter_mut().filter(|s| s.id == "Normal" || s.id == "Quote") {
                s.chr.font = None;
            }
        }
        if let Some(size) = self.body_size {
            st.default_chr.size = Some(size);
        }
        if let Some(f) = &self.heading_font {
            for s in st.styles.iter_mut().filter(|s| s.id.starts_with("Heading") || s.id == "Title") {
                s.chr.font = Some(f.clone());
            }
        }
    }
}
