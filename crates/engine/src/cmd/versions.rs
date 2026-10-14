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

/// Commands whose result Markdown can't keep: off in strict Markdown mode.
pub const NOT_IN_MARKDOWN: &[&str] = &[
    "format.underline",
    "format.doubleUnderline",
    "format.wordUnderline",
    "format.subscript",
    "format.superscript",
    "format.allCaps",
    "format.smallCaps",
    "format.outline",
    "format.shadow",
    "format.emboss",
    "format.engrave",
    "format.color",
    "format.highlight",
    "format.shading",
    "format.border",
    "format.hidden",
    "format.enclose",
    "format.textEffects",
    "para.alignCenter",
    "para.alignRight",
    "para.distribute",
    "para.shading",
    "para.borders",
    "insert.textBox",
    "insert.shape",
    "insert.wordArt",
    "insert.dropCap",
    "insert.canvas",
    "insert.signatureLine",
    "insert.object",
    "insert.spreadsheet",
    "insert.coverPage",
    "insert.blankPage",
    "insert.pageBreak",
    "insert.header",
    "insert.footer",
    "insert.editHeader",
    "insert.editFooter",
    "insert.createHeader",
    "insert.createFooter",
    "insert.pageNumber",
    "layout.columns",
    "layout.break",
    "layout.lineNumbers",
    "layout.section",
    "design.watermark",
    "design.pageColor",
    "design.pageBorders",
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("markdown.strict", "Keep to Markdown", "File › Options › Markdown", |s, v| {
            let on = p::bool(v, "value").unwrap_or(!s.prefs.markdown_strict);
            s.prefs.markdown_strict = on;
            Ok(json!({"value": on}))
        })
        .params(r#"{"value"?: bool} → {value}: on, a Markdown document's commands whose result Markdown can't keep (colour, underline, text boxes…) are disabled; off (default), they work and the save report lists what Markdown didn't keep"#)
        .pure(),
        CommandSpec::new("forge.signIn", "Sign In to a Forge", "File › Open", remote::sign_in)
            .params(r#"{"forge": "github|gitlab|forgejo", "host"?: string (GitLab or Forgejo server; default gitlab.com / codeberg.org), "token": access token} → {signedIn: api}: the token is kept for this session and never returned"#)
            .pure(),
        CommandSpec::new("forge.signOut", "Sign Out of Forges", "File › Open", |s, _| {
            s.forge_tokens.clear();
            Ok(json!({"signedOut": true}))
        })
        .pure(),
        CommandSpec::new("forge.open", "Open from a Forge", "File › Open", remote::open)
            .params(r#"{"forge": "github|gitlab|forgejo", "host"?: string, "repo": "owner/name" (GitLab: group/…/project), "branch"?: string (default main), "path": "docs/guide.md"} → {opened, version}: the Markdown document and its pictures, read from the forge; Save Version records a version there"#)
            .pure(),
        CommandSpec::new("vcs.status", "Version Status", "Review › Versions", |s, v| if s.forge.is_some() { remote::status(s, v) } else { status(s, v) })
            .params(r#"{} → {versioned: bool, root?, file?, modified?, servers?: [name], reason?}"#)
            .pure(),
        CommandSpec::new("vcs.history", "Version History", "Review › Versions", |s, v| if s.forge.is_some() { remote::history(s, v) } else { history(s, v) })
            .when(markdown_document)
            .params(r#"{"limit"?: 1..1000 (default 100)} → {versions: [{id, short, author, email, date, message}]} newest first"#)
            .pure(),
        CommandSpec::new("vcs.init", "Start Keeping Versions", "Review › Versions", |s, v| if s.forge.is_some() { remote::init(s, v) } else { init(s, v) })
            .when(markdown_document)
            .params(r#"{} → {root}: makes the document's folder a Git repository"#)
            .pure(),
        CommandSpec::new("vcs.saveVersion", "Save Version", "Review › Versions", |s, v| if s.forge.is_some() { remote::save_version(s, v) } else { save_version(s, v) })
            .when(markdown_document)
            .params(r#"{"message": string (what changed), "author"?: string, "email"?: string} → {version: {id, short, author, date, message}}: saves the document as Markdown, then records it and its pictures as a version"#)
            .pure(),
        CommandSpec::new("vcs.showChanges", "Show Changes", "Review › Versions", |s, v| if s.forge.is_some() { remote::show_changes(s, v) } else { show_changes(s, v) })
            .when(markdown_document)
            .params(r#"{"id": version id, "since"?: version id (default: the version before)} → {changes, author}: an untitled document with the version's changes tracked, by its author"#),
        CommandSpec::new("vcs.compare", "Compare with Version", "Review › Versions", |s, v| if s.forge.is_some() { remote::compare_with(s, v) } else { compare_with(s, v) })
            .when(markdown_document)
            .params(r#"{"id": version id} → {changes}: an untitled document showing what changed since that version, as tracked changes"#),
        CommandSpec::new("vcs.restore", "Restore Version", "Review › Versions", |s, v| if s.forge.is_some() { remote::restore(s, v) } else { restore(s, v) })
            .when(markdown_document)
            .params(r#"{"id": version id} → {restored}: the document becomes that version (undoable; save a version to keep it)"#),
        CommandSpec::new("vcs.sync", "Synchronize", "Review › Versions", |s, v| if s.forge.is_some() { remote::sync(s, v) } else { sync(s, v) })
            .when(markdown_document)
            .params(r#"{} → {synced: true}: gets the server's versions (never merging silently), then sends this copy's"#)
            .pure(),
    ]
}

/// Versions need a saved Markdown document, or one opened from a forge (the reason shown on a
/// disabled button).
fn markdown_document(s: &Session) -> Option<&'static str> {
    if s.forge.is_some() {
        return None;
    }
    markdown_path(s).err().map(|_| "Save the document as Markdown first")
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

/// The folder of a repository path, with a trailing `/` (empty at the root).
fn folder_of(path: &str) -> String {
    path.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default()
}

/// A document opened from GitHub, GitLab or Forgejo: where it is, and the blob id the edits
/// started from (the server refuses a version saved over someone else's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub doc: wordcraft_vcs::forge::DocRef,
    pub sha: String,
}

/// Versions on a forge, through its REST API: the same commands, no clone.
mod remote {
    use std::sync::Arc;

    use wordcraft_vcs::forge::{Client, DocRef, Forge, ForgeError, Transport};

    use super::*;

    /// Most pictures one document loads from a forge.
    const MAX_PICTURES: usize = 200;

    fn ferr(e: ForgeError) -> CmdError {
        CmdError::Failed(e.to_string())
    }

    fn forge_of(v: &Value) -> Result<Forge, CmdError> {
        let host = p::str(v, "host").map(|h| h.trim().trim_start_matches("https://").trim_end_matches('/').to_string()).filter(|h| !h.is_empty());
        if let Some(h) = &host
            && (h.contains(['/', '?', '#', '@', ' ']) || h.chars().any(char::is_control))
        {
            return Err(CmdError::Params(format!("{h:?}: give the server's name, like gitlab.example.org")));
        }
        match p::str(v, "forge").unwrap_or("github") {
            "github" => Ok(Forge::github()),
            "gitlab" => Ok(Forge::gitlab(host.as_deref().unwrap_or("gitlab.com"))),
            "forgejo" | "gitea" | "codeberg" => Ok(Forge::forgejo(host.as_deref().unwrap_or("codeberg.org"))),
            f => Err(CmdError::Params(format!("unknown forge {f:?}: github, gitlab or forgejo"))),
        }
    }

    fn transport(s: &Session) -> Result<Arc<dyn Transport + Send + Sync>, CmdError> {
        s.forge_transport.clone().ok_or_else(|| CmdError::Failed("forges can't be reached from here".into()))
    }

    fn token(s: &Session, forge: &Forge) -> Result<String, CmdError> {
        s.forge_tokens
            .get(&forge.api)
            .map(|t| t.as_str().to_string())
            .ok_or_else(|| CmdError::Failed(format!("sign in to {} first", forge.kind.name())))
    }

    fn current(s: &Session) -> Result<Remote, CmdError> {
        s.forge.clone().ok_or_else(|| CmdError::Failed("this document wasn't opened from a forge".into()))
    }

    pub(super) fn sign_in(s: &mut Session, v: &Value) -> CmdResult {
        let forge = forge_of(v)?;
        let token = p::req_str(v, "token")?.trim();
        if token.is_empty() || token.len() > 4096 {
            return Err(CmdError::Params("`token` is the forge's access token".into()));
        }
        s.forge_tokens.insert(forge.api.clone(), crate::io::Password::new(token));
        Ok(json!({"signedIn": forge.api}))
    }

    /// The document at `at` (or the branch's latest), its pictures read from the same version.
    fn load(c: &Client, doc: &DocRef, at: Option<&str>) -> Result<(Document, String), CmdError> {
        let (bytes, sha) = c.read(doc, at).map_err(ferr)?;
        let text = wordcraft_formats::txt::decode(&bytes);
        let dir = folder_of(&doc.path);
        let count = std::cell::Cell::new(0usize);
        let budget = std::cell::Cell::new(1u64 << 30);
        let resolve = |src: &str| -> Option<Arc<Vec<u8>>> {
            if count.get() >= MAX_PICTURES || src.contains("://") {
                return None;
            }
            count.set(count.get() + 1);
            let src = src.split(['#', '?']).next().unwrap_or("");
            let decoded = String::from_utf8(wordcraft_formats::model::percent_decode(src)).ok()?;
            let full = normalize(&format!("{dir}{decoded}"))?;
            let (data, _) = c.read_path(doc, &full, at).ok()?;
            budget.set(budget.get().checked_sub(data.len() as u64)?);
            Some(Arc::new(data))
        };
        let mut d = wordcraft_formats::markdown::import_with(&text, &crate::io::region_page(&Default::default()), &resolve);
        wordcraft_formats::markdown::apply_theme(&mut d, Default::default());
        d.ensure_nonempty();
        Ok((d, sha))
    }

    pub(super) fn open(s: &mut Session, v: &Value) -> CmdResult {
        let forge = forge_of(v)?;
        let path = p::req_str(v, "path")?.trim().trim_start_matches('/').to_string();
        let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if !matches!(ext.as_str(), "md" | "markdown") {
            return Err(CmdError::Params("`path` names a Markdown file (.md)".into()));
        }
        let doc = DocRef {
            forge,
            repo: p::req_str(v, "repo")?.trim().trim_matches('/').to_string(),
            branch: p::str(v, "branch").unwrap_or("main").trim().to_string(),
            path,
        };
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &doc.forge)? };
        let (d, sha) = load(&c, &doc, None)?;
        s.set_document(d);
        let name = doc.path.rsplit('/').next().unwrap_or(&doc.path).to_string();
        s.path = None;
        s.file_name = Some(name.trim_end_matches(".markdown").trim_end_matches(".md").to_string());
        s.forge = Some(Remote { doc, sha: sha.clone() });
        s.imported_from = None;
        s.import_report.clear();
        s.dirty = false;
        Ok(json!({"opened": true, "version": sha}))
    }

    pub(super) fn status(s: &mut Session, _: &Value) -> CmdResult {
        let r = current(s)?;
        Ok(
            json!({"versioned": true, "forge": r.doc.forge.kind.name(), "repo": r.doc.repo, "branch": r.doc.branch, "file": r.doc.path, "modified": s.dirty}),
        )
    }

    pub(super) fn init(_: &mut Session, _: &Value) -> CmdResult {
        Ok(json!({"existing": true}))
    }

    fn versions(s: &Session, limit: usize) -> Result<Vec<wordcraft_vcs::Version>, CmdError> {
        let r = current(s)?;
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        c.history(&r.doc, limit).map_err(ferr)
    }

    pub(super) fn history(s: &mut Session, v: &Value) -> CmdResult {
        let limit = p::u64(v, "limit").unwrap_or(100).clamp(1, wordcraft_vcs::forge::MAX_HISTORY as u64) as usize;
        let list = versions(s, limit)?;
        Ok(json!({"versions": list.iter().map(wordcraft_vcs::Version::to_json).collect::<Vec<_>>()}))
    }

    pub(super) fn save_version(s: &mut Session, v: &Value) -> CmdResult {
        let r = current(s)?;
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let doc = crate::io::web_pictures(&s.doc);
        let bundle = wordcraft_formats::markdown::export_native(&doc, &Default::default());
        let dir = folder_of(&r.doc.path);
        let assets: Vec<(String, Vec<u8>)> =
            bundle.assets.iter().filter_map(|(p, b)| normalize(&format!("{dir}{p}")).map(|full| (full, b.to_vec()))).collect();
        let message = p::str(v, "message").unwrap_or("");
        let id = match c.save_version(&r.doc, bundle.text.as_bytes(), &assets, message, Some(&r.sha)) {
            Ok(id) => id,
            Err(ForgeError::Conflict) => {
                return Err(CmdError::Failed("someone saved a newer version: use Synchronize to see their changes first".into()));
            }
            Err(e) => return Err(ferr(e)),
        };
        let (_, sha) = c.read(&r.doc, None).map_err(ferr)?;
        if let Some(f) = s.forge.as_mut() {
            f.sha = sha;
        }
        s.dirty = false;
        let message = wordcraft_vcs::clean_message(message, "Update document");
        s.versions.push((message.clone(), super::super::now_iso(), s.doc.clone()));
        if s.versions.len() > 50 {
            s.versions.remove(0);
        }
        let short = id.get(..7).unwrap_or(&id).to_string();
        Ok(json!({"version": {"id": id, "short": short, "message": message}}))
    }

    fn version_id(v: &Value, key: &str) -> Result<String, CmdError> {
        let id = p::req_str(v, key)?;
        if !wordcraft_vcs::is_object_id(id) {
            return Err(CmdError::Params(format!("`{key}` must be a version id")));
        }
        Ok(id.to_string())
    }

    pub(super) fn show_changes(s: &mut Session, v: &Value) -> CmdResult {
        let r = current(s)?;
        let id = version_id(v, "id")?;
        let list = versions(s, wordcraft_vcs::forge::MAX_HISTORY)?;
        let pos = list.iter().position(|x| x.id.starts_with(&id) || id.starts_with(&x.id));
        let author = pos.and_then(|k| list.get(k)).map(|x| x.author.clone()).unwrap_or_default();
        let since = match v.get("since").filter(|x| !x.is_null()) {
            Some(_) => Some(version_id(v, "since")?),
            None => pos.and_then(|k| list.get(k + 1)).map(|x| x.id.clone()),
        };
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let (after, _) = load(&c, &r.doc, Some(&id))?;
        let before = match since {
            Some(b) => load(&c, &r.doc, Some(&b))?.0,
            None => Document::new(),
        };
        s.doc = before;
        s.doc.ensure_nonempty();
        // The comparison is a new, untitled document: never saved over the forge's.
        s.forge = None;
        super::super::tools::compare_docs(s, after, &json!({"showIn": "new", "author": author}))
    }

    pub(super) fn compare_with(s: &mut Session, v: &Value) -> CmdResult {
        let r = current(s)?;
        let id = version_id(v, "id")?;
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let (old, _) = load(&c, &r.doc, Some(&id))?;
        let current_doc = s.doc.clone();
        let author = s.author.clone();
        s.doc = old;
        s.doc.ensure_nonempty();
        s.forge = None;
        super::super::tools::compare_docs(s, current_doc, &json!({"showIn": "new", "author": author}))
    }

    pub(super) fn restore(s: &mut Session, v: &Value) -> CmdResult {
        let r = current(s)?;
        let id = version_id(v, "id")?;
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let (d, _) = load(&c, &r.doc, Some(&id))?;
        s.doc = d;
        s.touch();
        s.clamp_selection();
        Ok(json!({"restored": id}))
    }

    /// Get the server's latest version: reloaded when this copy has no unsaved changes; with
    /// changes on both sides, an error that says how to see theirs (Compare with Version).
    pub(super) fn sync(s: &mut Session, _: &Value) -> CmdResult {
        let r = current(s)?;
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let (_, sha) = c.read(&r.doc, None).map_err(ferr)?;
        if sha == r.sha {
            return Ok(json!({"synced": true, "upToDate": true}));
        }
        if s.dirty {
            return Err(CmdError::Failed(
                "the document changed on the server and here: open Version History and use Compare on the latest version to see their changes".into(),
            ));
        }
        let (d, sha) = load(&c, &r.doc, None)?;
        let remote = Remote { doc: r.doc, sha };
        let name = s.file_name.clone();
        s.set_document(d);
        s.forge = Some(remote);
        s.file_name = name;
        s.dirty = false;
        Ok(json!({"synced": true, "updated": true}))
    }
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
            let full = super::normalize(&format!("{dir}{decoded}"))?;
            let data = repo.read_at(id, &full).ok()?;
            budget.set(budget.get().checked_sub(data.len() as u64)?);
            Some(std::sync::Arc::new(data))
        };
        let mut doc = wordcraft_formats::markdown::import_with(&text, &crate::io::region_page(&Default::default()), &resolve);
        wordcraft_formats::markdown::apply_theme(&mut doc, Default::default());
        doc.ensure_nonempty();
        Ok(doc)
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
