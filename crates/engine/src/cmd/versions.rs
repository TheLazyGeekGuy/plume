//! Document versions kept by Git (Plume): Review › Versions. A version is a commit of the
//! document's Markdown and its pictures; the history is the file's log; "Show changes" opens a
//! version's changes as tracked changes by its author, so Accept and Reject work on them.
//!
//! The commands work on a Markdown document saved in a folder under Git (`vcs.init` makes one).
//! Without Git they say why; `file.versions` (in-memory snapshots) keeps working everywhere, and
//! every saved version is also added there.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("vcs.status", "Version Status", "Review › Versions", status)
            .params(r#"{} → {versioned: bool, root?, file?, modified?, servers?: [name], reason?}"#)
            .pure(),
        CommandSpec::new("vcs.history", "Version History", "Review › Versions", history)
            .params(r#"{"limit"?: 1..1000 (default 100)} → {versions: [{id, short, author, email, date, message}]} newest first"#)
            .pure(),
        CommandSpec::new("vcs.init", "Start Keeping Versions", "Review › Versions", init)
            .params(r#"{} → {root}: makes the document's folder a Git repository"#)
            .pure(),
        CommandSpec::new("vcs.saveVersion", "Save Version", "Review › Versions", save_version)
            .params(r#"{"message": string (what changed), "author"?: string, "email"?: string} → {version: {id, short, author, date, message}}: saves the document as Markdown, then records it and its pictures as a version"#)
            .pure(),
        CommandSpec::new("vcs.showChanges", "Show Changes", "Review › Versions", show_changes)
            .params(r#"{"id": version id, "since"?: version id (default: the version before)} → {changes, author}: an untitled document with the version's changes tracked, by its author"#),
        CommandSpec::new("vcs.compare", "Compare with Version", "Review › Versions", compare_with)
            .params(r#"{"id": version id} → {changes}: an untitled document showing what changed since that version, as tracked changes"#),
        CommandSpec::new("vcs.restore", "Restore Version", "Review › Versions", restore)
            .params(r#"{"id": version id} → {restored}: the document becomes that version (undoable; save a version to keep it)"#),
        CommandSpec::new("vcs.sync", "Synchronize", "Review › Versions", sync)
            .params(r#"{} → {synced: true}: gets the server's versions (never merging silently), then sends this copy's"#)
            .pure(),
    ]
}

/// The document's Markdown file: versions are only kept for a saved Markdown document.
fn markdown_path(s: &Session) -> Result<std::path::PathBuf, CmdError> {
    let path = s.path.clone().ok_or_else(|| CmdError::Failed("save the document first".into()))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !matches!(ext.as_str(), "md" | "markdown") {
        return Err(CmdError::Failed("versions are kept for Markdown documents: save the document as Markdown first".into()));
    }
    Ok(path)
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use wordcraft_vcs::git::{GitError, Repo};

    fn err(e: GitError) -> CmdError {
        CmdError::Failed(e.to_string())
    }

    pub(super) fn repo(s: &Session) -> Result<(Repo, std::path::PathBuf), CmdError> {
        let path = markdown_path(s)?;
        let repo = Repo::discover(&path).map_err(err)?;
        Ok((repo, path))
    }

    pub(super) fn status(s: &mut Session, _: &Value) -> CmdResult {
        let (repo, path) = match repo(s) {
            Ok(r) => r,
            Err(e) => return Ok(json!({"versioned": false, "reason": e.to_string()})),
        };
        Ok(json!({
            "versioned": true,
            "root": repo.root().to_string_lossy(),
            "file": repo.relative(&path).map_err(err)?,
            "modified": repo.is_modified(&path).map_err(err)?,
            "servers": repo.remotes().map_err(err)?,
        }))
    }

    pub(super) fn history(s: &mut Session, v: &Value) -> CmdResult {
        let (repo, path) = repo(s)?;
        let limit = p::u64(v, "limit").unwrap_or(100).clamp(1, wordcraft_vcs::git::MAX_HISTORY as u64) as usize;
        let versions = repo.history(&path, limit).map_err(err)?;
        Ok(json!({"versions": versions.iter().map(wordcraft_vcs::Version::to_json).collect::<Vec<_>>()}))
    }

    pub(super) fn init(s: &mut Session, _: &Value) -> CmdResult {
        let path = markdown_path(s)?;
        if let Ok(repo) = Repo::discover(&path) {
            return Ok(json!({"root": repo.root().to_string_lossy(), "existing": true}));
        }
        let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
        crate::file_access::check_write(&dir.to_string_lossy()).map_err(CmdError::Failed)?;
        let repo = Repo::init(dir).map_err(err)?;
        Ok(json!({"root": repo.root().to_string_lossy(), "existing": false}))
    }

    pub(super) fn save_version(s: &mut Session, v: &Value) -> CmdResult {
        let (repo, path) = repo(s)?;
        let message = p::str(v, "message").unwrap_or("");
        let identity = match (p::str(v, "author"), p::str(v, "email")) {
            (Some(a), e) if !a.trim().is_empty() => Some((a.trim().to_string(), e.unwrap_or("").trim().to_string())),
            _ => None,
        };
        // The version is what is on disk: save first.
        let (_, assets) = crate::io::save_markdown_files(&path, &s.doc).map_err(CmdError::Failed)?;
        s.dirty = false;
        let mut files = vec![path.clone()];
        files.extend(assets);
        let version = repo.save_version(&files, message, identity.as_ref().map(|(a, e)| (a.as_str(), e.as_str()))).map_err(err)?;
        // Kept with the in-memory versions too (File › Info › Version History).
        s.versions.push((version.message.clone(), version.date.clone(), s.doc.clone()));
        if s.versions.len() > 50 {
            s.versions.remove(0);
        }
        Ok(json!({"version": version.to_json()}))
    }

    /// The document as it was in version `id`, its pictures read from that version too.
    pub(super) fn document_at(repo: &Repo, path: &std::path::Path, id: &str) -> Result<Document, CmdError> {
        let rel = repo.relative(path).map_err(err)?;
        let bytes = repo.read_at(id, &rel).map_err(err)?;
        let dir = rel.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
        let text = wordcraft_formats::txt::decode(&bytes);
        let budget = std::cell::Cell::new(1u64 << 30);
        let resolve = |src: &str| -> Option<std::sync::Arc<Vec<u8>>> {
            let src = src.split(['#', '?']).next().unwrap_or("");
            let decoded = String::from_utf8(wordcraft_formats::model::percent_decode(src)).ok()?;
            let full = normalize(&format!("{dir}{decoded}"))?;
            let data = repo.read_at(id, &full).ok()?;
            budget.set(budget.get().checked_sub(data.len() as u64)?);
            Some(std::sync::Arc::new(data))
        };
        let mut doc = wordcraft_formats::markdown::import_with(&text, &crate::io::region_page(&Default::default()), &resolve);
        doc.ensure_nonempty();
        Ok(doc)
    }

    /// `a/b/../c` → `a/c`; `None` when it climbs out of the repository or isn't relative.
    fn normalize(path: &str) -> Option<String> {
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

    pub(super) fn version_id(v: &Value, key: &str) -> Result<String, CmdError> {
        let id = p::req_str(v, key)?;
        if !wordcraft_vcs::is_object_id(id) {
            return Err(CmdError::Params(format!("`{key}` must be a version id")));
        }
        Ok(id.to_string())
    }

    pub(super) fn author_of(repo: &Repo, path: &std::path::Path, id: &str) -> String {
        repo.history(path, wordcraft_vcs::git::MAX_HISTORY)
            .ok()
            .and_then(|h| h.into_iter().find(|v| v.id.starts_with(id) || id.starts_with(&v.id)))
            .map(|v| v.author)
            .unwrap_or_default()
    }

    pub(super) fn show_changes(s: &mut Session, v: &Value) -> CmdResult {
        let (repo, path) = repo(s)?;
        let id = version_id(v, "id")?;
        let since = match v.get("since").filter(|x| !x.is_null()) {
            Some(_) => Some(version_id(v, "since")?),
            None => repo.parent(&id).map_err(err)?,
        };
        let after = document_at(&repo, &path, &id)?;
        let before = match since {
            Some(b) => document_at(&repo, &path, &b)?,
            None => Document::new(),
        };
        let author = author_of(&repo, &path, &id);
        s.doc = before;
        s.doc.ensure_nonempty();
        super::super::tools::compare_docs(s, after, &json!({"showIn": "new", "author": author}))
    }

    pub(super) fn compare_with(s: &mut Session, v: &Value) -> CmdResult {
        let (repo, path) = repo(s)?;
        let id = version_id(v, "id")?;
        let old = document_at(&repo, &path, &id)?;
        let current = s.doc.clone();
        let author = s.author.clone();
        s.doc = old;
        s.doc.ensure_nonempty();
        super::super::tools::compare_docs(s, current, &json!({"showIn": "new", "author": author}))
    }

    pub(super) fn restore(s: &mut Session, v: &Value) -> CmdResult {
        let (repo, path) = repo(s)?;
        let id = version_id(v, "id")?;
        let doc = document_at(&repo, &path, &id)?;
        s.doc = doc;
        s.touch();
        s.dirty = true;
        s.clamp_selection();
        Ok(json!({"restored": id}))
    }

    pub(super) fn sync(s: &mut Session, _: &Value) -> CmdResult {
        let (repo, _) = repo(s)?;
        repo.sync().map_err(err)?;
        Ok(json!({"synced": true}))
    }
}

#[cfg(not(target_arch = "wasm32"))]
use native::{compare_with, history, init, restore, save_version, show_changes, status, sync};

#[cfg(target_arch = "wasm32")]
fn web(s: &mut Session, _: &Value) -> CmdResult {
    markdown_path(s)?;
    Err(CmdError::Failed("versions on the web are kept on GitHub, GitLab or Forgejo: open the document from there".into()))
}

#[cfg(target_arch = "wasm32")]
fn status(s: &mut Session, v: &Value) -> CmdResult {
    let reason = web(s, v).err().map(|e| e.to_string()).unwrap_or_default();
    Ok(json!({"versioned": false, "reason": reason}))
}

#[cfg(target_arch = "wasm32")]
fn history(s: &mut Session, v: &Value) -> CmdResult {
    web(s, v)
}

#[cfg(target_arch = "wasm32")]
fn init(s: &mut Session, v: &Value) -> CmdResult {
    web(s, v)
}

#[cfg(target_arch = "wasm32")]
fn save_version(s: &mut Session, v: &Value) -> CmdResult {
    web(s, v)
}

#[cfg(target_arch = "wasm32")]
fn show_changes(s: &mut Session, v: &Value) -> CmdResult {
    web(s, v)
}

#[cfg(target_arch = "wasm32")]
fn compare_with(s: &mut Session, v: &Value) -> CmdResult {
    web(s, v)
}

#[cfg(target_arch = "wasm32")]
fn restore(s: &mut Session, v: &Value) -> CmdResult {
    web(s, v)
}

#[cfg(target_arch = "wasm32")]
fn sync(s: &mut Session, v: &Value) -> CmdResult {
    web(s, v)
}
