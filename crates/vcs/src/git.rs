//! A document's versions in a Git repository on disk, through the `git` program.
//!
//! Every call runs `git -C <root> …` with arguments (never a shell), `GIT_TERMINAL_PROMPT=0` (a
//! missing credential is an error, not a hang) and a `--` before paths. Revisions are checked to
//! be object ids; paths are made relative to the repository and refused if they leave it.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Version;

/// Longest error text kept from `git`'s standard error.
const MAX_ERR: usize = 600;
/// Most versions one history call lists.
pub const MAX_HISTORY: usize = 1_000;

/// Why a Git operation failed, in words for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitError {
    /// The `git` program isn't installed (or can't run).
    NoGit,
    /// The document isn't in a Git repository.
    NotARepository,
    /// A path outside the repository, or a malformed revision.
    BadInput(String),
    /// Nothing changed since the last version.
    NoChanges,
    /// The repository has no remote to synchronize with.
    NoRemote,
    /// The server's copy changed in a way that can't be merged automatically.
    Diverged(String),
    /// Anything else `git` reported.
    Failed(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NoGit => f.write_str("Git isn't installed: install Git to keep versions of documents"),
            GitError::NotARepository => f.write_str("this document isn't in a versioned folder (a Git repository)"),
            GitError::BadInput(e) => write!(f, "{e}"),
            GitError::NoChanges => f.write_str("nothing changed since the last version"),
            GitError::NoRemote => f.write_str("this repository has no server to synchronize with"),
            GitError::Diverged(e) => write!(f, "the server's copy has changes that can't be merged automatically: {e}"),
            GitError::Failed(e) => write!(f, "git: {e}"),
        }
    }
}

impl std::error::Error for GitError {}

/// A Git repository on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    root: PathBuf,
}

fn git() -> Command {
    let mut c = Command::new("git");
    c.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C").env("GIT_OPTIONAL_LOCKS", "0").stdin(Stdio::null());
    c
}

fn trim_err(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let s = s.trim();
    let mut out: String = s.chars().take(MAX_ERR).collect();
    if s.chars().count() > MAX_ERR {
        out.push('…');
    }
    out
}

/// A value for `-c user.name=…`: one line, no control characters.
fn config_value(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(200).collect::<String>().trim().to_string()
}

/// `git log` fields, separated by unit separators, records by record separators.
const LOG_FORMAT: &str = "--format=%H%x1f%an%x1f%ae%x1f%aI%x1f%s%x1e";

fn parse_log(out: &[u8]) -> Vec<Version> {
    String::from_utf8_lossy(out)
        .split('\u{1e}')
        .filter_map(|rec| {
            let mut f = rec.trim_start_matches('\n').split('\u{1f}');
            let id = f.next()?.trim().to_string();
            if !crate::is_object_id(&id) {
                return None;
            }
            Some(Version {
                id,
                author: f.next().unwrap_or("").to_string(),
                email: f.next().unwrap_or("").to_string(),
                date: f.next().unwrap_or("").to_string(),
                message: f.next().unwrap_or("").trim().to_string(),
            })
        })
        .take(MAX_HISTORY)
        .collect()
}

impl Repo {
    /// The repository holding `file` (which must exist), if any.
    pub fn discover(file: &Path) -> Result<Repo, GitError> {
        let dir = file.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let out = git().arg("-C").arg(dir).args(["rev-parse", "--show-toplevel"]).output().map_err(|_| GitError::NoGit)?;
        if !out.status.success() {
            return Err(GitError::NotARepository);
        }
        let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let root = std::fs::canonicalize(&root).map_err(|e| GitError::Failed(format!("{root}: {e}")))?;
        Ok(Repo { root })
    }

    /// Make `dir` a repository (a folder of documents starts keeping versions).
    pub fn init(dir: &Path) -> Result<Repo, GitError> {
        let dir = std::fs::canonicalize(dir).map_err(|e| GitError::BadInput(format!("{}: {e}", dir.display())))?;
        let out = git().arg("init").arg("-q").arg("--").arg(&dir).output().map_err(|_| GitError::NoGit)?;
        if !out.status.success() {
            return Err(GitError::Failed(trim_err(&out.stderr)));
        }
        Ok(Repo { root: dir })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn run(&self, args: &[OsString], stdin: Option<&[u8]>) -> Result<Vec<u8>, GitError> {
        let mut cmd = git();
        cmd.arg("-C").arg(&self.root).args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().map_err(|_| GitError::NoGit)?;
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(input).map_err(|e| GitError::Failed(e.to_string()))?;
        }
        let out = child.wait_with_output().map_err(|e| GitError::Failed(e.to_string()))?;
        if out.status.success() { Ok(out.stdout) } else { Err(GitError::Failed(trim_err(&out.stderr))) }
    }

    /// `file` relative to the repository root, with `/` separators. The file's folder must exist;
    /// the file itself may not (a picture about to be written).
    pub fn relative(&self, file: &Path) -> Result<String, GitError> {
        let name = file.file_name().ok_or_else(|| GitError::BadInput(format!("{}: not a file", file.display())))?;
        let dir = file.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let dir = std::fs::canonicalize(dir).map_err(|e| GitError::BadInput(format!("{}: {e}", dir.display())))?;
        let full = dir.join(name);
        let rel = full.strip_prefix(&self.root).map_err(|_| GitError::BadInput(format!("{}: outside the repository", file.display())))?;
        let mut parts = Vec::new();
        for c in rel.components() {
            match c {
                Component::Normal(p) => parts.push(p.to_string_lossy().to_string()),
                _ => return Err(GitError::BadInput(format!("{}: outside the repository", file.display()))),
            }
        }
        if parts.is_empty() {
            return Err(GitError::BadInput(format!("{}: not a file", file.display())));
        }
        Ok(parts.join("/"))
    }

    /// The versions of `file`, newest first (following renames), at most `limit`.
    pub fn history(&self, file: &Path, limit: usize) -> Result<Vec<Version>, GitError> {
        let rel = self.relative(file)?;
        let n = format!("-n{}", limit.clamp(1, MAX_HISTORY));
        let args: Vec<OsString> = vec!["log".into(), n.into(), "--follow".into(), LOG_FORMAT.into(), "--".into(), rel.into()];
        match self.run(&args, None) {
            Ok(out) => Ok(parse_log(&out)),
            // A repository with no commit yet has no history.
            Err(GitError::Failed(e)) if e.contains("does not have any commits") => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// The bytes of `rel` (relative to the root, `/` separators) in version `id`.
    pub fn read_at(&self, id: &str, rel: &str) -> Result<Vec<u8>, GitError> {
        if !crate::is_object_id(id) {
            return Err(GitError::BadInput(format!("{id:?} is not a version id")));
        }
        if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
            return Err(GitError::BadInput(format!("{rel:?}: not a path inside the repository")));
        }
        self.run(&["cat-file".into(), "blob".into(), format!("{id}:{rel}").into()], None)
    }

    /// The version before `id` (its first parent), if any.
    pub fn parent(&self, id: &str) -> Result<Option<String>, GitError> {
        if !crate::is_object_id(id) {
            return Err(GitError::BadInput(format!("{id:?} is not a version id")));
        }
        match self.run(&["rev-parse".into(), "--verify".into(), "-q".into(), format!("{id}^").into()], None) {
            Ok(out) => Ok(Some(String::from_utf8_lossy(&out).trim().to_string()).filter(|s| crate::is_object_id(s))),
            Err(_) => Ok(None),
        }
    }

    /// Does `file` differ from its last version (or is it not versioned yet)?
    pub fn is_modified(&self, file: &Path) -> Result<bool, GitError> {
        let rel = self.relative(file)?;
        let out = self.run(&["status".into(), "--porcelain".into(), "--".into(), rel.into()], None)?;
        Ok(!out.is_empty())
    }

    /// Save a version: commit `files` (the document and the pictures it uses) with `message`.
    /// `identity` (name, email) is used when given, and when Git has none configured.
    pub fn save_version(&self, files: &[PathBuf], message: &str, identity: Option<(&str, &str)>) -> Result<Version, GitError> {
        let rels = files.iter().map(|f| self.relative(f)).collect::<Result<Vec<_>, _>>()?;
        if rels.is_empty() {
            return Err(GitError::BadInput("no file to save".into()));
        }
        let mut add: Vec<OsString> = vec!["add".into(), "--".into()];
        add.extend(rels.iter().map(OsString::from));
        self.run(&add, None)?;
        let mut staged: Vec<OsString> = vec!["diff".into(), "--cached".into(), "--name-only".into(), "--".into()];
        staged.extend(rels.iter().map(OsString::from));
        if self.run(&staged, None)?.is_empty() {
            return Err(GitError::NoChanges);
        }
        let mut args: Vec<OsString> = Vec::new();
        let configured = |key: &str| self.run(&["config".into(), "--get".into(), key.into()], None).is_ok_and(|v| !v.is_empty());
        let (name, email) = identity.unwrap_or(("Plume", "plume@localhost"));
        if identity.is_some() || !configured("user.name") {
            args.extend(["-c".into(), format!("user.name={}", config_value(name)).into()]);
        }
        if identity.is_some() || !configured("user.email") {
            args.extend(["-c".into(), format!("user.email={}", config_value(email)).into()]);
        }
        args.extend(["commit".into(), "-q".into(), "--no-verify".into(), "-F".into(), "-".into(), "--".into()]);
        args.extend(rels.iter().map(OsString::from));
        let msg = crate::clean_message(message, "Update document");
        self.run(&args, Some(msg.as_bytes()))?;
        let out = self.run(&["log".into(), "-1".into(), LOG_FORMAT.into(), "HEAD".into()], None)?;
        parse_log(&out).into_iter().next().ok_or_else(|| GitError::Failed("the new version can't be read".into()))
    }

    /// The repository's remotes.
    pub fn remotes(&self) -> Result<Vec<String>, GitError> {
        let out = self.run(&["remote".into()], None)?;
        Ok(String::from_utf8_lossy(&out).lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect())
    }

    /// Synchronize: take the server's changes (fast-forward only: nothing is ever merged
    /// silently), then send this copy's versions.
    pub fn sync(&self) -> Result<(), GitError> {
        if self.remotes()?.is_empty() {
            return Err(GitError::NoRemote);
        }
        match self.run(&["pull".into(), "-q".into(), "--ff-only".into()], None) {
            Ok(_) => {}
            Err(GitError::Failed(e)) if e.contains("Not possible to fast-forward") || e.contains("diverge") => return Err(GitError::Diverged(e)),
            Err(e) => return Err(e),
        }
        self.run(&["push".into(), "-q".into()], None).map(|_| ())
    }
}
