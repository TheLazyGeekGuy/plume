# 07 — Migration and development plan

> Status: proposal. Estimates in development days for one person who knows Rust, **[hypothesis]** to revise after phase 2.

## 1. Principles

1. **Stay close to upstream**: crates keep their internal `wordcraft-*` names. Only visible items (name, icons, identifiers, paths) change. This lets us pick up improvements from `storytold/wordcraft` (`upstream` remote) with plain merges.
2. **Add rather than modify**: new code in dedicated modules or crates (native profile in `formats/markdown.rs`, `forge` crate, `VersionStore` in `engine`).
3. **Every step builds and passes `cargo xtask ci`**; every new behaviour is a command with tests.
4. **Every structural decision has its ADR** (`docs/architecture/adr/`).

## 2. Phases

### Phase 0 — Audit ✅ (this folder)

### Phase 1 — New project (identity) — 3 to 5 d — in progress

Branding is being done now. The owner works in the cloud: every step below happens in the cloud checkout, with no "on your machine" steps.

| Task | Files |
|---|---|
| Name: **Plume** (decided by the owner, see §4) | — |
| Add the `upstream` remote (`git remote add upstream https://github.com/storytold/wordcraft`) | the cloud checkout's Git configuration |
| Remove ArtCraft trademarks (README logo, `docs/brand/` outside the upstream repository, Discord/site link in "About") | `README.md`, `docs/brand/`, `crates/ui-egui/src/dialogs.rs:2578` |
| New application id `io.github.thelazygeekguy.plume` (replaces `ai.storyteller.wordcraft`) | `apps/wordcraft/src/main.rs:293`, `default_apps.rs:35`, `packaging/macos/Info.plist.in`, `packaging/linux/*`, flatpak |
| **New Windows `UpgradeCode`** and manufacturer | `packaging/windows/wordcraft.wxs:13-23` |
| Own configuration paths (avoid sharing configuration with an installed WordCraft) | `crates/control-key/src/lib.rs:7-18`, `crates/proof/src/hunspell.rs:60`, IndexedDB database `apps/wordcraft-web/src/web/font_store.rs:10` |
| Titles, PDF/DOCX producer metadata, MCP name | `ui-egui/src/lib.rs:1867`, `chrome.rs:350`, `pdf/src/lib.rs:366`, `docx/src/write/mod.rs:1237`, `mcp/src/server.rs:136` |
| New **original** icon drawn in code or CC0, with an `ATTRIBUTION.md` row | `assets/app-icon/`, `packaging/icons.sh` |
| `NOTICE`: "based on WordCraft by the ArtCraft team", MIT/Apache licences kept | `NOTICE`, `README.md` |

### Phase 2 — Native Markdown — 10 to 15 d (mostly done, 2026-10-14)

| Task | State |
|---|---|
| Zero-HTML `export_native`, `assets/` with stable names, `Report` | ✅ done |
| `import_with` (relative images); opening `.md` from disk | ✅ done |
| `io::save_markdown`; a `.md` path saves natively | ✅ done |
| End-to-end test DOCX → MD → MD → PDF | ✅ done |
| `.md` = master format: default name on save, "modified" flag, autosave; Word files imported into `<name>.md` | ✅ done (`Session::native_markdown`) |
| Report shown after import and on save | ✅ done (status bar; a richer banner is still to do) |
| YAML front matter (import + export) | ✅ done |
| Footnotes `[^n]` | ✅ done (endnotes written as footnotes) |
| Anchored comments `<!-- cN -->` (03 §6) | ✅ done (block `<!-- plume:comments … -->`) |
| Internal links to heading slugs | ✅ done |
| PNG conversion of non-web images, charts/SmartArt rendered as images | ✅ done; equations as `$…$` too |
| Strict/lenient command mode (03 §5) | ✅ done (`markdown.strict`) |
| `gfm` / `vitepress` visual themes (stylesheets applied on import) | ✅ done |
| Publishing profile `publication/default.toml` (page, margins, fonts) | ✅ done |

### Phase 3 — Word import and PDF — 5 to 8 d

Corpus (09 §2), import report, asset conversion, Word 6/95 (optional fallback), PDF export according to the publishing profile.

### Phase 4 — Storage and Git — 15 to 25 d

`VersionStore` + `MemoryStore` (taking over `file.versions`), `LocalGitStore` (`gix`), History panel, "Save a version", "Show changes" via `review.compare`, restore, recovery journal, document folder browser.

### Phase 5 — Forges and review — 20 to 30 d

`forge` crate (GitHub, GitLab, Forgejo), authentication (device flow / PKCE / tokens), keychain, open from a forge, sync, conflicts via `compare`, review (PR/MR), forge comments ↔ Word comments, approval workflow.

**Status 2026-10-14 (phases 4–5):** done — crate `wordcraft-vcs` (Git through the `git` program; GitHub/GitLab/Forgejo REST connectors), commands `vcs.*` and `forge.*`, Review › Versions ribbon group, Save Version and Version History dialogs, File › Open › Open from a Forge…, tokens in the macOS Keychain, macOS CI (`.github/workflows/plume-macos.yml`). Also done: Request Review (pull/merge request on a review branch), Get Review Comments (as Word comments on the right paragraph), review status, the Documents pane, Send Replies (2026-10-15: a reply to a reviewer's comment goes to its thread — GitHub line-comment threads, GitLab discussions — or to the review's conversation, quoting it; colleagues' answers in a thread come back as replies), forge lists loaded on a worker thread, sign-in with the browser (OAuth device flow, GitHub and GitLab: the OAuth application's client id is typed once in Open from a Forge, or comes from `PLUME_GITHUB_CLIENT_ID` / `PLUME_GITLAB_CLIENT_ID`; not on the web build). Still to do: register Plume's own OAuth applications so no client id has to be typed, the web transport (async `fetch`), conflict merge beyond "reload or compare".

### Phase 6 — Distribution — 5 to 10 d

macOS/Windows/Linux/Web packages under the new identity, user documentation (English first; translations through the existing localization work), compatibility tests.

## 3. Renaming strategy

- **Do not** rename crates or modules (unreadable diffs, upstream merges impossible).
- Centralise the display name in a single constant (for example `wordcraft_engine::APP_NAME`) and reference it wherever "WordCraft" is visible — a change upstream might even accept.
- Rename binaries at the packaging level only (`[[bin]] name`), not the Cargo packages.

## 4. Name: Plume

The owner chose **Plume** as the product name. Criteria it meets: no "Craft" (avoids confusion with the ArtCraft trademark), no reference to Word/Office.

Check done on 2026-10-14:

- No registered trademark found in a quick web search.
- Other software already uses the name:
  - "Plume", a desktop writing app for Windows/Mac (https://www.producthunt.com/p/plume-2);
  - Plume Creator, a GPL novel-writing tool, discontinued and succeeded by Skribisto;
  - Plume, an AGPL federated blogging engine written in Rust (https://github.com/Plume-org/Plume, joinplu.me);
  - the Rust crate `plume` (LGPL).

Recommendation: keep Plume as the display name, keep it in a single constant (§3) so it can change, and have the owner run a formal EUIPO/INPI/USPTO search (classes 9 and 42) before any public release.

Application id chosen: `io.github.thelazygeekguy.plume`.
