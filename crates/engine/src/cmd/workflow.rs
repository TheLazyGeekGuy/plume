//! Plume: the work-branch workflow of a repository whose main branch is locked. Its admins put
//! `.plume/workflow.toml` on that branch (only they can change it, through a merge request, since
//! the branch is locked):
//!
//! ```toml
//! # Each person works on their own branch; main takes changes through merge requests.
//! required = true
//! target = "main"            # the locked branch (default: the branch documents are opened on)
//! branch = "{user}-{repo}"   # {user}: the forge login, {repo}: the repository's name
//! ```
//!
//! A document opened (or saved) on the target branch then opens on the person's work branch:
//! created from the target when it doesn't exist, started again from it when its last merge
//! request was merged with nothing saved since. Versions are saved there; Request Review opens
//! the merge request into the target.

use wordcraft_vcs::forge::{BranchReview, Client, DocRef, ForgeError, Review, WORKFLOW_FILE, check_branch};

use crate::CmdError;

/// The workflow a forge document follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workflow {
    /// The locked branch merge requests go into.
    pub target: String,
    /// This person's work branch, where versions are saved.
    pub branch: String,
    /// The work branch was created, or started again from the target, when the document opened.
    pub fresh: bool,
}

/// What `.plume/workflow.toml` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub required: bool,
    pub target: Option<String>,
    pub branch: String,
}

impl Default for Config {
    fn default() -> Self {
        Config { required: true, target: None, branch: "{user}-{repo}".into() }
    }
}

/// Largest workflow file read.
const MAX_CONFIG: usize = 16 * 1024;

impl Config {
    /// Read the file's `key = value` lines (strings, booleans, `#` comments); anything else is
    /// ignored, so a hand-edited file never stops a document from opening.
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines().take(500) {
            let line = line.split(" #").next().unwrap_or("").trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            let text = v.strip_prefix('"').and_then(|r| r.strip_suffix('"')).map(|s| s.chars().take(200).collect::<String>());
            match (k.trim(), text) {
                ("required", _) => c.required = v != "false",
                ("target", Some(t)) if !t.trim().is_empty() => c.target = Some(t.trim().to_string()),
                ("branch", Some(t)) if !t.trim().is_empty() => c.branch = t.trim().to_string(),
                _ => {}
            }
        }
        c
    }
}

/// `s` as a branch name part: lower case, letters and digits, other runs as one `-`.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(60).collect()
}

/// The work branch the template names for `user` in repository `repo` (`owner/name`; GitLab:
/// `group/sub/name`).
pub fn branch_name(template: &str, user: &str, repo: &str) -> Result<String, CmdError> {
    let name = repo.rsplit('/').next().unwrap_or(repo);
    let branch = template.replace("{user}", &slug(user)).replace("{repo}", &slug(name));
    check_branch(&branch).map_err(|e| CmdError::Failed(format!("{WORKFLOW_FILE}: the branch name {branch:?} won't do ({e})")))?;
    Ok(branch)
}

/// Apply the repository's workflow to `doc`: on its target branch, the document moves to the
/// person's work branch (made ready on the forge). `None`: the repository has no workflow, or
/// the document was opened on another branch on purpose. Also gives the work branch's open
/// review, if any.
pub fn apply(c: &Client, doc: &mut DocRef) -> Result<Option<(Workflow, Option<Review>)>, CmdError> {
    let ferr = |e: ForgeError| CmdError::Failed(e.to_string());
    let text = match c.read_path(doc, WORKFLOW_FILE, None) {
        Ok((b, _)) if b.len() <= MAX_CONFIG => String::from_utf8_lossy(&b).into_owned(),
        Ok(_) => return Err(CmdError::Failed(format!("{WORKFLOW_FILE} is too large"))),
        Err(ForgeError::NotFound) => return Ok(None),
        Err(e) => return Err(ferr(e)),
    };
    let cfg = Config::parse(&text);
    if !cfg.required {
        return Ok(None);
    }
    let target = cfg.target.clone().unwrap_or_else(|| doc.branch.clone());
    let user = c.current_user(&doc.forge).map_err(ferr)?;
    let branch = branch_name(&cfg.branch, &user, &doc.repo)?;
    if branch == target {
        return Err(CmdError::Failed(format!("{WORKFLOW_FILE}: the work branch can't be {target}")));
    }
    if doc.branch != target && doc.branch != branch {
        return Ok(None);
    }
    let on_target = DocRef { branch: target.clone(), ..doc.clone() };
    let mut fresh = false;
    let mut last: Option<BranchReview> = None;
    if !c.branch_exists(&on_target, &branch).map_err(ferr)? {
        c.create_branch(&on_target, &branch).map_err(ferr)?;
        fresh = true;
    } else {
        last = c.branch_review(&on_target, &branch).map_err(ferr)?;
        // Merged, and nothing saved on the branch since: it starts again from the target.
        if let Some(r) = last.as_ref().filter(|r| r.merged && !r.head.is_empty())
            && c.head_of(&on_target, &branch).map_err(ferr)? == r.head
        {
            c.reset_branch(&on_target, &branch).map_err(ferr)?;
            fresh = true;
            last = None;
        }
    }
    doc.branch.clone_from(&branch);
    let open = last.filter(|r| r.open).map(|r| r.review);
    Ok(Some((Workflow { target, branch, fresh }, open)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_and_the_branch_name() {
        let c = Config::parse("# locked\nrequired = true\ntarget = \"main\"  # the published one\nbranch = \"{user}-{repo}\"\nother = 3\n");
        assert_eq!(c, Config { required: true, target: Some("main".into()), branch: "{user}-{repo}".into() });
        assert!(!Config::parse("required = false").required);
        assert_eq!(Config::parse("junk\n= =\n"), Config::default());
        assert_eq!(branch_name("{user}-{repo}", "Ada.Lovelace", "acme/User Guide").unwrap(), "ada-lovelace-user-guide");
        assert_eq!(branch_name("work/{user}", "ada", "group/sub/docs").unwrap(), "work/ada");
        assert!(branch_name("{user}..{repo}", "ada", "x/docs").is_err());
    }
}
