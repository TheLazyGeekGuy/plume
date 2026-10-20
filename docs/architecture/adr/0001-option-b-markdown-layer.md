# ADR 0001 — Option B: keep WordCraft, Markdown profile at the persistence boundary

- Status: proposed (2026-10-14)
- Context: the specification requires Markdown as the only save format, with no silent loss, while keeping WordCraft's Word experience. The `Document` model is much richer than Markdown; an intermediate `Flow` model already exists for all text formats (`crates/formats/src/model.rs`).
- Decision: do not change `doc`, `layout`, `render`, `pdf`, `docx`, `docbin`. Add a native Markdown profile in `crates/formats/src/markdown.rs` (`export_native`, `import_with`, `Report`) and connect the engine's I/O to it.
- Consequences: editing keeps every WordCraft feature; losses happen on save, once only, and are reported; some commands will have to be flagged or greyed out (strict/lenient mode).
- Alternatives rejected: option A (direct save: silent loss or hidden HTML), option C (alternative Markdown engine: nothing justifies it).
