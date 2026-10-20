//! A document in a repository on GitHub, GitLab or Forgejo, through their REST APIs: read it
//! (now or at a version), list its versions, save a version (one commit with the document and
//! its pictures), with the server's own concurrency check. No clone, no `git`: it works the same
//! on the desktop and in the browser.
//!
//! This module builds requests and reads responses; the app sends them ([`Transport`]: a native
//! HTTP client, or the browser's `fetch`). Responses are untrusted: sizes are capped, fields are
//! checked, and a malformed answer is an error, never a panic.
//!
//! Sign-in: a personal access token works everywhere. GitHub (and GitLab 17.2+) also offer the
//! OAuth device flow ([`device_start`], [`device_poll`]): the user types a short code on the
//! forge's site, no password ever reaches Plume.

use serde_json::{Value, json};

use crate::Version;

/// Largest file read through an API (bytes, after decoding).
pub const MAX_FILE: usize = 64 << 20;
/// Most versions listed.
pub const MAX_HISTORY: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    GitHub,
    GitLab,
    /// Forgejo, and Gitea (same API).
    Forgejo,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::GitHub => "GitHub",
            Kind::GitLab => "GitLab",
            Kind::Forgejo => "Forgejo",
        }
    }
}

/// A forge instance: its kind and the base URL of its API (`https://api.github.com`,
/// `https://gitlab.com/api/v4`, `https://codeberg.org/api/v1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forge {
    pub kind: Kind,
    pub api: String,
}

impl Forge {
    pub fn github() -> Forge {
        Forge { kind: Kind::GitHub, api: "https://api.github.com".into() }
    }
    pub fn gitlab(host: &str) -> Forge {
        Forge { kind: Kind::GitLab, api: format!("https://{}/api/v4", host.trim_end_matches('/')) }
    }
    pub fn forgejo(host: &str) -> Forge {
        Forge { kind: Kind::Forgejo, api: format!("https://{}/api/v1", host.trim_end_matches('/')) }
    }
}

/// One document in one repository: `owner/repo` (GitLab: the project's full path, groups
/// included), the branch, and the file's path in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocRef {
    pub forge: Forge,
    pub repo: String,
    pub branch: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

/// An HTTP request for the app's transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// JSON body, if any.
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Sends requests: the app's HTTP client (desktop) or `fetch` (browser).
pub trait Transport {
    fn send(&self, req: &Request) -> Result<Response, String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForgeError {
    /// Bad input (a path leaving the repository, an empty name…).
    BadInput(String),
    /// Not signed in: the token is wrong, expired or revoked (401).
    Unauthorized,
    /// The token works but may not do this: missing permissions (scopes), or no access to the
    /// repository (403); what the forge said.
    Forbidden(String),
    /// No such repository, branch or file (404).
    NotFound,
    /// Someone saved a version since this copy was read (409/412/422 on a stale sha): the
    /// changes must be resolved first.
    Conflict,
    /// The transport failed (offline, CORS…).
    Network(String),
    /// The server said something else, or something malformed.
    Server(u16, String),
}

impl std::fmt::Display for ForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForgeError::BadInput(e) => write!(f, "{e}"),
            ForgeError::Unauthorized => f.write_str("the forge didn't accept the token (wrong, expired or revoked): sign in again with a new one"),
            ForgeError::Forbidden(e) if e.is_empty() => f.write_str("the token isn't allowed to do this: give it the permissions Plume needs"),
            ForgeError::Forbidden(e) => write!(f, "the token isn't allowed to do this: give it the permissions Plume needs (the forge said: {e})"),
            ForgeError::NotFound => f.write_str("the repository, branch or document wasn't found"),
            ForgeError::Conflict => f.write_str("someone saved a newer version: get their changes first"),
            ForgeError::Network(e) => write!(f, "can't reach the forge: {e}"),
            ForgeError::Server(s, e) => write!(f, "the forge answered {s}: {e}"),
        }
    }
}

impl std::error::Error for ForgeError {}

/// Percent-encode everything but unreserved characters (`/` too, for GitLab's path segments).
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// [`encode_component`] keeping `/` (a path in a URL).
pub fn encode_path(s: &str) -> String {
    s.split('/').map(encode_component).collect::<Vec<_>>().join("/")
}

/// A repository path: relative, `/`-separated, no `.`/`..`/empty segments, no control characters.
pub fn check_path(path: &str) -> Result<(), ForgeError> {
    let ok = !path.is_empty()
        && path.len() <= 4096
        && !path.starts_with('/')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|p| !p.is_empty() && p != "." && p != "..");
    if ok { Ok(()) } else { Err(ForgeError::BadInput(format!("{path:?}: not a path inside the repository"))) }
}

fn check_doc(doc: &DocRef) -> Result<(), ForgeError> {
    check_path(&doc.path)?;
    let repo_ok = !doc.repo.is_empty() && doc.repo.split('/').count() >= 2 && check_path(&doc.repo).is_ok();
    if !repo_ok {
        return Err(ForgeError::BadInput(format!("{:?}: not an owner/repository name", doc.repo)));
    }
    if doc.branch.is_empty() || doc.branch.chars().any(|c| c.is_control() || c == ' ') || doc.branch.starts_with('-') {
        return Err(ForgeError::BadInput(format!("{:?}: not a branch name", doc.branch)));
    }
    if !doc.forge.api.starts_with("https://") && !doc.forge.api.starts_with("http://127.0.0.1") && !doc.forge.api.starts_with("http://localhost") {
        return Err(ForgeError::BadInput("the forge's address must use https".into()));
    }
    Ok(())
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk.first().copied().unwrap_or(0), chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (k, shift) in [18u32, 12, 6, 0].iter().enumerate() {
            if k <= chunk.len() {
                out.push(B64.get(((n >> shift) & 63) as usize).copied().unwrap_or(b'A') as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decode base64, ignoring whitespace (GitHub wraps lines). `None` on bad input or past `max`.
pub fn base64_decode(s: &str, max: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity((s.len() / 4 * 3).min(max));
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            if out.len() > max {
                return None;
            }
        }
    }
    Some(out)
}

/// The authorization header each forge expects for a token.
fn auth(kind: Kind, token: &str) -> (String, String) {
    let token: String = token.chars().filter(|c| !c.is_control() && !c.is_whitespace()).collect();
    match kind {
        Kind::GitHub | Kind::GitLab => ("Authorization".into(), format!("Bearer {token}")),
        Kind::Forgejo => ("Authorization".into(), format!("token {token}")),
    }
}

/// The reason a forge gives in an error answer (`message`, or OAuth's `error_description`):
/// one line, at most 200 characters.
fn server_message(text: &str) -> String {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let m = ["message", "error_description", "error"].iter().find_map(|k| v.get(*k).and_then(Value::as_str)).unwrap_or("");
    m.chars().map(|c| if c.is_control() { ' ' } else { c }).take(200).collect::<String>().trim().to_string()
}

/// A forge session: where, as whom, through what.
pub struct Client<'a> {
    pub transport: &'a dyn Transport,
    pub token: String,
}

impl Client<'_> {
    fn request(&self, kind: Kind, method: Method, url: String, body: Option<Value>) -> Request {
        let mut headers = vec![auth(kind, &self.token), ("Accept".into(), "application/json".into())];
        if kind == Kind::GitHub {
            headers.push(("X-GitHub-Api-Version".into(), "2022-11-28".into()));
        }
        if body.is_some() {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        Request { method, url, headers, body: body.map(|b| b.to_string()) }
    }

    fn call(&self, kind: Kind, method: Method, url: String, body: Option<Value>) -> Result<Value, ForgeError> {
        let req = self.request(kind, method, url, body);
        let resp = self.transport.send(&req).map_err(ForgeError::Network)?;
        if resp.body.len() > MAX_FILE * 2 {
            return Err(ForgeError::Server(resp.status, "answer too large".into()));
        }
        let text = String::from_utf8_lossy(&resp.body);
        match resp.status {
            200..=299 => {
                if text.trim().is_empty() {
                    Ok(Value::Null)
                } else {
                    serde_json::from_str(&text).map_err(|e| ForgeError::Server(resp.status, format!("malformed answer: {e}")))
                }
            }
            401 => Err(ForgeError::Unauthorized),
            403 => Err(ForgeError::Forbidden(server_message(&text))),
            404 => Err(ForgeError::NotFound),
            409 | 412 => Err(ForgeError::Conflict),
            // GitHub and Forgejo answer 422 for a stale `sha`.
            422 if text.contains("sha") || text.contains("does not match") => Err(ForgeError::Conflict),
            s => Err(ForgeError::Server(s, text.chars().take(300).collect())),
        }
    }

    fn repo_url(doc: &DocRef) -> String {
        match doc.forge.kind {
            Kind::GitHub | Kind::Forgejo => format!("{}/repos/{}", doc.forge.api, encode_path(&doc.repo)),
            Kind::GitLab => format!("{}/projects/{}", doc.forge.api, encode_component(&doc.repo)),
        }
    }

    /// The document's bytes on its branch (or at version `at`), and the blob id the server
    /// checks a later save against.
    pub fn read(&self, doc: &DocRef, at: Option<&str>) -> Result<(Vec<u8>, String), ForgeError> {
        check_doc(doc)?;
        self.read_path(doc, &doc.path, at)
    }

    /// [`Self::read`] for another file of the repository (a picture next to the document).
    pub fn read_path(&self, doc: &DocRef, path: &str, at: Option<&str>) -> Result<(Vec<u8>, String), ForgeError> {
        check_doc(doc)?;
        check_path(path)?;
        if let Some(a) = at
            && !crate::is_object_id(a)
        {
            return Err(ForgeError::BadInput(format!("{a:?} is not a version id")));
        }
        let rev = encode_component(at.unwrap_or(&doc.branch));
        let kind = doc.forge.kind;
        let url = match kind {
            Kind::GitHub | Kind::Forgejo => format!("{}/contents/{}?ref={rev}", Self::repo_url(doc), encode_path(path)),
            Kind::GitLab => format!("{}/repository/files/{}?ref={rev}", Self::repo_url(doc), encode_component(path)),
        };
        let v = self.call(kind, Method::Get, url, None)?;
        let content = v
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| ForgeError::Server(200, "no content (a folder, or a file too large for the API)".into()))?;
        let bytes = base64_decode(content, MAX_FILE).ok_or_else(|| ForgeError::Server(200, "bad or oversized content".into()))?;
        let sha = match kind {
            Kind::GitHub | Kind::Forgejo => v.get("sha"),
            Kind::GitLab => v.get("blob_id"),
        }
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
        Ok((bytes, sha))
    }

    /// The document's versions on its branch, newest first.
    pub fn history(&self, doc: &DocRef, limit: usize) -> Result<Vec<Version>, ForgeError> {
        check_doc(doc)?;
        let n = limit.clamp(1, MAX_HISTORY);
        let path = encode_component(&doc.path);
        let branch = encode_component(&doc.branch);
        let kind = doc.forge.kind;
        let url = match kind {
            Kind::GitHub => format!("{}/commits?path={path}&sha={branch}&per_page={n}", Self::repo_url(doc)),
            Kind::Forgejo => format!("{}/commits?path={path}&sha={branch}&limit={n}&stat=false&files=false", Self::repo_url(doc)),
            Kind::GitLab => format!("{}/repository/commits?path={path}&ref_name={branch}&per_page={n}", Self::repo_url(doc)),
        };
        let v = self.call(kind, Method::Get, url, None)?;
        let list = v.as_array().ok_or_else(|| ForgeError::Server(200, "expected a list of commits".into()))?;
        let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("").to_string();
        let out = list
            .iter()
            .take(n)
            .filter_map(|c| {
                let version = match kind {
                    Kind::GitHub | Kind::Forgejo => {
                        let a = c.pointer("/commit/author");
                        Version {
                            id: s(c.get("sha")),
                            author: s(a.and_then(|a| a.get("name"))),
                            email: s(a.and_then(|a| a.get("email"))),
                            date: s(a.and_then(|a| a.get("date"))),
                            message: s(c.pointer("/commit/message")).lines().next().unwrap_or("").to_string(),
                        }
                    }
                    Kind::GitLab => Version {
                        id: s(c.get("id")),
                        author: s(c.get("author_name")),
                        email: s(c.get("author_email")),
                        date: s(c.get("authored_date")),
                        message: s(c.get("title")),
                    },
                };
                crate::is_object_id(&version.id).then_some(version)
            })
            .collect();
        Ok(out)
    }

    /// The blob id of `path` on the document's branch, or `None` when it doesn't exist.
    fn blob_id(&self, doc: &DocRef, path: &str) -> Result<Option<String>, ForgeError> {
        match self.read_path(doc, path, None) {
            Ok((_, sha)) => Ok(Some(sha)),
            Err(ForgeError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Save a version: one commit on the document's branch with the document (`text`) and the
    /// new pictures (`assets`: repository path → bytes; pictures already there are skipped, their
    /// names being their content). `expected` is the document's blob id when it was read: if the
    /// server's copy changed since, the save fails with [`ForgeError::Conflict`].
    pub fn save_version(
        &self,
        doc: &DocRef,
        text: &[u8],
        assets: &[(String, Vec<u8>)],
        message: &str,
        expected: Option<&str>,
    ) -> Result<String, ForgeError> {
        check_doc(doc)?;
        for (p, _) in assets {
            check_path(p)?;
        }
        let message = crate::clean_message(message, "Update document");
        // The server's copy must still be the one this edit started from.
        let current = self.blob_id(doc, &doc.path)?;
        if let Some(expected) = expected
            && current.as_deref() != Some(expected)
        {
            return Err(ForgeError::Conflict);
        }
        let mut new_assets = Vec::new();
        for (p, b) in assets {
            if self.blob_id(doc, p)?.is_none() {
                new_assets.push((p.as_str(), b.as_slice()));
            }
        }
        match doc.forge.kind {
            Kind::GitHub => self.save_github(doc, text, &new_assets, &message),
            Kind::GitLab => self.save_gitlab(doc, text, &new_assets, &message, current.is_some()),
            Kind::Forgejo => self.save_forgejo(doc, text, &new_assets, &message, current.as_deref()),
        }
    }

    /// GitHub: blobs, a tree on the branch's tree, a commit, then move the branch (a stale
    /// branch makes the last step fail: someone saved meanwhile).
    fn save_github(&self, doc: &DocRef, text: &[u8], assets: &[(&str, &[u8])], message: &str) -> Result<String, ForgeError> {
        let base = Self::repo_url(doc);
        let k = Kind::GitHub;
        let r = self.call(k, Method::Get, format!("{base}/git/ref/heads/{}", encode_path(&doc.branch)), None)?;
        let head = r
            .pointer("/object/sha")
            .and_then(Value::as_str)
            .filter(|s| crate::is_object_id(s))
            .ok_or_else(|| ForgeError::Server(200, "no branch head".into()))?
            .to_string();
        let c = self.call(k, Method::Get, format!("{base}/git/commits/{head}"), None)?;
        let base_tree = c
            .pointer("/tree/sha")
            .and_then(Value::as_str)
            .filter(|s| crate::is_object_id(s))
            .ok_or_else(|| ForgeError::Server(200, "no tree".into()))?
            .to_string();
        let mut tree = Vec::new();
        for (path, bytes) in std::iter::once((doc.path.as_str(), text)).chain(assets.iter().copied()) {
            let b = self.call(k, Method::Post, format!("{base}/git/blobs"), Some(json!({"content": base64_encode(bytes), "encoding": "base64"})))?;
            let sha = b
                .get("sha")
                .and_then(Value::as_str)
                .filter(|s| crate::is_object_id(s))
                .ok_or_else(|| ForgeError::Server(200, "no blob id".into()))?;
            tree.push(json!({"path": path, "mode": "100644", "type": "blob", "sha": sha}));
        }
        let t = self.call(k, Method::Post, format!("{base}/git/trees"), Some(json!({"base_tree": base_tree, "tree": tree})))?;
        let tree_sha = t.get("sha").and_then(Value::as_str).ok_or_else(|| ForgeError::Server(200, "no tree id".into()))?;
        let commit =
            self.call(k, Method::Post, format!("{base}/git/commits"), Some(json!({"message": message, "tree": tree_sha, "parents": [head]})))?;
        let id = commit
            .get("sha")
            .and_then(Value::as_str)
            .filter(|s| crate::is_object_id(s))
            .ok_or_else(|| ForgeError::Server(200, "no commit id".into()))?
            .to_string();
        self.call(k, Method::Patch, format!("{base}/git/refs/heads/{}", encode_path(&doc.branch)), Some(json!({"sha": id, "force": false})))?;
        Ok(id)
    }

    /// GitLab: one commit with an action per file.
    fn save_gitlab(&self, doc: &DocRef, text: &[u8], assets: &[(&str, &[u8])], message: &str, exists: bool) -> Result<String, ForgeError> {
        let mut actions = vec![json!({
            "action": if exists { "update" } else { "create" },
            "file_path": doc.path, "content": base64_encode(text), "encoding": "base64",
        })];
        for (p, b) in assets {
            actions.push(json!({"action": "create", "file_path": p, "content": base64_encode(b), "encoding": "base64"}));
        }
        let body = json!({"branch": doc.branch, "commit_message": message, "actions": actions});
        let v = self.call(Kind::GitLab, Method::Post, format!("{}/repository/commits", Self::repo_url(doc)), Some(body))?;
        v.get("id")
            .and_then(Value::as_str)
            .filter(|s| crate::is_object_id(s))
            .map(str::to_string)
            .ok_or_else(|| ForgeError::Server(200, "no commit id".into()))
    }

    /// Forgejo: one commit through the multi-file contents endpoint.
    fn save_forgejo(&self, doc: &DocRef, text: &[u8], assets: &[(&str, &[u8])], message: &str, sha: Option<&str>) -> Result<String, ForgeError> {
        let mut files = vec![match sha {
            Some(sha) => json!({"operation": "update", "path": doc.path, "content": base64_encode(text), "sha": sha}),
            None => json!({"operation": "create", "path": doc.path, "content": base64_encode(text)}),
        }];
        for (p, b) in assets {
            files.push(json!({"operation": "create", "path": p, "content": base64_encode(b)}));
        }
        let body = json!({"branch": doc.branch, "message": message, "files": files});
        let v = self.call(Kind::Forgejo, Method::Post, format!("{}/contents", Self::repo_url(doc)), Some(body))?;
        v.pointer("/commit/sha")
            .and_then(Value::as_str)
            .filter(|s| crate::is_object_id(s))
            .map(str::to_string)
            .ok_or_else(|| ForgeError::Server(200, "no commit id".into()))
    }
}

/// A repository the signed-in user can open: its full name and default branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoInfo {
    pub name: String,
    pub branch: String,
    pub description: String,
}

/// Most repositories listed, and most files listed from one repository.
pub const MAX_REPOS: usize = 1_000;
pub const MAX_FILES: usize = 5_000;
/// Pages read for one list.
const MAX_PAGES: usize = 10;

impl Client<'_> {
    /// The repositories the signed-in user owns or belongs to, most recently active first.
    pub fn list_repos(&self, forge: &Forge) -> Result<Vec<RepoInfo>, ForgeError> {
        let k = forge.kind;
        let mut out = Vec::new();
        for page in 1..=MAX_PAGES {
            let url = match k {
                Kind::GitHub => format!("{}/user/repos?per_page=100&sort=updated&page={page}", forge.api),
                Kind::GitLab => format!("{}/projects?membership=true&simple=true&order_by=last_activity_at&per_page=100&page={page}", forge.api),
                Kind::Forgejo => format!("{}/user/repos?limit=50&page={page}", forge.api),
            };
            let list = self.call(k, Method::Get, url, None)?.as_array().cloned().unwrap_or_default();
            let n = list.len();
            for r in list {
                let name = match k {
                    Kind::GitLab => r.get("path_with_namespace"),
                    _ => r.get("full_name"),
                }
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
                if name.is_empty() || r.get("archived").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                let branch = r.get("default_branch").and_then(Value::as_str).filter(|b| check_branch(b).is_ok()).unwrap_or("main").to_string();
                let description: String =
                    r.get("description").and_then(Value::as_str).unwrap_or("").chars().filter(|c| !c.is_control()).take(200).collect();
                out.push(RepoInfo { name, branch, description });
            }
            if n == 0 || out.len() >= MAX_REPOS || n < if k == Kind::Forgejo { 50 } else { 100 } {
                break;
            }
        }
        out.truncate(MAX_REPOS);
        Ok(out)
    }

    /// The Markdown files of `repo` on `branch`, by path.
    pub fn list_markdown(&self, forge: &Forge, repo: &str, branch: &str) -> Result<Vec<String>, ForgeError> {
        check_branch(branch)?;
        let doc = DocRef { forge: forge.clone(), repo: repo.to_string(), branch: branch.to_string(), path: "x.md".into() };
        check_doc(&doc)?;
        let base = Self::repo_url(&doc);
        let k = forge.kind;
        let is_md = |p: &str| {
            let l = p.to_ascii_lowercase();
            (l.ends_with(".md") || l.ends_with(".markdown")) && check_path(p).is_ok()
        };
        let mut out = Vec::new();
        match k {
            Kind::GitHub | Kind::Forgejo => {
                let url = match k {
                    Kind::GitHub => format!("{base}/git/trees/{}?recursive=1", encode_path(branch)),
                    _ => format!("{base}/git/trees/{}?recursive=true&per_page=10000", encode_path(branch)),
                };
                let v = self.call(k, Method::Get, url, None)?;
                for e in v.get("tree").and_then(Value::as_array).cloned().unwrap_or_default() {
                    if e.get("type").and_then(Value::as_str) == Some("blob")
                        && let Some(p) = e.get("path").and_then(Value::as_str)
                        && is_md(p)
                    {
                        out.push(p.to_string());
                    }
                }
            }
            Kind::GitLab => {
                for page in 1..=MAX_PAGES {
                    let url = format!("{base}/repository/tree?recursive=true&per_page=100&page={page}&ref={}", encode_component(branch));
                    let list = self.call(k, Method::Get, url, None)?.as_array().cloned().unwrap_or_default();
                    let n = list.len();
                    for e in list {
                        if e.get("type").and_then(Value::as_str) == Some("blob")
                            && let Some(p) = e.get("path").and_then(Value::as_str)
                            && is_md(p)
                        {
                            out.push(p.to_string());
                        }
                    }
                    if n < 100 {
                        break;
                    }
                }
            }
        }
        out.sort();
        out.truncate(MAX_FILES);
        Ok(out)
    }
}

/// A review request: a pull request (GitHub, Forgejo) or merge request (GitLab).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    /// The pull request number, or the merge request's project-wide iid.
    pub number: u64,
    /// Its page on the forge.
    pub url: String,
    /// The branch the reviewed versions are saved on.
    pub branch: String,
}

/// A reviewer's comment: on a line of the document (`path`, `line`), or on the review as a whole.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewComment {
    /// The forge's id for it (stable: comments already read aren't read twice).
    pub id: String,
    pub author: String,
    pub date: String,
    pub body: String,
    pub path: Option<String>,
    /// 1-based line of the file, when the forge says it.
    pub line: Option<u32>,
    /// Where a reply to it goes.
    pub reply_to: ReplyTo,
}

/// Where a reply to a reviewer's comment goes.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplyTo {
    /// A new comment on the review's conversation (comments on the review as a whole; Forgejo's
    /// line comments, which take no replies through its API).
    #[default]
    Conversation,
    /// GitHub: the thread under this line comment (the thread's first comment).
    Thread(String),
    /// GitLab: this discussion.
    Discussion(String),
}

/// A branch's newest review: where its head was, and whether it was merged or is still open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchReview {
    pub review: Review,
    /// The branch's head when the review was last updated (merged: what went in).
    pub head: String,
    pub merged: bool,
    pub open: bool,
}

/// A review from the forge's answer for it (number and page), on `branch`.
fn review_of(k: Kind, v: &Value, branch: &str) -> Option<Review> {
    let number = match k {
        Kind::GitLab => v.get("iid"),
        _ => v.get("number"),
    }
    .and_then(Value::as_u64)?;
    let url = match k {
        Kind::GitLab => v.get("web_url"),
        _ => v.get("html_url"),
    }
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_string();
    Some(Review { number, url, branch: branch.to_string() })
}

/// Plume's workflow file: in a repository whose main branch is locked, where each person's work
/// goes (see the engine's `workflow`).
pub const WORKFLOW_FILE: &str = ".plume/workflow.toml";

/// Where a review stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewState {
    pub approved_by: Vec<String>,
    pub changes_requested_by: Vec<String>,
    /// Merged (published) or closed.
    pub merged: bool,
    pub closed: bool,
}

/// Most comments read from one review.
pub const MAX_COMMENTS: usize = 2_000;
/// The longest reply sent (GitHub takes 65,536 characters).
pub const MAX_REPLY: usize = 65_000;

/// A branch name made of safe characters only.
pub fn check_branch(branch: &str) -> Result<(), ForgeError> {
    let ok = !branch.is_empty()
        && branch.len() <= 200
        && !branch.starts_with(['-', '/', '.'])
        && !branch.ends_with(['/', '.'])
        && !branch.contains("..")
        && branch.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.'));
    if ok { Ok(()) } else { Err(ForgeError::BadInput(format!("{branch:?}: not a branch name"))) }
}

impl Client<'_> {
    fn branch_head(&self, doc: &DocRef) -> Result<String, ForgeError> {
        let base = Self::repo_url(doc);
        let (url, ptr) = match doc.forge.kind {
            Kind::GitHub => (format!("{base}/git/ref/heads/{}", encode_path(&doc.branch)), "/object/sha"),
            Kind::GitLab => (format!("{base}/repository/branches/{}", encode_component(&doc.branch)), "/commit/id"),
            Kind::Forgejo => (format!("{base}/branches/{}", encode_path(&doc.branch)), "/commit/id"),
        };
        let v = self.call(doc.forge.kind, Method::Get, url, None)?;
        v.pointer(ptr)
            .and_then(Value::as_str)
            .filter(|s| crate::is_object_id(s))
            .map(str::to_string)
            .ok_or_else(|| ForgeError::Server(200, "no branch head".into()))
    }

    /// Open a review request: a new branch `branch` from the document's branch, then a pull or
    /// merge request from it into the document's branch. Versions saved on `branch` (a
    /// [`DocRef`] with that branch) are what reviewers see.
    pub fn request_review(&self, doc: &DocRef, branch: &str, title: &str, body: &str) -> Result<Review, ForgeError> {
        self.create_branch(doc, branch)?;
        self.open_review(doc, branch, title, body)
    }

    /// A new branch `branch` from the head of the document's branch.
    pub fn create_branch(&self, doc: &DocRef, branch: &str) -> Result<(), ForgeError> {
        check_doc(doc)?;
        check_branch(branch)?;
        let base = Self::repo_url(doc);
        let k = doc.forge.kind;
        match k {
            Kind::GitHub => {
                let head = self.branch_head(doc)?;
                self.call(k, Method::Post, format!("{base}/git/refs"), Some(json!({"ref": format!("refs/heads/{branch}"), "sha": head})))?;
            }
            Kind::GitLab => {
                self.call(
                    k,
                    Method::Post,
                    format!("{base}/repository/branches?branch={}&ref={}", encode_component(branch), encode_component(&doc.branch)),
                    None,
                )?;
            }
            Kind::Forgejo => {
                self.call(k, Method::Post, format!("{base}/branches"), Some(json!({"new_branch_name": branch, "old_branch_name": doc.branch})))?;
            }
        }
        Ok(())
    }

    /// Whether the repository has branch `branch`.
    pub fn branch_exists(&self, doc: &DocRef, branch: &str) -> Result<bool, ForgeError> {
        check_branch(branch)?;
        match self.branch_head(&DocRef { branch: branch.to_string(), ..doc.clone() }) {
            Ok(_) => Ok(true),
            Err(ForgeError::NotFound) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Branch `branch` starts again from the head of the document's branch: after its review
    /// was merged there. The caller makes sure nothing on it is left unmerged.
    pub fn reset_branch(&self, doc: &DocRef, branch: &str) -> Result<(), ForgeError> {
        check_doc(doc)?;
        check_branch(branch)?;
        if branch == doc.branch {
            return Err(ForgeError::BadInput("a branch can't start again from itself".into()));
        }
        let base = Self::repo_url(doc);
        let k = doc.forge.kind;
        match k {
            Kind::GitHub => {
                let head = self.branch_head(doc)?;
                self.call(k, Method::Patch, format!("{base}/git/refs/heads/{}", encode_path(branch)), Some(json!({"sha": head, "force": true})))?;
                Ok(())
            }
            Kind::GitLab => {
                self.call(k, Method::Delete, format!("{base}/repository/branches/{}", encode_component(branch)), None)?;
                self.create_branch(doc, branch)
            }
            Kind::Forgejo => {
                self.call(k, Method::Delete, format!("{base}/branches/{}", encode_path(branch)), None)?;
                self.create_branch(doc, branch)
            }
        }
    }

    /// A pull or merge request from `branch` (which exists) into the document's branch.
    pub fn open_review(&self, doc: &DocRef, branch: &str, title: &str, body: &str) -> Result<Review, ForgeError> {
        check_doc(doc)?;
        check_branch(branch)?;
        let base = Self::repo_url(doc);
        let k = doc.forge.kind;
        let title = crate::clean_message(title, "Review request");
        let v = match k {
            Kind::GitHub | Kind::Forgejo => {
                self.call(k, Method::Post, format!("{base}/pulls"), Some(json!({"title": title, "head": branch, "base": doc.branch, "body": body})))?
            }
            Kind::GitLab => self.call(
                k,
                Method::Post,
                format!("{base}/merge_requests"),
                Some(json!({"source_branch": branch, "target_branch": doc.branch, "title": title, "description": body})),
            )?,
        };
        review_of(k, &v, branch).ok_or_else(|| ForgeError::Server(200, "no review number".into()))
    }

    /// The newest review from `branch` into the document's branch, open or not.
    pub fn branch_review(&self, doc: &DocRef, branch: &str) -> Result<Option<BranchReview>, ForgeError> {
        check_doc(doc)?;
        check_branch(branch)?;
        let base = Self::repo_url(doc);
        let k = doc.forge.kind;
        let s = |v: &Value, p: &str| v.pointer(p).and_then(Value::as_str).unwrap_or("").to_string();
        let list = match k {
            Kind::GitHub => {
                let owner = doc.repo.split('/').next().unwrap_or("");
                let url = format!(
                    "{base}/pulls?state=all&head={}&base={}&sort=created&direction=desc&per_page=20",
                    encode_component(&format!("{owner}:{branch}")),
                    encode_component(&doc.branch)
                );
                self.call(k, Method::Get, url, None)?
            }
            Kind::Forgejo => self.call(k, Method::Get, format!("{base}/pulls?state=all&sort=newest&limit=50"), None)?,
            Kind::GitLab => {
                let url = format!(
                    "{base}/merge_requests?state=all&source_branch={}&target_branch={}&order_by=created_at&sort=desc&per_page=20",
                    encode_component(branch),
                    encode_component(&doc.branch)
                );
                self.call(k, Method::Get, url, None)?
            }
        };
        let newest = list.as_array().into_iter().flatten().find(|v| match k {
            Kind::GitLab => s(v, "/source_branch") == branch && s(v, "/target_branch") == doc.branch,
            _ => s(v, "/head/ref") == branch && s(v, "/base/ref") == doc.branch,
        });
        Ok(newest.and_then(|v| {
            let review = review_of(k, v, branch)?;
            let (head, merged, open) = match k {
                Kind::GitLab => (s(v, "/sha"), s(v, "/state") == "merged", s(v, "/state") == "opened"),
                Kind::GitHub => (s(v, "/head/sha"), v.get("merged_at").is_some_and(|m| !m.is_null()), s(v, "/state") == "open"),
                Kind::Forgejo => (s(v, "/head/sha"), v.get("merged").and_then(Value::as_bool).unwrap_or(false), s(v, "/state") == "open"),
            };
            Some(BranchReview { review, head, merged, open })
        }))
    }

    /// The signed-in user's name on the forge (their login).
    pub fn current_user(&self, forge: &Forge) -> Result<String, ForgeError> {
        let v = self.call(forge.kind, Method::Get, format!("{}/user", forge.api), None)?;
        let key = if forge.kind == Kind::GitLab { "username" } else { "login" };
        v.get(key)
            .and_then(Value::as_str)
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| ForgeError::Server(200, "no user name".into()))
    }

    /// The head of branch `branch` of the document's repository.
    pub fn head_of(&self, doc: &DocRef, branch: &str) -> Result<String, ForgeError> {
        check_branch(branch)?;
        self.branch_head(&DocRef { branch: branch.to_string(), ..doc.clone() })
    }

    /// The comments on review `number`: line comments on the document and general ones, oldest
    /// first.
    pub fn review_comments(&self, doc: &DocRef, number: u64) -> Result<Vec<ReviewComment>, ForgeError> {
        check_doc(doc)?;
        let base = Self::repo_url(doc);
        let k = doc.forge.kind;
        let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("").to_string();
        let id = |v: Option<&Value>| {
            v.map(|x| x.as_u64().map(|n| n.to_string()).unwrap_or_else(|| x.as_str().unwrap_or("").to_string())).unwrap_or_default()
        };
        let line = |v: Option<&Value>| v.and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0);
        let list = |v: Value| v.as_array().cloned().unwrap_or_default();
        let mut out = Vec::new();
        match k {
            Kind::GitHub | Kind::Forgejo => {
                for c in list(self.call(k, Method::Get, format!("{base}/issues/{number}/comments?per_page=100&limit=50"), None)?) {
                    out.push(ReviewComment {
                        id: id(c.get("id")),
                        author: s(c.pointer("/user/login")),
                        date: s(c.get("created_at")),
                        body: s(c.get("body")),
                        path: None,
                        line: None,
                        reply_to: ReplyTo::Conversation,
                    });
                }
                if k == Kind::GitHub {
                    for c in list(self.call(k, Method::Get, format!("{base}/pulls/{number}/comments?per_page=100"), None)?) {
                        out.push(ReviewComment {
                            id: id(c.get("id")),
                            author: s(c.pointer("/user/login")),
                            date: s(c.get("created_at")),
                            body: s(c.get("body")),
                            path: c.get("path").and_then(Value::as_str).map(str::to_string),
                            line: line(c.get("line")).or_else(|| line(c.get("original_line"))),
                            reply_to: ReplyTo::Thread({
                                let root = id(c.get("in_reply_to_id"));
                                if root.is_empty() { id(c.get("id")) } else { root }
                            }),
                        });
                    }
                } else {
                    for r in list(self.call(k, Method::Get, format!("{base}/pulls/{number}/reviews"), None)?).iter().take(100) {
                        let rid = id(r.get("id"));
                        if !s(r.get("body")).trim().is_empty() {
                            out.push(ReviewComment {
                                id: format!("r{rid}"),
                                author: s(r.pointer("/user/login")),
                                date: s(r.get("submitted_at")),
                                body: s(r.get("body")),
                                path: None,
                                line: None,
                                reply_to: ReplyTo::Conversation,
                            });
                        }
                        if rid.is_empty() || !rid.bytes().all(|b| b.is_ascii_digit()) {
                            continue;
                        }
                        for c in list(self.call(k, Method::Get, format!("{base}/pulls/{number}/reviews/{rid}/comments"), None)?) {
                            out.push(ReviewComment {
                                id: id(c.get("id")),
                                author: s(c.pointer("/user/login")),
                                date: s(c.get("created_at")),
                                body: s(c.get("body")),
                                path: c.get("path").and_then(Value::as_str).map(str::to_string),
                                line: line(c.get("position")).or_else(|| line(c.get("original_position"))),
                                reply_to: ReplyTo::Conversation,
                            });
                        }
                    }
                }
            }
            Kind::GitLab => {
                for d in list(self.call(k, Method::Get, format!("{base}/merge_requests/{number}/discussions?per_page=100"), None)?) {
                    for n in d.get("notes").and_then(Value::as_array).cloned().unwrap_or_default() {
                        if n.get("system").and_then(Value::as_bool) == Some(true) {
                            continue;
                        }
                        out.push(ReviewComment {
                            id: id(n.get("id")),
                            author: s(n.pointer("/author/name")),
                            date: s(n.get("created_at")),
                            body: s(n.get("body")),
                            path: n.pointer("/position/new_path").and_then(Value::as_str).map(str::to_string),
                            line: line(n.pointer("/position/new_line")),
                            reply_to: ReplyTo::Discussion(s(d.get("id"))),
                        });
                    }
                }
            }
        }
        out.retain(|c| !c.id.is_empty() && !c.body.trim().is_empty());
        out.truncate(MAX_COMMENTS);
        out.sort_by(|a, b| a.date.cmp(&b.date));
        Ok(out)
    }

    /// Post `body` as a reply on review `number`, where `to` says; the new comment's id.
    pub fn reply(&self, doc: &DocRef, number: u64, to: &ReplyTo, body: &str) -> Result<String, ForgeError> {
        check_doc(doc)?;
        if body.trim().is_empty() || body.len() > MAX_REPLY {
            return Err(ForgeError::BadInput("a reply is 1 to 65,000 bytes of text".into()));
        }
        // Ids go into the address: only the forges' own (digits; GitLab's discussions, hex).
        let safe = |id: &str| !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric());
        let base = Self::repo_url(doc);
        let k = doc.forge.kind;
        let url = match (k, to) {
            (Kind::GitHub, ReplyTo::Thread(id)) if safe(id) => format!("{base}/pulls/{number}/comments/{id}/replies"),
            (Kind::GitLab, ReplyTo::Discussion(id)) if safe(id) => format!("{base}/merge_requests/{number}/discussions/{id}/notes"),
            (Kind::GitLab, _) => format!("{base}/merge_requests/{number}/notes"),
            (_, ReplyTo::Conversation) | (Kind::Forgejo, _) => format!("{base}/issues/{number}/comments"),
            _ => return Err(ForgeError::BadInput("this comment can't be answered".into())),
        };
        let v = self.call(k, Method::Post, url, Some(json!({"body": body})))?;
        let id = v.get("id").map(|x| x.as_u64().map(|n| n.to_string()).unwrap_or_else(|| x.as_str().unwrap_or("").to_string())).unwrap_or_default();
        Ok(id)
    }

    /// Who approved review `number`, who asked for changes, and whether it was merged.
    pub fn review_state(&self, doc: &DocRef, number: u64) -> Result<ReviewState, ForgeError> {
        check_doc(doc)?;
        let base = Self::repo_url(doc);
        let k = doc.forge.kind;
        let mut st = ReviewState::default();
        match k {
            Kind::GitHub | Kind::Forgejo => {
                let pr = self.call(k, Method::Get, format!("{base}/pulls/{number}"), None)?;
                st.merged = pr.get("merged").and_then(Value::as_bool).unwrap_or(false);
                st.closed = pr.get("state").and_then(Value::as_str) == Some("closed");
                // The latest review per reviewer counts.
                let mut latest: std::collections::BTreeMap<String, String> = Default::default();
                for r in self.call(k, Method::Get, format!("{base}/pulls/{number}/reviews"), None)?.as_array().cloned().unwrap_or_default() {
                    let who = r.pointer("/user/login").and_then(Value::as_str).unwrap_or("").to_string();
                    let state = r.get("state").and_then(Value::as_str).unwrap_or("").to_string();
                    if !who.is_empty() && state != "COMMENTED" && state != "COMMENT" {
                        latest.insert(who, state);
                    }
                }
                for (who, state) in latest {
                    match state.as_str() {
                        "APPROVED" => st.approved_by.push(who),
                        "CHANGES_REQUESTED" | "REQUEST_CHANGES" => st.changes_requested_by.push(who),
                        _ => {}
                    }
                }
            }
            Kind::GitLab => {
                let mr = self.call(k, Method::Get, format!("{base}/merge_requests/{number}"), None)?;
                let state = mr.get("state").and_then(Value::as_str).unwrap_or("");
                st.merged = state == "merged";
                st.closed = state == "closed";
                let a = self.call(k, Method::Get, format!("{base}/merge_requests/{number}/approvals"), None)?;
                for u in a.get("approved_by").and_then(Value::as_array).cloned().unwrap_or_default() {
                    if let Some(n) = u.pointer("/user/name").and_then(Value::as_str) {
                        st.approved_by.push(n.to_string());
                    }
                }
            }
        }
        Ok(st)
    }
}

/// An OAuth device-flow sign-in in progress: show `user_code` and `verification_uri`, then
/// call [`device_poll`] every `interval` seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

/// Where a forge's device flow lives: GitHub's site, or a GitLab instance (`https://host`).
fn device_urls(kind: Kind, site: &str) -> Option<(String, String)> {
    let site = site.trim_end_matches('/');
    match kind {
        Kind::GitHub => Some(("https://github.com/login/device/code".into(), "https://github.com/login/oauth/access_token".into())),
        Kind::GitLab => Some((format!("{site}/oauth/authorize_device"), format!("{site}/oauth/token"))),
        Kind::Forgejo => None,
    }
}

/// Start a device-flow sign-in with the app's OAuth `client_id`. Forgejo has no device flow
/// (use a token).
pub fn device_start(t: &dyn Transport, kind: Kind, site: &str, client_id: &str, scope: &str) -> Result<DeviceCode, ForgeError> {
    let (code_url, _) =
        device_urls(kind, site).ok_or_else(|| ForgeError::BadInput(format!("{} has no device sign-in: use an access token", kind.name())))?;
    let body = json!({"client_id": client_id, "scope": scope});
    let req = Request {
        method: Method::Post,
        url: code_url,
        headers: vec![("Accept".into(), "application/json".into()), ("Content-Type".into(), "application/json".into())],
        body: Some(body.to_string()),
    };
    let resp = t.send(&req).map_err(ForgeError::Network)?;
    let v: Value = serde_json::from_slice(&resp.body).map_err(|e| ForgeError::Server(resp.status, e.to_string()))?;
    if !(200..300).contains(&resp.status) {
        return Err(ForgeError::Server(resp.status, v.get("error").and_then(Value::as_str).unwrap_or("").to_string()));
    }
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| ForgeError::Server(resp.status, format!("no {k}")));
    let verification_uri = s("verification_uri")?;
    if !verification_uri.starts_with("https://") {
        return Err(ForgeError::Server(resp.status, "the sign-in page isn't https".into()));
    }
    Ok(DeviceCode {
        device_code: s("device_code")?,
        user_code: s("user_code")?,
        verification_uri,
        interval: v.get("interval").and_then(Value::as_u64).unwrap_or(5).clamp(1, 60),
    })
}

/// Poll a device-flow sign-in: `Ok(Some(token))` once the user approved, `Ok(None)` while
/// pending (wait `interval` seconds, longer after a slow-down), an error if refused or expired.
pub fn device_poll(t: &dyn Transport, kind: Kind, site: &str, client_id: &str, code: &DeviceCode) -> Result<Option<String>, ForgeError> {
    let (_, token_url) = device_urls(kind, site).ok_or_else(|| ForgeError::BadInput(format!("{} has no device sign-in", kind.name())))?;
    let body = json!({"client_id": client_id, "device_code": code.device_code, "grant_type": "urn:ietf:params:oauth:grant-type:device_code"});
    let req = Request {
        method: Method::Post,
        url: token_url,
        headers: vec![("Accept".into(), "application/json".into()), ("Content-Type".into(), "application/json".into())],
        body: Some(body.to_string()),
    };
    let resp = t.send(&req).map_err(ForgeError::Network)?;
    let v: Value = serde_json::from_slice(&resp.body).map_err(|e| ForgeError::Server(resp.status, e.to_string()))?;
    if let Some(token) = v.get("access_token").and_then(Value::as_str) {
        return Ok(Some(token.to_string()));
    }
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending" | "slow_down") => Ok(None),
        Some("access_denied") => Err(ForgeError::Unauthorized),
        Some(e) => Err(ForgeError::Server(resp.status, e.to_string())),
        None => Err(ForgeError::Server(resp.status, "no token".into())),
    }
}
