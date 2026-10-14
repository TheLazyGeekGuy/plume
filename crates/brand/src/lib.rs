//! The product's identity, in one place: its name, its reverse-DNS app id and the names of its
//! settings folders. Every crate that shows or stores the name (window titles, the About dialog,
//! PDF, DOCX, ODT, RTF and HTML metadata, the MCP server, the settings and dictionary folders)
//! reads it from here.
//!
//! This crate is standalone (layer 0, no workspace dependencies; see `xtask/src/layers.rs`), so
//! any crate may use it. Crate and module names keep their upstream `wordcraft-*` spelling so
//! merges from WordCraft stay easy; only what users see is branded.
//!
//! Translated interface strings spell the name literally (their catalogs are keyed by the English
//! text); `crates/ui-egui/src/i18n/tests.rs` checks that no catalog still names the upstream
//! product.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

/// The product name shown to users.
pub const APP_NAME: &str = "Plume";

/// Reverse-DNS application id: Wayland app id, Linux desktop entry and AppStream id, Flatpak id
/// and macOS bundle identifier.
pub const APP_ID: &str = "io.github.thelazygeekguy.plume";

/// Settings folder name where folders are capitalised (macOS `Application Support`, Windows
/// `%APPDATA%`).
pub const CONFIG_DIR_TITLE: &str = "Plume";

/// Settings folder name under `$XDG_CONFIG_HOME` (Linux and BSD).
pub const CONFIG_DIR: &str = "plume";

/// The browser build's IndexedDB database name.
pub const WEB_DB_NAME: &str = "plume";

/// The project's home page (source code and issues).
pub const REPO_URL: &str = "https://github.com/TheLazyGeekGuy/wordcraft";

/// The project this one is based on.
pub const UPSTREAM_NAME: &str = "WordCraft";

/// The upstream project's source code.
pub const UPSTREAM_URL: &str = "https://github.com/storytold/wordcraft";

/// Who holds the copyright in metadata and notices.
pub const AUTHORS: &str = "Plume contributors";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_consistent() {
        assert_eq!(CONFIG_DIR, APP_NAME.to_lowercase());
        assert_eq!(CONFIG_DIR_TITLE, APP_NAME);
        assert_eq!(WEB_DB_NAME, CONFIG_DIR);
        assert!(APP_ID.ends_with(&format!(".{CONFIG_DIR}")));
        assert!(APP_ID.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.'));
        assert_ne!(APP_NAME, UPSTREAM_NAME);
    }
}
