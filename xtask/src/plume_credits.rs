//! `cargo xtask plume-credits [--upstream <ref>] [--repo <owner/name>]`: Plume's own credits.
//!
//! Writes `contributors/plume.json`, in the schema of `contributors.json` (craftrules
//! standards/contributors.md), from the commits Plume has and WordCraft (`--upstream`, default
//! `upstream/main`) doesn't. Each commit is credited to its author's GitHub login: a
//! `users.noreply.github.com` address gives it, and a commit an AI wrote in the repository
//! owner's session (author `noreply@anthropic.com`) is the owner's. Pull requests are counted from
//! the "Merge pull request #N from <login>/…" commits; models from `Co-Authored-By` trailers.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

#[derive(Default)]
struct Person {
    prs: u64,
    commits: u64,
    added: u64,
    deleted: u64,
    binary_added: u64,
    binary_deleted: u64,
    first: String,
    last: String,
}

#[derive(Default)]
struct ModelStats {
    commits: u64,
    added: u64,
    deleted: u64,
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git").current_dir(root).args(args).output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The GitHub login an author's address names, if any (`123+login@users.noreply.github.com`).
fn login_of(email: &str) -> Option<String> {
    let local = email.strip_suffix("@users.noreply.github.com")?;
    Some(local.split_once('+').map_or(local, |(_, l)| l).to_string())
}

/// `Claude Opus 5.5 <noreply@anthropic.com>` → (company, model, version).
fn model_of(trailer: &str) -> Option<(String, String, String)> {
    let (name, email) = trailer.split_once('<')?;
    let company = if email.contains("anthropic.com") { "Anthropic" } else { return None };
    let name = name.trim();
    let (model, version) = match name.rsplit_once(' ') {
        Some((m, v)) if v.chars().next().is_some_and(|c| c.is_ascii_digit()) => (m, v),
        _ => (name, ""),
    };
    Some((company.into(), model.into(), version.into()))
}

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    let opt = |name: &str| args.iter().position(|a| *a == name).and_then(|i| args.get(i + 1)).copied();
    let upstream = opt("--upstream").unwrap_or("upstream/main");
    let repo = opt("--repo").unwrap_or("TheLazyGeekGuy/plume");
    let owner = repo.split('/').next().unwrap_or(repo).to_string();
    let range = format!("{upstream}..HEAD");
    let mut people: BTreeMap<String, Person> = BTreeMap::new();
    let mut models: BTreeMap<(String, String, String), ModelStats> = BTreeMap::new();
    let mut total = 0u64;

    // One record per commit: header line, then its numstat lines.
    let log = git(
        root,
        &["log", "--no-merges", "--numstat", "--format=%x1e%ae%x1f%aI%x1f%(trailers:key=Co-Authored-By,valueonly,separator=%x1d)", &range],
    )?;
    for rec in log.split('\u{1e}').filter(|r| !r.trim().is_empty()) {
        let mut lines = rec.lines();
        let head = lines.next().unwrap_or("");
        let mut f = head.split('\u{1f}');
        let (email, date, trailers) = (f.next().unwrap_or(""), f.next().unwrap_or(""), f.next().unwrap_or(""));
        let (mut added, mut deleted, mut bin) = (0u64, 0u64, 0u64);
        for l in lines {
            let mut c = l.split('\t');
            match (c.next(), c.next()) {
                (Some("-"), Some("-")) => bin += 1,
                (Some(a), Some(d)) => {
                    added += a.parse::<u64>().unwrap_or(0);
                    deleted += d.parse::<u64>().unwrap_or(0);
                }
                _ => {}
            }
        }
        total += 1;
        let login = login_of(email).unwrap_or_else(|| if email.ends_with("anthropic.com") { owner.clone() } else { email.to_string() });
        let p = people.entry(login).or_default();
        p.commits += 1;
        p.added += added;
        p.deleted += deleted;
        p.binary_added += bin;
        if p.first.is_empty() || date < p.first.as_str() {
            p.first = date.to_string();
        }
        if date > p.last.as_str() {
            p.last = date.to_string();
        }
        for t in trailers.split('\u{1d}').filter(|t| !t.trim().is_empty()) {
            if let Some(key) = model_of(t) {
                let m = models.entry(key).or_default();
                m.commits += 1;
                m.added += added;
                m.deleted += deleted;
            }
        }
    }
    for subject in git(root, &["log", "--merges", "--format=%s", &range])?.lines() {
        if let Some(rest) = subject.strip_prefix("Merge pull request #")
            && let Some((_, from)) = rest.split_once(" from ")
        {
            let login = from.split('/').next().unwrap_or(from).to_string();
            people.entry(login).or_default().prs += 1;
        }
    }

    let head = git(root, &["rev-parse", "HEAD"])?.trim().to_string();
    let head_date = git(root, &["log", "-1", "--format=%aI", "HEAD"])?.trim().to_string();
    let mut list: Vec<(&String, &Person)> = people.iter().collect();
    list.sort_by(|a, b| a.1.first.cmp(&b.1.first).then(a.0.cmp(b.0)));
    let contributors: Vec<Value> = list
        .iter()
        .map(|(login, p)| {
            json!({"login": login, "people_entry": false, "github_name": null, "prs": p.prs, "commits": p.commits,
                "lines_added": p.added, "lines_deleted": p.deleted, "binary_added": p.binary_added, "binary_deleted": p.binary_deleted,
                "first_commit": p.first, "last_commit": p.last})
        })
        .collect();
    let mut mlist: Vec<(&(String, String, String), &ModelStats)> = models.iter().collect();
    mlist.sort_by_key(|m| std::cmp::Reverse(m.1.commits));
    let models: Vec<Value> = mlist
        .iter()
        .map(|((company, model, version), m)| {
            let id = format!("{}/{}/{version}", company.to_lowercase(), model.to_lowercase().replace(' ', "-"));
            json!({"id": id, "company": company, "model": model, "version": version, "commits": m.commits, "lines_added": m.added, "lines_deleted": m.deleted})
        })
        .collect();
    let out = json!({"schema": 1, "repo": repo, "branch": "main", "upstream": upstream, "head": head, "head_date": head_date,
        "commits": total, "unattributed_commits": 0, "contributors": contributors, "models": models});
    let path = root.join("contributors/plume.json");
    let text = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}: {total} commits, {} contributors, {} models", path.display(), people.len(), mlist.len());
    Ok(())
}
