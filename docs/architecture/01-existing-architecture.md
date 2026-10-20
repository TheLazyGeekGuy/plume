# 01 — Existing WordCraft architecture

> Status: initial audit, 14 October 2026. Branch `claude/gallant-allen-2rb6nx`.
> Legend: **[code]** verified in the code · **[doc]** stated by the repository documentation · **[tested]** verified by a test run · **[hypothesis]** to be confirmed.

## 1. Fork state

| Item | Finding |
|---|---|
| Working repository | `TheLazyGeekGuy/plume` (public fork of `storytold/wordcraft`, named `TheLazyGeekGuy/wordcraft` until 2026-10-20) |
| Remotes | `origin` → `https://github.com/TheLazyGeekGuy/plume`; `upstream` → `https://github.com/storytold/wordcraft` (merged regularly) |
| Branches | `main`, `claude/gallant-allen-2rb6nx` (working branch, identical to `main` at the start) |
| Uncommitted changes | none when the audit started (clean `git status`) |
| Last upstream commit merged | `eb54d88` "Mail merge: .xls recipients…" |
| Version | `0.7.0`, Rust 2024 edition, MSRV 1.90 (`Cargo.toml`) |
| Code licence | MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`) |
| `plan/` directory | absent (gitignored upstream) — the audit starts from `ROADMAP.md`, `docs/gaps.md`, `docs/roadmap.md` |

Recommendation: add the original repository as the `upstream` remote (`git remote add upstream https://github.com/storytold/wordcraft`) to pick up upstream changes. Not done in the audit: the owner works in the cloud, so the remote is added in the cloud checkout (07 phase 1).

## 2. Layering **[code]**

The layering is enforced by `cargo xtask layers` (`xtask/src/layers.rs:35-58`).

| Layer | Crate | Role | Relevance to the project |
|---|---|---|---|
| L0 | `geom` | units, measurements, regions (paper size) | kept as is |
| L1 | `doc` | document model + edit primitives | **core, kept** |
| L1 | `fonts` | font database, shaping, Word substitution | kept |
| L2 | `layout` | line breaking, pagination, tables, hit testing | kept (pagination of the preview and the PDF) |
| L2 | `docx` | OOXML read/write | becomes an **importer** (writing stays for "Export to Word") |
| L2 | `docbin` | Word 97-2003 (`.doc`) reading | importer |
| L2 | `formats` | TXT, **Markdown**, HTML, RTF, ODT, LaTeX | **main place of change** |
| L3 | `render` | vello_cpu rasterisation | kept |
| L3 | `pdf` | PDF export (krilla) | kept |
| L3 | `proof` | spelling / hyphenation | kept |
| L4 | `engine` | `Session`, undo, **command registry**, I/O dispatch, Word catalog | targeted changes (I/O, version commands) |
| L5 | `mcp` | MCP server | kept |
| L6 | `ui-egui` | Word-style UI (ribbon, backstage) | additions (Review/Publishing tab, History panel) |
| apps | `wordcraft`, `wordcraft-cli`, `wordcraft-web` | desktop, CLI, WASM binaries | renamed later |

Other crates: `metafile` (WMF/EMF), `zotero` (bibliography), `control-key` (control channel key).

Structural principle: **every user-facing behaviour is a command** (`CommandSpec` in `crates/engine/src/cmd/*.rs`), callable from the ribbon, shortcuts, the CLI, the control channel and MCP. New features (save a version, sync, compare with a version) must follow this model.

## 3. In-memory document model **[code]**

`crates/doc/src/lib.rs:623-650` — `Document`:

- `body: Blocks` with `Blocks = Vec<Arc<Block>>` and `enum Block { Para(Paragraph), Table(Table) }` (lib.rs:78-98);
- `parts: BTreeMap<u32, Part>`: headers, footers, footnotes, endnotes, comments, text boxes;
- `styles`, `numbering`, `settings`, `core` (metadata), `sources` (bibliography);
- `comments: BTreeMap<u32, Comment>` (author, initials, date, parent for replies, `resolved`);
- `revisions: Vec<Revision>` (track changes: insertion / deletion / format, author, date);
- `media: BTreeMap<String, Arc<Vec<u8>>>`: **image bytes**, referenced by key;
- `passthrough`: OOXML parts kept verbatim for DOCX round-tripping.

`Paragraph` (para.rs:751) = `text: String` + `runs` (length + `CharProps`) + `objects: Vec<InlineObject>` anchored on U+FFFC. Links are a run attribute (`CharProps.link`), images an `InlineObject::Image { media, w, h, alt, float, crop }`, comments `CommentStart/End` pairs, bookmarks `BookmarkStart/End`, notes `NoteRef`.

Consequence: the model is **much richer than Markdown**; it does not need to change. The Markdown constraint sits at the persistence boundary (see 03).

## 4. "Flow" model of the text formats **[code]**

`crates/formats/src/model.rs` defines a reduced intermediate model (`Flow` → `FBlock::Para | FBlock::Table`, `Inline::Text/Image/Anchor/Figure/Equation`, `Fmt`) with `to_doc` / `from_doc`. **All** text formats (Markdown, HTML, RTF, ODT, LaTeX) go through it. This is the "controlled transformation layer" required by option B: it already exists.

## 5. Input/output **[code]**

- `crates/engine/src/io.rs`: `OPEN_EXTS` (docx, docm, dotx, dotm, doc, dot, txt, **md, markdown**, html, rtf, odt, tex, xml, json), `SAVE_EXTS` (… **md** …, pdf, png). Dispatch by extension, then by signature (`open_by_content`).
- `crates/engine/src/io_ext.rs`: routes DOCX/DOC/XML to `docx`/`docbin`, PDF/PNG, and the rest to `wordcraft_formats::import_on/export_with`.
- Atomic save (`io/safe_save.rs`: temporary file, fsync, rename).
- **Default save format: `.docx`** (`ui-egui/src/lib.rs:1547-1552`, `default_save_name`). `.md` is treated as a copy (the document stays "modified", `cmd/file.rs:286-289`).
- Autosave: UI side only, every 2.5 s, and only for a lossless format (`keeps_everything`); never in the browser. **No on-disk crash recovery.**

## 6. Undo / versions / review **[code]**

- Undo by full snapshot (`Session.history`, `MAX_UNDO = 500`, `session.rs:283-292, 569`): cheap thanks to `Arc`.
- **Versions**: `file.versions` (`cmd/tools.rs:396-412`) — named snapshots **in memory only** (`Session.versions`, max 50), lost on close. `file.recover` lists these versions.
- **Review** (`cmd/review.rs`): comments (new, reply, edit, delete, resolve), track changes (Ctrl+Shift+E), accept/reject (one, all), revisions pane.
- **Compare / Combine** (`cmd/tools.rs:386-387`, `fn compare` :1018): compares two documents and produces a document with revisions — **reusable as is for "compare two Git versions"**.
- **No Git integration** and **no HTTP network dependency** (`reqwest`, `ureq`, `gix`, `git2` absent from `Cargo.lock`). Only `std::net::TcpStream` is used (Zotero, MCP).

## 7. User interface **[code]**

- Ribbon: `crates/engine/src/ribbon.rs:18-22` — File, Home, Insert, Draw, Design, Layout, References, Mailings, Review, View, Developer, Zotero, Help tabs + contextual tabs (Table, Picture, Shape, Equation, Chart, SmartArt).
- Review tab: `ui-egui/src/ribbon.rs:1716-1790` (Proofing, Speech, Accessibility, Language, Comments, Tracking, Changes, Compare, Protect, Ink).
- Backstage (File menu): `ui-egui/src/backstage.rs` (Home, New, Open, Info, Save, Save As, Print, Export, Close, Options).
- Dialogs: `ui-egui/src/file_dialogs.rs` (shared filters), `apps/wordcraft/src/file_dialogs.rs` (native rfd + built-in fallback browser).

## 8. PDF **[code]**

`crates/pdf/src/lib.rs`: `export(doc, &PdfOptions)` → layout then krilla. Fonts subset and embedded with Unicode mapping (selectable text), clickable links (URL and `#bookmark`), **PDF bookmarks from headings** (`fn outline`, lib.rs:1441), tagged PDF (H1–H6, P, Figure with alt text). Works offline and in WASM (built by `cargo xtask wasm`). Covers all requirements of §7.3 of the specification.

## 9. Web / WASM **[code]**

`apps/wordcraft-web` (Trunk + eframe/wgpu, WebGL2 fallback). Open via file picker or drag and drop, save via `showSaveFilePicker` or Blob download. On wasm: no file system, no built-in browser, no autosave, no render threads, no system font scan. `cargo xtask wasm` checks every L0–L6 crate for `wasm32-unknown-unknown`.

## 10. Quality and tooling **[code]**

- `cargo xtask ci`: fmt, clippy `-D warnings`, tests, assets, layers, wasm (`xtask/src/main.rs:156-205`).
- ~128 tests in `formats`, ~556 in `engine` (`#[test]` count).
- "Never crash" rule: no `unwrap/expect/panic` outside tests, `unsafe` forbidden, hostile input bounded.
- Asset rule: no Microsoft/Adobe artwork; every asset file has its row in `ATTRIBUTION.md`.

## 11. Identity and trademarks **[code]**

- Application id `ai.storyteller.wordcraft` (`apps/wordcraft/src/main.rs:293`, `packaging/macos/Info.plist.in`, `packaging/linux/*`, flatpak).
- Windows: `packaging/windows/wordcraft.wxs` — manufacturer "Learning Machines LLC" and an `UpgradeCode` **specific to WordCraft** (must be regenerated for the fork).
- Configuration paths: `crates/control-key/src/lib.rs:7-18` (`~/.config/wordcraft`, `%APPDATA%\WordCraft`, `~/Library/Application Support/WordCraft`).
- ArtCraft trademarks (`docs/brand/`, `NOTICE`, `LICENSE-brand.txt`, README:360-385): **a modified fork must remove or replace** the logos and must not present itself as ArtCraft; it may say "based on WordCraft by the ArtCraft team". The "About" link points to getartcraft.com and the ArtCraft Discord (`ui-egui/src/dialogs.rs:2578-2586`).
- Application icons (`assets/app-icon/`): "placeholder", MIT/Apache — reusable but to be replaced for our own identity.
