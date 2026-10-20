# 02 — Feasibility study

> Status: initial audit, 14 October 2026. Legend: **[code]** verified in the code · **[tested]** verified by a test run · **[hypothesis]** to be confirmed · **[prototype]** demonstrated by this branch's prototype.

## 1. Conclusion

**Option B chosen: adapt WordCraft with a dedicated document layer.**

- The UI, the document engine, pagination, PDF, the DOCX/DOC importers and WASM are kept **unchanged**.
- The controlled transformation layer already exists (`crates/formats/src/model.rs`, `Flow` model); we add a **native Markdown profile** to it (zero HTML, external assets, adaptation report). This is done in this branch (prototype, see §4).
- What changes: the persistence boundary (`.md` becomes the default save format), a few editing commands to constrain or flag, and two new building blocks: **Git storage/versioning** and **forge connectors**.

Why not pure option A: the in-memory model is much richer than Markdown (colours, fonts, floating objects, sections…). Saving it "as is" to Markdown would mean either silent loss (forbidden by the specification) or hidden HTML/XML (forbidden). A profile layer with a report is needed — hence B.

Why not option C: nothing in the audit justifies replacing a component. The existing Markdown parser is already CommonMark + GFM, bounded and panic-free.

## 2. Answers to the 12 Markdown audit questions (§15.2)

| # | Question | Answer | Source |
|---|---|---|---|
| 1 | In-memory representation | Rich `Document` (`Para`/`Table` blocks, runs + inline objects, secondary parts, media by key) | `crates/doc/src/lib.rs:623` **[code]** |
| 2 | Markdown import | `markdown::import`: in-house CommonMark parser (ATX/setext headings, nested lists, block quotes, code, rules, GFM tables, emphasis via the delimiter algorithm, links, autolinks, entities, strikethrough). Images: only `data:` URIs became images. **Added: `import_with` resolves relative paths** | `crates/formats/src/markdown.rs` **[code][prototype]** |
| 3 | Markdown export | `markdown::export` via `model::from_doc`. Wrote `<br>` in cells, `<a id>` for bookmarks and `![](image.png)` **without writing the image files**. **Added: `export_native`** | same **[code][prototype]** |
| 4 | Conversion losses | underline, superscript/subscript, colours, highlighting, fonts/sizes, alignment, page breaks, sections/headers/footers, footnotes (reference mark only), merged cells, nested tables, floating objects (become inline), charts/SmartArt (alt text), equations (linear text), track changes, fields (result only) | `model.rs:803-960` **[code]** |
| 5 | Markdown as the default format? | Yes: change `default_save_name` (`ui-egui/src/lib.rs:1547`) and treat `.md` as the "master" format (path + "modified" flag) in `cmd/file.rs:286-289` | **[code]** — to do in phase 2 |
| 6 | Disable non-representable features? | Yes, through the `enabled` field of `CommandSpec` (greyed out according to a session "Markdown mode") — without removing commands from the ribbon | `crates/engine/src/cmd/mod.rs` **[code]**, see 03 §5 |
| 7 | Rendering styles outside the document? | Yes: the `Document.styles` stylesheet is rebuilt on import (`model::to_doc_in`); a TOML **publishing profile** can set fonts, sizes, margins on open | **[hypothesis]** to prototype (phase 3) |
| 8 | Images | `InlineObject::Image { media, w, h, alt }` + bytes in `Document.media`. The native profile writes them to `assets/img-<hash>.<ext>` | **[prototype]** |
| 9 | Relative links | Links are kept as is (run attribute); relative images are now resolved on open, **confined to the document's folder** (`LocalImages`, `..`/symlink/size protections) | `crates/engine/src/io.rs` **[code][prototype]** |
| 10 | Tables over several cycles | Yes for simple GFM tables: export → import → export is identical. Multi-line cells are flattened **in the first cycle**, then stable | test `native_markdown_is_html_free_and_stable` **[tested]** |
| 11 | Keep pagination? | Yes, fully: Markdown is converted to a `Document` and laid out by `layout` like any document | **[code]** |
| 12 | Avoid a deep engine change? | Yes: no change to `doc`, `layout`, `render`, `pdf`, `docx`, `docbin`. Changes limited to `formats/markdown.rs` and `engine/io.rs` | this branch's diff **[code]** |

## 3. Feasibility table (§19)

| Feature | Existing | Reusable | Changes | Risk |
|---|---|---|---|---|
| Word UI | Full ribbon, backstage, shortcuts, contextual tabs — `crates/engine/src/ribbon.rs`, `crates/ui-egui/src/ribbon.rs`, `backstage.rs` | Yes, fully | Add a **Publishing** tab, a **Versions** group in Review, a **History** panel; grey out non-representable commands in Markdown mode | Low |
| Document engine | `crates/doc` (model), `crates/engine` (Session, undo, 500 levels, commands) | Yes | None in `doc`; in `engine`: notion of "master format" and Markdown mode | Low |
| Markdown | `crates/formats/src/markdown.rs` (CommonMark+GFM import/export), `model.rs` (Flow) | Yes | **Done:** `export_native`, `import_with`, `html_lines`, report. **To do:** GFM footnotes `[^n]`, anchored comments, YAML front matter, task lists on export | Medium (fidelity) |
| DOCX | `crates/docx` (OOXML read/write, encryption) | Yes, as an importer | None | Low |
| Legacy DOC | `crates/docbin` (Word 97-2003, [MS-DOC], read only) | Yes | **Word 6.0/95 rejected** (`docbin/src/fib.rs:117`): to document, possible later addition | Medium (Word 95 and earlier) |
| PDF | `crates/pdf` (krilla: embedded fonts, links, heading bookmarks, tagged PDF) | Yes, fully | None; apply the publishing profile before export | Low |
| Pagination | `crates/layout` | Yes | None | Low |
| Images | `Document.media`, `InlineObject::Image`, `LocalImages` (HTML) | Yes | **Done:** `assets/` extraction, stable names, deduplication, re-import. **To do:** display size (lost in pure GFM), non-web images (EMF/WMF/TIFF → PNG) | Medium |
| Local Git | Nothing (in-memory versions only: `cmd/tools.rs:396`) | The `file.versions` command model | New `vcs` crate (L2/L4): history, commit, diff, restore — see 05 | Medium |
| GitHub | Nothing | — | REST API connector (contents, commits, PRs, comments) + OAuth device flow | Medium |
| GitLab | Nothing | — | Same, API v4 | Medium |
| Forgejo | Nothing | — | Same, API (Gitea-compatible); the owner already has a `forgejo-mcp` fork | Medium |
| WebAssembly | `apps/wordcraft-web`, all crates checked by `cargo xtask wasm` | Yes | No file system or native Git in the browser: use the **forges' REST APIs** (no clone) and IndexedDB/OPFS storage | High (Git + CORS + auth) |

## 4. Prototype built in this branch

Target milestone (§21):

```text
Open DOCX → edit → save Markdown → close → reopen Markdown → edit without loss → export PDF
```

What is demonstrated **by an automated test** (`crates/engine/src/tests.rs::docx_to_native_markdown_round_trip`):

1. a document (WordCraft sample document + one image) is written as DOCX then reopened through the DOCX importer;
2. `io::save_markdown` writes `doc.md` + `assets/img-<hash>.png`;
3. the file contains **no HTML line** (`markdown::html_lines`);
4. reopening reloads the image from `assets/`; saving again produces **exactly the same text**;
5. an edit through the command layer (`text.insert`) is saved, read back, and the rest of the file is unchanged;
6. the PDF export of the reopened document produces a valid PDF.

What remains **not demonstrated**: usability in the GUI (Save → `.md` by default), the import report shown to the user, the owner's real corpus of Word documents.

## 5. Points to watch

1. **The target "visual" rendering** (GitHub, VitePress, VitePress + plugins) changes what can be represented: footnotes, `::: tip` containers, `{#id}` anchors, maths. See 03 §3 — the owner has since confirmed the `gfm` (GitHub) profile as the default.
2. **Comments**: the only exception to zero HTML (HTML comments `<!-- -->`). The prototype keeps them as a block at the end of the file; precise anchoring is designed in 03 §6.
3. **Word 95 and earlier**: not supported by `docbin`. "Any Word file" requires either a new importer or an optional external converter.
4. **The owner's existing conversion scripts** (`convertDoC`): not consulted — access to that repository was not granted to the audit session. The comparison with the WordCraft importer has since been dropped by the owner's decision (done without).
