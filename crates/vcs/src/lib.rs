//! Versions of one document, kept by Git.
//!
//! Plume shows Git in a word processor's words: a *version* is a commit of the document's
//! Markdown file and its pictures, the *history* is the file's log, *synchronize* is pull then
//! push. Two backends:
//!
//! - [`git`]: a repository on disk, through the `git` program (desktop only). The user's own
//!   Git setup (identity, SSH keys, credential helpers) applies.
//! - [`forge`]: a file in a repository on GitHub, GitLab or Forgejo, through their REST APIs,
//!   without a clone; works in the browser too. The HTTP transport is the app's
//!   ([`forge::Transport`]), so this crate does no networking of its own.
//!
//! Every input that reaches `git` or a URL is checked: revisions must be hex object ids, paths
//! must stay inside the repository, and nothing user-given is ever passed where `git` would read
//! it as an option.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod forge;
#[cfg(not(target_arch = "wasm32"))]
pub mod git;

/// One saved version of a document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Version {
    /// Full object id (commit SHA).
    pub id: String,
    pub author: String,
    pub email: String,
    /// ISO 8601.
    pub date: String,
    /// The version's description (first line of the commit message).
    pub message: String,
}

impl Version {
    /// The first seven characters of the id, as Git shows them.
    pub fn short(&self) -> &str {
        self.id.get(..7).unwrap_or(&self.id)
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"id": self.id, "short": self.short(), "author": self.author, "email": self.email, "date": self.date, "message": self.message})
    }
}

/// Is `id` a Git object id (or an abbreviation of one): 4 to 64 hex digits?
pub fn is_object_id(id: &str) -> bool {
    (4..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A version description fit for a commit message: trimmed, control characters dropped, at
/// most 5,000 characters; empty becomes `fallback`.
pub fn clean_message(message: &str, fallback: &str) -> String {
    let m: String = message.chars().filter(|c| !c.is_control() || *c == '\n').take(5_000).collect();
    let m = m.trim();
    if m.is_empty() { fallback.to_string() } else { m.to_string() }
}

#[cfg(test)]
mod tests;
