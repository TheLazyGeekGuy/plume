//! Plume's Markdown profile: Plume edits Markdown, so what Markdown can't keep (page layout,
//! colours and fonts, drawings, mail merge, Word's own change tracking…) is hidden from the
//! interface and turned off while the profile is on (`markdown.strict`, on by default). The
//! code stays: hiding rather than deleting keeps merges from upstream WordCraft cheap.
//!
//! Every engine command is classified in `plume_commands.txt`, reviewed by hand. A command
//! upstream adds is shown until it is classified, and a test names it.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Ribbon tabs whose every command is Office-only (contextual ones included).
pub const OFFICE_TABS: &[&str] = &["Draw", "Design", "Layout", "Mailings", "Developer", "Chart Design", "SmartArt Design", "Shape Format"];

const COMMANDS: &str = include_str!("plume_commands.txt");

fn table() -> &'static HashMap<&'static str, bool> {
    static T: OnceLock<HashMap<&'static str, bool>> = OnceLock::new();
    T.get_or_init(|| {
        COMMANDS
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter_map(|l| l.split_once(' '))
            .filter_map(|(kind, id)| match kind {
                "office" => Some((id.trim(), true)),
                "markdown" => Some((id.trim(), false)),
                _ => None,
            })
            .collect()
    })
}

/// Whether command `id` is Office-only: hidden and off in the Markdown profile. An unclassified
/// command is not (it shows until it is classified).
pub fn is_office(id: &str) -> bool {
    table().get(id).copied().unwrap_or(false)
}

/// Whether command `id` has been classified (Markdown or Office).
pub fn is_classified(id: &str) -> bool {
    table().contains_key(id)
}

/// Whether ribbon tab `name` holds only Office commands.
pub fn is_office_tab(name: &str) -> bool {
    OFFICE_TABS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command is classified, and the list names no command that doesn't exist: after a
    /// merge from upstream, this test lists what to classify in `plume_commands.txt`.
    #[test]
    fn every_command_is_classified() {
        let reg = crate::cmd::registry();
        let missing: Vec<&str> = reg.all().iter().map(|s| s.id).filter(|id| !is_classified(id)).collect();
        assert!(missing.is_empty(), "classify these commands in crates/engine/src/plume_commands.txt (office or markdown): {missing:?}");
        let gone: Vec<&&str> = table().keys().filter(|id| reg.get(id).is_none()).collect();
        assert!(gone.is_empty(), "these commands no longer exist; remove them from plume_commands.txt: {gone:?}");
    }

    #[test]
    fn markdown_editing_stays_office_layout_goes() {
        for id in [
            "format.bold",
            "format.italic",
            "para.heading1",
            "para.bullets",
            "insert.table",
            "insert.picture",
            "insert.link",
            "review.newComment",
            "references.footnote",
            "references.citation",
            "references.toc",
            "review.trackChanges",
            "vcs.saveVersion",
            "file.exportPdf",
        ] {
            assert!(is_classified(id) && !is_office(id), "{id} is Markdown");
        }
        for id in [
            "format.color",
            "format.underline",
            "insert.textBox",
            "layout.margins",
            "mailings.start",
            "table.merge",
            "design.watermark",
            "draw.pen",
            "review.compare",
            "review.trackingOptions",
        ] {
            assert!(is_office(id), "{id} is Office");
        }
    }
}
