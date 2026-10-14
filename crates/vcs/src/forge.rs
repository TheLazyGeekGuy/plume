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
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
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
    /// Not signed in, or the token can't do this (401/403).
    Unauthorized,
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
            ForgeError::Unauthorized => f.write_str("sign in again: the forge refused the request"),
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
            401 | 403 => Err(ForgeError::Unauthorized),
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
