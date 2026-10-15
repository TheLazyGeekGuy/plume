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
        CommandSpec::new("file.documents", "Documents", "View › Show", documents)
            .params(r#"{} → {root, current?, documents: [path relative to root]}: the Markdown documents in this document's repository (or folder)"#)
            .pure(),
        CommandSpec::new("forge.signIn", "Sign In to a Forge", "File › Open", remote::sign_in)
            .params(r#"{"forge": "github|gitlab|forgejo", "host"?: string (GitLab or Forgejo server; default gitlab.com / codeberg.org), "token": access token} → {signedIn: api}: the token is kept for this session and never returned"#)
            .pure(),
        CommandSpec::new("forge.signOut", "Sign Out of Forges", "File › Open", |s, _| {
            s.forge_tokens.clear();
            Ok(json!({"signedOut": true}))
        })
        .pure(),
        CommandSpec::new("forge.repos", "Forge Repositories", "File › Open", remote::repos)
            .params(r#"{"forge": "github|gitlab|forgejo", "host"?: string} → {repos: [{name, branch, description}]}: the signed-in user's repositories, most recently active first"#)
            .pure(),
        CommandSpec::new("forge.files", "Forge Documents", "File › Open", remote::files)
            .params(r#"{"forge": "github|gitlab|forgejo", "host"?: string, "repo": string, "branch"?: string} → {files: [path]}: the repository's Markdown documents"#)
            .pure(),
        CommandSpec::new("forge.open", "Open from a Forge", "File › Open", remote::open)
            .params(r#"{"url"?: the file's page on the forge (https://github.com/o/r/blob/main/a.md, …/-/blob/…, …/src/branch/…), or "forge": "github|gitlab|forgejo", "host"?: string, "repo": "owner/name" (GitLab: group/…/project), "branch"?: string (default main), "path": "docs/guide.md"} → {opened, version}: the Markdown document and its pictures, read from the forge; Save Version records a version there"#)
            .pure(),
        CommandSpec::new("forge.saveAs", "Save to a Forge", "File › Save As", remote::save_as)
            .params(r#"{"forge": "github|gitlab|forgejo", "host"?: string, "repo": "owner/name", "branch"?: string (default main), "path": "docs/guide.md" (.md added when missing), "message"?: string, "replace"?: bool} → {saved, path, version}: the document saved as Markdown (pictures beside it) in a new version of the repository; from then on Save Version records versions there. An existing file is replaced only with "replace": true"#)
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
        CommandSpec::new("vcs.requestReview", "Request Review", "Review › Versions", remote::request_review)
            .when(forge_document)
            .params(r#"{"title": string, "message"?: string} → {review: {number, url, branch}}: saves this version on a new branch and opens a pull request (GitHub, Forgejo) or merge request (GitLab) for colleagues to review"#)
            .pure(),
        CommandSpec::new("vcs.reviewComments", "Get Review Comments", "Review › Versions", remote::review_comments)
            .when(forge_document)
            .params(r#"{} → {imported, comments}: colleagues' comments on the review become comments in the document, on the paragraph they're about"#),
        CommandSpec::new("vcs.sendReplies", "Send Replies", "Review › Versions", remote::send_replies)
            .when(forge_document)
            .params(r#"{} → {sent}: your replies to reviewers' comments are posted on the review, in the comment's thread"#),
        CommandSpec::new("vcs.reviewStatus", "Review Status", "Review › Versions", remote::review_status)
            .when(forge_document)
            .params(r#"{} → {review?, approvedBy: [name], changesRequestedBy: [name], merged, closed}"#)
            .pure(),
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

/// The Markdown documents beside this one: in its Git repository, else in its folder.
#[cfg(not(target_arch = "wasm32"))]
fn documents(s: &mut Session, _: &Value) -> CmdResult {
    let path = s.path.clone().ok_or_else(|| CmdError::Failed("save the document first".into()))?;
    let root = match wordcraft_vcs::git::Repo::discover(&path) {
        Ok(repo) => repo.root().to_path_buf(),
        Err(_) => path.parent().filter(|d| !d.as_os_str().is_empty()).map(std::path::Path::to_path_buf).unwrap_or_else(|| ".".into()),
    };
    let list = crate::io::markdown_documents(&root).map_err(CmdError::Failed)?;
    // Compare resolved paths: on macOS `/var/…` is `/private/var/…`, and Git names the latter.
    let resolved = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let current = resolved(&path).strip_prefix(resolved(&root)).ok().map(|p| p.to_string_lossy().replace('\\', "/"));
    Ok(json!({"root": root.to_string_lossy(), "current": current, "documents": list}))
}

#[cfg(target_arch = "wasm32")]
fn documents(_: &mut Session, _: &Value) -> CmdResult {
    Err(CmdError::Failed("folders of documents work in the desktop app".into()))
}

/// Reviews need a document opened from a forge.
fn forge_document(s: &Session) -> Option<&'static str> {
    s.forge.is_none().then_some("Open the document from GitHub, GitLab or Forgejo to ask for a review")
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
    /// The review request this document is in, if any.
    pub review: Option<wordcraft_vcs::forge::Review>,
    /// Reviewers' comments already brought in as comments (by forge id).
    pub imported: std::collections::BTreeSet<String>,
    /// The document's comments that start a reviewer's thread: where replies to them go, and
    /// who wrote them.
    pub threads: std::collections::BTreeMap<u32, (wordcraft_vcs::forge::ReplyTo, String)>,
    /// The document's comments already on the forge (brought in, or replies sent).
    pub sent: std::collections::BTreeSet<u32>,
}

pub use remote::{DeviceSignIn, ForgeListing, parse_web_address};

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

    /// A file's page on a forge → (forge, server, repository, branch, path in the repository):
    /// `https://github.com/owner/repo/blob/main/docs/a.md`,
    /// `https://gitlab.com/group/sub/project/-/blob/main/docs/a.md`,
    /// `https://codeberg.org/owner/repo/src/branch/main/docs/a.md` (Forgejo, Gitea). `hint` names
    /// the forge when the server's name doesn't (a self-hosted GitLab or Forgejo).
    pub fn parse_web_address(url: &str, hint: Option<&str>) -> Result<(String, String, String, String, String), CmdError> {
        let bad = || CmdError::Params(format!("{url:?}: paste the address of the Markdown file's page on the forge"));
        let rest = url.trim().trim_start_matches("https://").trim_start_matches("http://");
        let rest = rest.split(['?', '#']).next().unwrap_or("");
        let (host, path) = rest.split_once('/').ok_or_else(bad)?;
        let host = host.to_ascii_lowercase();
        let decode = |p: &str| String::from_utf8(wordcraft_formats::model::percent_decode(p)).unwrap_or_else(|_| p.to_string());
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let join = |p: &[&str]| p.iter().map(|x| decode(x)).collect::<Vec<_>>().join("/");
        // GitLab: …/-/blob/<branch>/<path>
        if let Some(k) = parts.iter().position(|p| *p == "-")
            && parts.get(k + 1) == Some(&"blob")
            && k >= 2
        {
            let branch = parts.get(k + 2).ok_or_else(bad)?;
            return Ok(("gitlab".into(), host, join(&parts[..k]), decode(branch), join(parts.get(k + 3..).unwrap_or(&[]))));
        }
        let (owner, repo) = (parts.first().ok_or_else(bad)?, parts.get(1).ok_or_else(bad)?);
        let repo_name = format!("{}/{}", decode(owner), decode(repo.trim_end_matches(".git")));
        match parts.get(2).copied() {
            // GitHub: owner/repo/blob/<branch>/<path>
            Some("blob") | Some("raw") if host == "github.com" || hint == Some("github") => {
                let branch = parts.get(3).ok_or_else(bad)?;
                Ok(("github".into(), host, repo_name, decode(branch), join(parts.get(4..).unwrap_or(&[]))))
            }
            // Forgejo, Gitea: owner/repo/src/branch/<branch>/<path>
            Some("src") | Some("raw") if parts.get(3) == Some(&"branch") => {
                let branch = parts.get(4).ok_or_else(bad)?;
                Ok(("forgejo".into(), host, repo_name, decode(branch), join(parts.get(5..).unwrap_or(&[]))))
            }
            _ => Err(bad()),
        }
    }

    /// A `forge.repos` or `forge.files` listing, prepared on the session and run anywhere: the
    /// app runs it on a worker thread so its window doesn't wait on the network.
    pub struct ForgeListing {
        transport: Arc<dyn Transport + Send + Sync>,
        token: String,
        forge: Forge,
        /// `None`: the account's projects; else (repository, branch): its Markdown documents.
        files: Option<(String, String)>,
    }

    impl ForgeListing {
        /// `command` is `forge.repos` or `forge.files`, with that command's parameters.
        pub fn prepare(s: &Session, command: &str, v: &Value) -> Result<Self, CmdError> {
            let forge = forge_of(v)?;
            let files = match command {
                "forge.repos" => None,
                "forge.files" => {
                    Some((p::req_str(v, "repo")?.trim().trim_matches('/').to_string(), p::str(v, "branch").unwrap_or("main").trim().to_string()))
                }
                c => return Err(CmdError::Params(format!("{c:?} is not a forge listing"))),
            };
            Ok(Self { transport: transport(s)?, token: token(s, &forge)?, forge, files })
        }

        /// The command's result.
        pub fn run(self) -> CmdResult {
            let c = Client { transport: self.transport.as_ref(), token: self.token };
            match &self.files {
                None => {
                    let list = c.list_repos(&self.forge).map_err(ferr)?;
                    Ok(
                        json!({"repos": list.iter().map(|r| json!({"name": r.name, "branch": r.branch, "description": r.description})).collect::<Vec<_>>()}),
                    )
                }
                Some((repo, branch)) => Ok(json!({"files": c.list_markdown(&self.forge, repo, branch).map_err(ferr)?})),
            }
        }
    }

    /// Signing in through the forge's web page (OAuth device flow, GitHub and GitLab): the person
    /// types a short code there; the app waits on a worker thread. Needs the OAuth application's
    /// client id (registered on the forge, device flow enabled).
    pub struct DeviceSignIn {
        transport: Arc<dyn Transport + Send + Sync>,
        forge: Forge,
        client_id: String,
    }

    impl DeviceSignIn {
        /// `{forge, host?, clientId}` as for `forge.signIn`.
        pub fn prepare(s: &Session, v: &Value) -> Result<Self, CmdError> {
            let forge = forge_of(v)?;
            let client_id = p::req_str(v, "clientId")?.trim().to_string();
            if client_id.is_empty()
                || client_id.len() > 200
                || !client_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
            {
                return Err(CmdError::Params("`clientId` is the OAuth application's client id".into()));
            }
            Ok(Self { transport: transport(s)?, forge, client_id })
        }

        fn site(&self) -> String {
            self.forge.api.trim_end_matches("/api/v4").trim_end_matches("/api/v1").to_string()
        }

        /// Ask the forge for a code (shown with `show`: the code to type, and the page to type it
        /// on), then wait until the person approves, refuses, the code expires (15 minutes) or
        /// `cancel` is set. The access token.
        pub fn run(self, show: impl FnOnce(&wordcraft_vcs::forge::DeviceCode), cancel: &std::sync::atomic::AtomicBool) -> Result<String, CmdError> {
            use std::sync::atomic::Ordering;
            use std::time::{Duration, Instant};
            use wordcraft_vcs::forge::{device_poll, device_start};
            let t = self.transport.as_ref();
            let site = self.site();
            let scope = if self.forge.kind == wordcraft_vcs::forge::Kind::GitLab { "api" } else { "repo" };
            let code = device_start(t, self.forge.kind, &site, &self.client_id, scope).map_err(ferr)?;
            show(&code);
            let end = Instant::now() + Duration::from_secs(15 * 60);
            let mut wait = Duration::from_secs(code.interval);
            loop {
                // Sleep in short steps so Cancel is quick.
                let until = Instant::now() + wait;
                while Instant::now() < until {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(CmdError::Failed("sign-in cancelled".into()));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                if Instant::now() > end {
                    return Err(CmdError::Failed("the sign-in code expired: start again".into()));
                }
                match device_poll(t, self.forge.kind, &site, &self.client_id, &code) {
                    Ok(Some(token)) => return Ok(token),
                    // Pending (or slow down: wait a little longer each time).
                    Ok(None) => wait = (wait + Duration::from_secs(1)).min(Duration::from_secs(30)),
                    Err(e) => return Err(ferr(e)),
                }
            }
        }
    }

    pub(super) fn repos(s: &mut Session, v: &Value) -> CmdResult {
        ForgeListing::prepare(s, "forge.repos", v)?.run()
    }

    pub(super) fn files(s: &mut Session, v: &Value) -> CmdResult {
        ForgeListing::prepare(s, "forge.files", v)?.run()
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
        // A document's web address (copied from the browser) says it all.
        let parsed;
        let v = match p::str(v, "url").filter(|u| !u.trim().is_empty()) {
            Some(u) => {
                let (forge, host, repo, branch, path) = parse_web_address(u, p::str(v, "forge"))?;
                parsed = json!({"forge": forge, "host": host, "repo": repo, "branch": branch, "path": path});
                &parsed
            }
            None => v,
        };
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
        s.forge =
            Some(Remote { doc, sha: sha.clone(), review: None, imported: Default::default(), threads: Default::default(), sent: Default::default() });
        s.imported_from = None;
        s.import_report.clear();
        s.dirty = false;
        Ok(json!({"opened": true, "version": sha}))
    }

    /// Save the document (new, or from anywhere) as a Markdown file in a forge repository; it
    /// becomes a forge document, as if opened from there.
    pub(super) fn save_as(s: &mut Session, v: &Value) -> CmdResult {
        let forge = forge_of(v)?;
        let mut path = p::req_str(v, "path")?.trim().trim_matches('/').to_string();
        let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if !matches!(ext.as_str(), "md" | "markdown") {
            path.push_str(".md");
        }
        let path = normalize(&path)
            .filter(|p| !p.is_empty() && !p.starts_with(".md"))
            .ok_or_else(|| CmdError::Params(format!("{path:?}: name the file, like docs/guide.md")))?;
        let doc = DocRef {
            forge,
            repo: p::req_str(v, "repo")?.trim().trim_matches('/').to_string(),
            branch: p::str(v, "branch").unwrap_or("main").trim().to_string(),
            path,
        };
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &doc.forge)? };
        let existing = match c.read(&doc, None) {
            Ok((_, sha)) => Some(sha),
            Err(ForgeError::NotFound) => None,
            Err(e) => return Err(ferr(e)),
        };
        if existing.is_some() && p::bool(v, "replace") != Some(true) {
            return Err(CmdError::Failed(format!("{} already has {}: choose another name, or replace it", doc.repo, doc.path)));
        }
        let pictures = crate::io::web_pictures(&s.doc);
        let bundle = wordcraft_formats::markdown::export_native(&pictures, &Default::default());
        let dir = folder_of(&doc.path);
        let assets: Vec<(String, Vec<u8>)> =
            bundle.assets.iter().filter_map(|(p, b)| normalize(&format!("{dir}{p}")).map(|full| (full, b.to_vec()))).collect();
        let name = doc.path.rsplit('/').next().unwrap_or(&doc.path).to_string();
        let default_message = format!("{} {name}", if existing.is_some() { "Update" } else { "Add" });
        let message = p::str(v, "message").filter(|m| !m.trim().is_empty()).unwrap_or(&default_message).to_string();
        let id = c.save_version(&doc, bundle.text.as_bytes(), &assets, &message, existing.as_deref()).map_err(|e| match e {
            ForgeError::Conflict => CmdError::Failed("someone saved this file meanwhile: try again".into()),
            e => ferr(e),
        })?;
        let (_, sha) = c.read(&doc, None).map_err(ferr)?;
        s.path = None;
        s.file_name = Some(name.trim_end_matches(".markdown").trim_end_matches(".md").to_string());
        let saved = doc.path.clone();
        s.forge = Some(Remote { doc, sha, review: None, imported: Default::default(), threads: Default::default(), sent: Default::default() });
        s.imported_from = None;
        s.dirty = false;
        let short = id.get(..7).unwrap_or(&id).to_string();
        Ok(json!({"saved": true, "path": saved, "version": {"id": id, "short": short, "message": message}}))
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

    pub(super) fn request_review(s: &mut Session, v: &Value) -> CmdResult {
        let r = current(s)?;
        if let Some(rv) = &r.review {
            return Err(CmdError::Failed(format!("this document is already in review {}: save versions to update it", rv.number)));
        }
        let title = p::str(v, "title").unwrap_or("").trim().to_string();
        let message = p::str(v, "message").unwrap_or("").to_string();
        // review/<file>-<yyyymmdd-hhmmss>: a new branch for this review's versions.
        let stem: String = r
            .doc
            .path
            .rsplit('/')
            .next()
            .unwrap_or("document")
            .trim_end_matches(".md")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .take(40)
            .collect();
        let stamp: String = super::super::now_iso().chars().filter(char::is_ascii_digit).take(14).collect();
        let branch = format!("review/{}-{stamp}", if stem.is_empty() { "document" } else { &stem });
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let review = c.request_review(&r.doc, &branch, if title.is_empty() { "Review request" } else { &title }, &message).map_err(ferr)?;
        // This copy's edits become the first version under review.
        let on_branch = DocRef { branch: branch.clone(), ..r.doc.clone() };
        if let Some(f) = s.forge.as_mut() {
            f.doc = on_branch;
            f.review = Some(review.clone());
        }
        let (_, sha) = c.read(&DocRef { branch, ..r.doc }, None).map_err(ferr)?;
        if let Some(f) = s.forge.as_mut() {
            f.sha = sha;
        }
        if s.dirty {
            save_version(s, &json!({"message": if title.is_empty() { "Version for review".to_string() } else { title }}))?;
        }
        Ok(json!({"review": {"number": review.number, "url": review.url, "branch": review.branch}}))
    }

    /// Words of a Markdown line without its syntax, for finding the paragraph it is.
    fn plain_words(line: &str) -> String {
        let t = line.trim().trim_start_matches(['#', '>', '-', '*', '+', '|', ' ']);
        let t = t.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches(['.', ')', ' ']);
        t.chars()
            .filter(|c| !matches!(c, '*' | '_' | '`' | '~' | '\\' | '[' | ']'))
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The top-level paragraph a Markdown line is about (by its words), else the first one.
    fn paragraph_for(doc: &Document, md: &str, line: Option<u32>) -> usize {
        let first = doc.body.iter().position(|b| matches!(&**b, wordcraft_doc::Block::Para(_))).unwrap_or(0);
        let Some(text) = line.and_then(|l| md.lines().nth(l.saturating_sub(1) as usize)).map(plain_words) else { return first };
        let probe: String = text.chars().take(40).collect();
        if probe.chars().count() < 3 {
            return first;
        }
        doc.body
            .iter()
            .position(|b| match &**b {
                wordcraft_doc::Block::Para(p) => p.plain_text().split_whitespace().collect::<Vec<_>>().join(" ").contains(&probe),
                wordcraft_doc::Block::Table(_) => false,
            })
            .unwrap_or(first)
    }

    pub(super) fn review_comments(s: &mut Session, _: &Value) -> CmdResult {
        let r = current(s)?;
        let review = r.review.clone().ok_or_else(|| CmdError::Failed("ask for a review first".into()))?;
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let comments = c.review_comments(&r.doc, review.number).map_err(ferr)?;
        let (md, _) = c.read(&r.doc, None).map_err(ferr)?;
        let md = wordcraft_formats::txt::decode(&md);
        let mut imported = 0usize;
        let (mut threads, mut sent) = (r.threads.clone(), r.sent.clone());
        for rc in comments.iter().filter(|rc| !r.imported.contains(&rc.id)) {
            // A comment on another file of the repository isn't about this document.
            if rc.path.as_deref().is_some_and(|p| p != r.doc.path) {
                continue;
            }
            let text = wordcraft_formats::model::clean_text(&rc.body);
            let initials: String = rc.author.split_whitespace().filter_map(|w| w.chars().next()).take(3).collect();
            let id = wordcraft_doc::free_id(s.doc.comments.keys().next_back().copied(), s.doc.comments.keys().copied(), 0)
                .ok_or_else(|| CmdError::Failed("no free comment id".into()))?;
            // A reply in a thread already in the document joins it.
            let root = (rc.reply_to != wordcraft_vcs::forge::ReplyTo::Conversation)
                .then(|| threads.iter().find(|(k, (to, _))| *to == rc.reply_to && s.doc.comments.contains_key(k)).map(|(k, _)| *k))
                .flatten();
            let part = s.doc.try_add_part(
                wordcraft_doc::PartKind::Comment,
                vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(&text, Default::default()))],
            )?;
            let comment =
                wordcraft_doc::Comment { author: rc.author.clone(), initials, date: rc.date.clone(), part, parent: root, ..Default::default() };
            if root.is_none() {
                let bi = paragraph_for(&s.doc, &md, rc.line);
                let Some(wordcraft_doc::Block::Para(para)) = s.doc.body.get(bi).map(|b| &**b) else { continue };
                let len = para.len();
                s.doc.comments.insert(id, comment);
                let props = wordcraft_doc::CharProps::default();
                s.doc.insert_object(&wordcraft_doc::Pos::body(bi, len), wordcraft_doc::InlineObject::CommentEnd { id }, &props)?;
                s.doc.insert_object(&wordcraft_doc::Pos::body(bi, 0), wordcraft_doc::InlineObject::CommentStart { id }, &props)?;
                threads.insert(id, (rc.reply_to.clone(), rc.author.clone()));
            } else {
                s.doc.comments.insert(id, comment);
            }
            sent.insert(id);
            imported += 1;
        }
        if let Some(f) = s.forge.as_mut() {
            f.imported.extend(comments.iter().map(|c| c.id.clone()));
            f.threads = threads;
            f.sent = sent;
        }
        if imported > 0 {
            s.view.comments_pane = true;
        }
        Ok(json!({"imported": imported, "comments": comments.len()}))
    }

    /// The replies typed under reviewers' comments that aren't on the forge yet go to the
    /// comment's thread (or, where the forge has none, to the review's conversation, quoting it).
    pub(super) fn send_replies(s: &mut Session, _: &Value) -> CmdResult {
        let r = current(s)?;
        let review = r.review.clone().ok_or_else(|| CmdError::Failed("ask for a review first".into()))?;
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let root_of = |mut id: u32| {
            for _ in 0..64 {
                match s.doc.comments.get(&id).and_then(|c| c.parent) {
                    Some(p) => id = p,
                    None => break,
                }
            }
            id
        };
        let text =
            |id: u32| s.doc.comments.get(&id).map(|c| s.doc.plain_text(wordcraft_doc::StoryRef::Part(c.part))).unwrap_or_default().trim().to_string();
        let mut todo = Vec::new();
        for (id, cm) in &s.doc.comments {
            if cm.parent.is_none() || r.sent.contains(id) {
                continue;
            }
            let root = root_of(*id);
            let Some((to, author)) = r.threads.get(&root) else { continue };
            let body = text(*id);
            if body.is_empty() {
                continue;
            }
            let body = if *to == wordcraft_vcs::forge::ReplyTo::Conversation {
                // No thread to reply in: say which comment this answers.
                let quoted: String = text(root).lines().next().unwrap_or("").chars().take(200).collect();
                format!("> {author}: {quoted}\n\n{body}")
            } else {
                body
            };
            todo.push((*id, to.clone(), body));
        }
        let mut done = 0usize;
        for (id, to, body) in todo {
            let posted = c.reply(&r.doc, review.number, &to, &body).map_err(ferr);
            let Some(f) = s.forge.as_mut() else { break };
            match posted {
                Ok(forge_id) => {
                    f.sent.insert(id);
                    if !forge_id.is_empty() {
                        f.imported.insert(forge_id);
                    }
                    done += 1;
                }
                Err(e) => return Err(CmdError::Failed(format!("{done} replies sent; the next one failed: {e}"))),
            }
        }
        Ok(json!({"sent": done}))
    }

    pub(super) fn review_status(s: &mut Session, _: &Value) -> CmdResult {
        let r = current(s)?;
        let Some(review) = r.review.clone() else { return Ok(json!({"approvedBy": [], "changesRequestedBy": [], "merged": false, "closed": false})) };
        let t = transport(s)?;
        let c = Client { transport: t.as_ref(), token: token(s, &r.doc.forge)? };
        let st = c.review_state(&r.doc, review.number).map_err(ferr)?;
        Ok(json!({
            "review": {"number": review.number, "url": review.url, "branch": review.branch},
            "approvedBy": st.approved_by,
            "changesRequestedBy": st.changes_requested_by,
            "merged": st.merged,
            "closed": st.closed,
        }))
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
        let remote = Remote { sha, ..r };
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
