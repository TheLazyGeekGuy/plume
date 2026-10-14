# 05 — Git and forge integration (GitHub, GitLab, Forgejo)

> Status: design (nothing implemented). Legend: **[code]** verified · **[doc]** public forge documentation · **[hypothesis]** to prototype.

## 1. Requirements

- Open **a single Markdown file at a time** in a Git repository (local or remote).
- **GitHub, GitLab and Forgejo** authentication; a document can be opened by anyone with access to the repository.
- Each "version" = **one commit with a message**; the user never sees the word "commit".
- See colleagues' revisions: **who changed what, when, with what message**.
- **Keep the existing version mechanism** (`file.versions`, in memory: `crates/engine/src/cmd/tools.rs:396-412`) and **plug it into Git** when the document lives in a repository.
- Save ≠ create a version ≠ sync (§9.2 of the specification).

## 2. User-facing vocabulary

| Git | UI |
|---|---|
| commit | **Save a version** (with "Describe your changes") |
| push | **Sync** (automatic after "Save a version" when signed in) |
| pull / fetch | **Get changes** |
| file log | **Document history** |
| diff | **Compare / Show changes** |
| branch | **Working version** |
| merge request / pull request | **Request a review** |
| review | **Review** (comments, approval) |
| merge | **Integrate / Publish** |
| conflict | **Changes to resolve** |

## 3. Architecture

```text
ui-egui (L6) ── History panel, Sign-in dialog, Publishing tab
     │ vcs.* commands (CommandSpec, like everything else)
engine (L4) ── VersionStore (trait): list / read_at / save_version / sync / restore
     │                      ├── MemoryStore  (current behaviour, documents outside a repo)
     │                      ├── LocalGitStore (repository on disk)        [desktop]
     │                      └── ForgeStore    (REST API, no clone)        [desktop + Web]
crate `forge` (new, L2, no network of its own) ── GitHub / GitLab / Forgejo: requests + JSON decoding
     │ HTTP transport injected by the application
apps ── native: Rust HTTP client (e.g. `ureq` + rustls); Web: browser `fetch` (web-sys)
```

Repository rules respected: the engine stays free of network and egui (`cargo xtask layers`), network access is a **service provided by the application** (like the file dialog today, `apps/wordcraft-web/src/web.rs`), and programmatic commands (CLI, MCP) go through the same `CommandSpec`s.

### 3.1 Why the forges' REST APIs rather than a Git clone?

The need is **one file at a time**. All three forges expose everything needed for a single file without cloning the repository **[doc]**:

| Need | GitHub (REST v3) | GitLab (API v4) | Forgejo (Gitea-compatible API) |
|---|---|---|---|
| Read the file (at a revision) | `GET /repos/{o}/{r}/contents/{path}?ref=` | `GET /projects/:id/repository/files/:path?ref=` | `GET /repos/{o}/{r}/contents/{path}?ref=` |
| Write = 1 commit | `PUT …/contents/{path}` (message, expected `sha`) | `PUT …/repository/files/:path` (commit_message, `last_commit_id`) | `PUT …/contents/{path}` (message, `sha`) |
| Several files in 1 commit (`.md` + images) | Git Data API (blobs, tree, commit, ref) | `POST …/repository/commits` (multiple actions) | `POST …/contents` (multiple files) |
| File history | `GET …/commits?path=` | `GET …/repository/commits?path=` | `GET …/commits?path=` |
| Commit comments | `GET/POST …/commits/{sha}/comments` | `GET/POST …/commits/:sha/comments` | **[hypothesis]** to check |
| Review request | Pull requests + reviews | Merge requests + notes/discussions | Pull requests + reviews |

Advantages: works **identically on desktop and in the browser**; no Git installed on the user's machine; concurrency control is built in (the expected `sha`/`last_commit_id` makes the write fail if someone changed the file in the meantime → triggers resolution, §7).

A **local repository** (desktop) is still needed for users who work offline or against a "bare" Git server: `LocalGitStore`.

### 3.2 Local Git: library choice

| Option | Licence | WASM | Commit / history | Push / pull | Assessment |
|---|---|---|---|---|---|
| `gix` (gitoxide, pure Rust) | MIT/Apache | partial (no network) | yes | fetch yes; push **[hypothesis]** to check on the chosen version | **recommended** for history, reading a revision and committing |
| `git2` (libgit2) | GPL-2.0 with linking exception | no | yes | yes | C dependency, complicates Windows/macOS universal builds |
| `git` command as a subprocess | — | no | yes | yes, with system credentials (SSH, credential manager) | **fallback** for push/pull when `git` is installed |

Recommendation: `gix` for all local operations; sync through the forge API if the remote is a known forge, otherwise through the `git` command if present, otherwise sync unavailable (clear message).

## 4. Authentication

| Forge | Preferred method | Fallback | Notes |
|---|---|---|---|
| GitHub | OAuth **device flow** (code entered on github.com) | fine-grained personal token | the device flow does not answer browser CORS calls **[doc]** → the Web version asks for a token or goes through a small relay |
| GitLab (gitlab.com or self-hosted) | OAuth (PKCE), device flow depending on version **[hypothesis]** | personal token | the self-hosted instance must register the OAuth application |
| Forgejo / Codeberg | OAuth2 + PKCE | access token | API CORS must be enabled on the instance (`[cors]`) for the Web version |

Secret storage:

- desktop: **system keychain** (macOS Keychain, Windows Credential Manager, Secret Service on Linux) via a keychain crate; never in a configuration file or in the repository;
- Web: token in session memory only (explicit "remember me" option, with a warning);
- SSH: left to the `git` command (the user's SSH agent).

The control channel and MCP **cannot read** tokens (the `vcs.*` commands receive an opaque token managed by the application).

## 5. User journeys

**Open from a forge**: File → Open → *From an online repository* → choose the forge → sign in → choose repository, branch, `.md` file → the document opens (images from `assets/` loaded through the API, cached locally).

**Save** (Ctrl+S): writes the working copy (local file, or local cache + recovery journal for a remote document). **No commit.**

**Save a version** (Review → Versions, or File → Info): "Describe your changes" box → 1 commit containing the `.md` and the new images → automatic sync when signed in. The default message is suggested from the diff report ("3 paragraphs changed in *Installation*").

**Document history** (side panel, reuses the `file.versions` list):

```text
History — user-guide.md
● Alice  · today 10:12      · "Update screenshots"           💬 2
● Bob    · yesterday 17:40  · "Review of chapter 3"
● Alice  · 12 Oct           · "Initial version imported from guide.docx"
[Show changes] [Compare with current version] [Restore]
```

**Show changes**: version N-1 and version N are read back into `Document`s and passed to **`review.compare`** (already implemented): the result shows as a document with tracked changes, coloured by author, with working **Accept / Reject**. This is the strongest reuse of WordCraft in this project.

**Restore**: opens the old version as the current document; saving a version creates a new commit (history is never rewritten).

## 6. Review and approval workflow

Mapping of the §10.3 workflow to forge objects (optional, enabled per document or per repository):

| Step | Forge object |
|---|---|
| Draft | commits on a working branch `review/<author>-<date>` |
| In review | open merge request, reviewers assigned |
| Changes requested | "changes requested" review |
| New revision | new commits on the branch |
| Approved | "approved" review |
| Published | merge into the main branch |

**Review comments** left on the forge (on a line of the `.md`) are imported as WordCraft comments anchored to the matching paragraph (line → paragraph via block positions at export); replies made in WordCraft are sent back to the forge. Comments **inside the file** (03 §6) work with any Git, even without a forge.

## 7. Conflicts ("changes to resolve")

When sync fails because the file changed remotely:

1. read the remote version and the common base version;
2. `review.compare(base, remote)` → the other author's revisions, applied to the local copy;
3. the user accepts/rejects as in Word; then a new version.

Never any `<<<<<<<` markers in the text. **[hypothesis]**: paragraph-level merging (`compare` + apply) is enough for common cases; to validate on real scenarios (09 §5).

## 8. Autosave and recovery

Independent of Git: a local journal in the application's data folder (not in the repository), written atomically every few seconds while the document is modified; offered on reopen after an abnormal exit. Today autosave only exists for lossless formats and only on the file itself (`ui-egui/src/lib.rs:1775`); it must be allowed for `.md` once Markdown becomes the master format.

## 9. Security

- Content coming from a forge is **untrusted**: same limits as files (sizes, number of images, depth); image paths confined to the repository.
- No network call without an explicit sign-in action; editing, conversion, PDF and local history work offline.
- Minimal token scopes (read/write repository contents, pull requests).
