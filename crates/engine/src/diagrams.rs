//! Plume: diagrams written as text (` ```mermaid `), drawn by [`wordcraft_diagram`] for the
//! Markdown reader and the diagram commands.

/// The Markdown reader's diagram renderer: Mermaid sources, drawn.
fn draw(info: &str, source: &str) -> Option<(std::sync::Arc<Vec<u8>>, f32, f32)> {
    if !wordcraft_diagram::is_mermaid(info) {
        return None;
    }
    wordcraft_diagram::draw(source).ok().map(|p| (p.png, p.width, p.height))
}

/// Let the Markdown reader draw diagrams (once; later calls do nothing).
pub fn register() {
    wordcraft_formats::markdown::set_diagram_renderer(draw);
}
