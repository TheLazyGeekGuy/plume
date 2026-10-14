# 04 — Conversion strategy

> Status: recommendation. Legend: **[code]** verified · **[doc]** public documentation of the tools · **[hypothesis]** to be checked on a corpus.

## 1. Recommendation

**The WordCraft engine is the only default conversion chain**, on every platform including the Web:

```text
.docx/.docm/.dotx ─ crates/docx ─┐
.doc (97-2003)   ─ crates/docbin ┤
.rtf / .odt / .html / .txt ──────┼─► Document ─► export_native ─► .md + assets/ + report
.md (native)     ─ import_with ──┘
```

No heavy external dependency is added. An external converter is only considered as an **optional, desktop-only fallback**, for formats WordCraft does not read (Word 6/95, Word for DOS, WordPerfect).

## 2. Comparison

| Criterion | Native WordCraft | Pandoc | Headless LibreOffice | Rust libraries (`docx-rs`, `docx-rust`) |
|---|---|---|---|---|
| Formats read | DOCX/DOCM/DOTX/DOTM, DOC 97-2003, Flat OPC, WordML 2003, RTF, ODT, HTML, TXT, MD, LaTeX **[code]** | DOCX, ODT, RTF, HTML, EPUB…; **no `.doc`** **[doc]** | almost everything, including Word 6/95 and old `.doc` **[doc]** | DOCX only |
| Expected quality | high: engine built for Word parity (styles, lists, tables, fields, notes, comments, revisions, images, WMF/EMF) **[code]** | good on structure, weak on layout **[doc]** | high **[doc]** | partial (reading mostly geared towards generation) **[hypothesis]** |
| Known losses | see 03 §4 (on the Markdown side, not the import side) | same + numbering, objects | goes through an intermediate format (DOCX/HTML) then through our chain | many |
| Dependencies | none new | external Haskell binary (~100 MB) | full office suite (~500 MB) | Rust crate |
| Licence | MIT/Apache **[code]** | **GPL-2.0+**: usable only as an **external program** invoked, never linked or copied (the repository's *clean room* rule) | MPL-2.0: external program | MIT |
| WebAssembly | **yes** (`cargo xtask wasm`) **[code]** | no (experimental WASM build exists, very heavy) | no | yes |
| Integration effort | **already integrated** | process call + error handling | process call | rewrite of the import layer |

Conclusion: only LibreOffice brings real value (formats older than Word 97), and only as a fallback. Pandoc brings nothing WordCraft does not already do, and adds a GPL constraint. The Rust libraries fall short of the existing engine.

## 3. Per input format

| Format | Component | State | Actions |
|---|---|---|---|
| `.md`, `.markdown` | `formats::markdown::import_with` | native, relative images resolved **[code]** | YAML front matter (currently read as a rule + text), `[^n]` notes, task lists, `> [!NOTE]` alerts |
| `.docx` & co | `crates/docx` | complete, including encrypted documents **[code]** | none |
| `.doc` 97-2003 | `crates/docbin` | complete read-only support (text, styles, lists, tables, notes, images, drawings) **[code]** | none |
| `.doc` Word 6.0/95 | — | **rejected** with a clear message (`docbin/src/fib.rs:117`) **[code]** | optional LibreOffice fallback, or a new importer (limited public specification) |
| `.rtf` | `formats::rtf` | read/write, revisions **[code]** | none |
| `.odt` | `formats::odt` | read/write **[code]** | none |
| `.html` | `formats::html` | read with local images **[code]** | none |
| `.txt` | `formats::txt` | encoding detection (UTF-8/16, Windows-1252, GBK…) **[code]** | none |
| `.pdf` | — | not editable | optional viewing, out of scope |

## 4. Asset conversion

| Case | Handling |
|---|---|
| PNG, JPEG, GIF, WebP, BMP | kept as is, named `img-<hash>.<ext>`, reloaded on open (`model::sniff_image`) **[prototype]** |
| TIFF, SVG, EMF, WMF | **current limitation**: written to `assets/` but **not reloaded** on reopen (`sniff_image` does not recognise them) → the image reverts to its alt text. **High priority**: convert to PNG on save |
| WMF / EMF (common in `.doc` files) | **to do**: vector rendering → PNG via `crates/metafile` + `crates/render` (or SVG) |
| Charts, SmartArt, shapes, WordArt | **to do**: PNG rendering of the object (the engine already draws them on screen); alt text kept |
| Cropped images | **to do**: apply the crop before writing (otherwise the full image reappears) |
| OLE objects (embedded Excel…) | preview image only, reported |

## 5. Import report

The `Report` produced by `export_native` feeds the requested report (§5.4 of the specification). When a `.docx`/`.doc` is opened, the application computes this report **without writing any file** and shows it in a non-blocking banner:

```text
Import complete — procedure.docx
Paragraphs: 156 · Headings: 12 · Tables: 8 · Images: 24 · Links: 17
Adapted items: 3   [Show details]
```

The details list the `Report::lines()` lines ("2 × merged cells written as empty cells"…). The structure counters (paragraphs, headings, links) have to be added to `Report` (**to do**, trivial from the `Flow`).

## 6. Pasting from Word

Paste already goes through `crates/engine/src/cmd/paste.rs` (clipboard HTML and RTF, images). Nothing to change: pasted content enters the rich `Document`, and saving applies the profile. "Strict" mode (03 §5) could offer "Paste keeping structure only" by default.

## 7. The owner's existing conversion scripts

The owner has DOC → Markdown conversion scripts (repository `TheLazyGeekGuy/convertDoC`, private). The audit session was not given access to that repository. **The comparison was dropped by the owner's decision** (done without): the WordCraft importer is the only conversion chain, and the scripts will not be compared or reused.
