# 08 — Risk assessment

> Scale: probability / impact Low · Medium · High.

| # | Risk | Prob. | Impact | Mitigation |
|---|---|---|---|---|
| R1 | **Word ↔ Markdown gap**: the user applies colours, fonts, cell merges and loses them on save | High | High | Adaptation report (`Report`, implemented), status bar indicator, strict mode, a visual theme that shows the final rendering while editing, Word/PDF export always available |
| R2 | **Silent loss** of content (footnotes, non-web images, objects) | Medium | High | "Everything lost is counted" invariant test; corpus; priority on `[^n]` notes and PNG conversion (04 §4) |
| R3 | **Drift across** open/save cycles | Low | High | Idempotence tested (`native_markdown_is_html_free_and_stable`, `docx_to_native_markdown_round_trip`); to extend into a property test (`proptest` is already a dependency) |
| R4 | Complex tables in administrative documents (merges, layout tables) | High | Medium | Unrolling of layout tables, report; "keep this table as an image" option to study |
| R5 | **Word 6/95 and earlier** not read | Medium | Medium | Clear message (existing); optional LibreOffice fallback (desktop) |
| R6 | Forge authentication in the browser (CORS, device flow) | High | Medium | Personal tokens first, optional OAuth relay; desktop first |
| R7 | Forgejo API differs between instance versions | Medium | Medium | Version detection, tests against Codeberg and a test instance |
| R8 | **Concurrent editing conflicts** on the same file | Medium | Medium | Expected-`sha` control, merge via `review.compare`, optional advisory lock |
| R9 | Badly stored secrets | Low | High | System keychain, no token written to disk, never exposed to MCP/control |
| R10 | **ArtCraft trademarks / identity** not removed | Medium | High (legal) | Dedicated phase 1, file list in 07 §2 |
| R11 | Divergence from upstream making merges costly | Medium | Medium | No crate renaming, additions in separate modules, regular merges |
| R12 | Web performance (rendering without threads) on 100 pages | Medium | Medium | Measurement in phase 3; the Markdown projection only happens on save |
| R13 | HTML comments altered by third-party tools (Markdown formatters, editors) | Medium | Low | Tolerant block format, orphaned anchors reattached to the paragraph, report on open |
| R14 | Untrusted content (repositories, documents): `..` paths, huge images | Medium | High | `LocalImages` confinement (existing, reused), caps, "never crash" rule and `hostile_params_never_panic` test |
| R15 | Licence: accidental inclusion of GPL code (Pandoc, LibreOffice) | Low | High | External tools as subprocesses only, never linked; the repository's *clean room* rule |
| R16 | The owner's conversion scripts (`convertDoC`) not evaluated | Closed | Low | Comparison dropped by the owner's decision (done without); the WordCraft importer is the only conversion chain (04 §7) |
