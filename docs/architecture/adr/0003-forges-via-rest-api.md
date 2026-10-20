# ADR 0003 — Forge connectors via REST API, local Git via gix

- Status: proposed (2026-10-14)
- Context: a single Markdown file open at a time; GitHub, GitLab, Forgejo; Web version in WASM with no file system or processes.
- Decision: for remote repositories, read/write the file and list its history through the forges' REST APIs (one commit per "Save a version", concurrency control by expected `sha`). For local repositories (desktop), `gix`; the `git` command as a fallback for SSH push/pull. HTTP transport and secret storage are services injected by the application; `engine` stays network-free.
- Consequences: same behaviour on desktop and Web; no C dependency; CORS and authentication to handle for the Web; no complex multi-file operations (not needed).
