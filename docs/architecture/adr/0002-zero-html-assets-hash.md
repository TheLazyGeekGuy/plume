# ADR 0002 — Zero HTML, only HTML comments tolerated, content-named assets

- Status: accepted for increment 1 (2026-10-14)
- Context: "zero HTML except for comments" requirement; documents versioned in Git and read on GitHub/GitLab/Forgejo/VitePress.
- Decision: the file contains only CommonMark + GFM; only `<!-- -->` comment tags are allowed (checked by `markdown::html_lines`). Images are written to `assets/img-<FNV-64 of the content>.<ext>`.
- Consequences: `<br>` in cells and bookmark `<a id>` disappear (reported); image sizes are not preserved in pure GFM; Git diffs stay stable (a re-save renames no image) and identical images are deduplicated.
