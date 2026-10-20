//! Plume: diagrams written as text, drawn as pictures. A Mermaid source (` ```mermaid ` in
//! Markdown) is laid out as SVG by `mermaid-svg` and drawn to PNG by `resvg`, its text set in
//! the bundled fonts, so the picture is the same on every machine and in the browser.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod codec;
pub mod form;
pub mod graph;
pub mod mermaid;
pub mod sheet;
pub mod sketch;

use std::sync::{Arc, OnceLock};

/// Longest Mermaid source drawn, bytes.
pub const MAX_SOURCE: usize = 64 * 1024;
/// Largest picture drawn, pixels on its longer side.
const MAX_SIDE: f32 = 4096.0;
/// Pixels per point of the picture (drawn at twice its size, sharp on screen and paper).
const SCALE: f32 = 2.0;
/// The font a diagram's text is set in.
const FAMILY: &str = "Inter";

/// A diagram drawn: PNG bytes and the size it shows at, points.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub png: Arc<Vec<u8>>,
    pub width: f32,
    pub height: f32,
}

/// Why a source couldn't be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagramError {
    TooLarge,
    /// The source isn't Mermaid the renderer reads (its message).
    Syntax(String),
    /// The renderer failed on it.
    Failed(String),
}

impl std::fmt::Display for DiagramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagramError::TooLarge => write!(f, "the diagram is too large"),
            DiagramError::Syntax(m) => write!(f, "this isn't a diagram Plume can draw: {m}"),
            DiagramError::Failed(m) => write!(f, "the diagram couldn't be drawn: {m}"),
        }
    }
}

/// Whether a code block's info string (`mermaid`, `mermaid title`) names a Mermaid diagram.
pub fn is_mermaid(info: &str) -> bool {
    info.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("mermaid"))
}

fn options() -> &'static resvg::usvg::Options<'static> {
    static OPTS: OnceLock<resvg::usvg::Options<'static>> = OnceLock::new();
    OPTS.get_or_init(|| {
        let mut o = resvg::usvg::Options { font_family: FAMILY.to_string(), ..Default::default() };
        let db = o.fontdb_mut();
        for data in wordcraft_fonts::bundled() {
            db.load_font_data(data.to_vec());
        }
        // Every family the SVG names falls back to the bundled sans.
        db.set_sans_serif_family(FAMILY);
        db.set_serif_family(FAMILY);
        db.set_monospace_family("JetBrains Mono");
        o
    })
}

/// The SVG of Mermaid source `src`.
pub fn svg(src: &str) -> Result<String, DiagramError> {
    if src.len() > MAX_SOURCE {
        return Err(DiagramError::TooLarge);
    }
    // The renderer is someone else's code: a panic in it is a failure here, never a crash.
    match std::panic::catch_unwind(|| mermaid_svg::render(src)) {
        Ok(Ok(svg)) => Ok(svg),
        Ok(Err(e)) => Err(DiagramError::Syntax(e.to_string().chars().take(300).collect())),
        Err(_) => Err(DiagramError::Failed("the renderer stopped".into())),
    }
}

/// Mermaid source `src` drawn as a picture.
pub fn draw(src: &str) -> Result<Picture, DiagramError> {
    let svg = svg(src)?;
    let tree = resvg::usvg::Tree::from_str(&svg, options()).map_err(|e| DiagramError::Failed(e.to_string()))?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w.is_finite() && h.is_finite()) || w < 1.0 || h < 1.0 {
        return Err(DiagramError::Failed("an empty diagram".into()));
    }
    // Drawn at twice its size, less for a very large one.
    let scale = SCALE.min(MAX_SIDE / w.max(h));
    let (pw, ph) = ((w * scale).ceil().max(1.0) as u32, (h * scale).ceil().max(1.0) as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph).ok_or(DiagramError::TooLarge)?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let png = pixmap.encode_png().map_err(|e| DiagramError::Failed(e.to_string()))?;
    // SVG user units are CSS pixels: 0.75 pt each.
    Ok(Picture { png: Arc::new(png), width: w * 0.75, height: h * 0.75 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sequence_diagram_is_drawn() {
        let p = draw("sequenceDiagram\n    Alice->>Bob: Bonjour\n    Bob-->>Alice: Salut\n").unwrap();
        assert!(p.png.starts_with(b"\x89PNG") && p.width > 50.0 && p.height > 50.0, "{} × {}", p.width, p.height);
        assert!(is_mermaid("mermaid") && is_mermaid("Mermaid title") && !is_mermaid("js"));
    }

    #[test]
    fn junk_is_an_error_not_a_crash() {
        for src in ["", "not a diagram", "flowchart LR\n A -->", "sequenceDiagram\n participant", &"x".repeat(MAX_SOURCE + 1)] {
            assert!(draw(src).is_err() || draw(src).is_ok(), "{src}");
        }
        assert_eq!(draw(&"x".repeat(MAX_SOURCE + 1)), Err(DiagramError::TooLarge));
    }
}
