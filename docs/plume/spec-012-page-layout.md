# PLUME-SPEC-012: mixed page orientation

Status: implementation in progress (parser increment).

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
- The API is intentionally **not** wired to Markdown import yet; interpreting
  a directive as a plain paragraph would be misleading.

## Follow-up integration tasks

1. Extend Markdown's flow model with section boundary metadata; protect directives
   from being treated as ordinary paragraphs, without losing unrecognised fenced divs.
2. Map section boundaries to `Document` section properties and section breaks.
   Retain previous page settings after `:::`, with next-page break semantics.
3. Update native Markdown export to emit the same directives after save/import/save.
4. Wire layout and PDF output to physical page size, not merely PDF /Rotate.
5. Verify DOCX `w:sectPr` / `w:pgSz` / `w:pgMar` imports and exports.
6. Add golden PDF/DOCX fixtures, pagination tests and UI selection controls.
7. Run `cargo fmt --check`, `cargo test -p wordcraft-formats` and repository
   `cargo xtask ci` on a Rust-enabled runner before marking ready for merge.

No CI execution has been claimed for this initial commit.
