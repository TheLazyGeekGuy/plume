# PLUME-SPEC-012: mixed page orientation

Status: implementation in progress (Markdown import -> document section integration).

## Syntax

```markdown
# Portrait introduction

::: {.page orientation="landscape" size="A3" margin="10mm"}
A wide table.
:::

# Portrait conclusion

::: {.page-break}
:::
```

This is a Plume extension using Pandoc-style fenced div spelling, not CommonMark.
The GFM publishing profile must warn when directives would not be interpreted by
GitHub; ordinary text and diagrams remain usable.

## Implemented

- `wordcraft_formats::page_directives::scan`: bounded parser of page section
  open/close and page break events, rejecting invalid units and unsupported values.
- Explicit unit tests for valid, invalid, unfinished, and fenced-code cases.
- Markdown `import` and `import_on` now translate valid directives into
  section boundaries and restore the previous geometry after closing fences.
- Existing layout, PDF and print engines consume document sections; these engines
  themselves have not been modified.
- Native Markdown save and image-resolving import are not yet round-trip complete.

## Follow-up integration tasks

1. Improve the current source chunk importer to preserve numbered lists and front matter
   across boundaries, and handle image-resolving import.
2. Add negative tests for unrecognised containers and deeper nested sections.
3. Update native Markdown export to emit the same directives after save/import/save.
4. Wire layout and PDF output to physical page size, not merely PDF /Rotate.
5. Verify DOCX `w:sectPr` / `w:pgSz` / `w:pgMar` imports and exports.
6. Add golden PDF/DOCX fixtures, pagination tests and UI selection controls.
7. Run `cargo fmt --check`, `cargo test -p wordcraft-formats` and repository
   `cargo xtask ci` on a Rust-enabled runner before marking ready for merge.

No CI execution has been claimed for this initial commit.
