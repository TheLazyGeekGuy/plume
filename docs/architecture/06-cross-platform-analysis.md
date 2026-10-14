# 06 — Cross-platform analysis

> Status: audit. Legend: **[code]** verified · **[hypothesis]** to be confirmed.

## 1. Current situation

| Platform | Binary | Existing packaging | State |
|---|---|---|---|
| macOS (Apple Silicon, Intel) | `apps/wordcraft` (eframe) | `packaging/macos` (Info.plist, signed DMG) **[code]** | supported |
| Windows x64 / ARM64 | same | `packaging/windows/wordcraft.wxs` (WiX MSI) **[code]** | supported |
| Linux x64 / ARM64 | same | `packaging/linux` (.desktop, metainfo, MIME), Flatpak **[code]** | supported |
| BSD | same | README (Gentoo overlay mentioned, BSD in CLAUDE.md) | announced |
| Web | `apps/wordcraft-web` (Trunk, eframe wgpu → WebGL2) | `packaging/web/package.sh` **[code]** | supported |

Business code is **fully shared**: every L0–L6 crate builds for `wasm32-unknown-unknown` (`cargo xtask wasm`, `xtask/src/main.rs:126-154`). No rewrite is needed.

## 2. Per-platform capabilities for the project

| Capability | Desktop | Web | Comment |
|---|---|---|---|
| WYSIWYG editing, ribbon, styles | yes | yes | same code |
| DOCX / DOC / RTF / ODT / HTML import | yes | yes | pure-Rust engine |
| Open / save `.md` | yes, with `assets/` | **partial**: one file at a time via the picker; images in `assets/` are not reachable | see §3 |
| PDF export | yes | yes (`print_pdf`, download) | krilla built for WASM |
| Fonts | system fonts + `CRAFT_FONTS_DIR` | embedded fonts + IndexedDB | visual theme: choose bundled free fonts |
| Autosave / recovery | to extend to `.md` | **absent** (`autosave_block`) | IndexedDB/OPFS for the journal |
| Local Git | `gix` | **no** | |
| Forges (REST) | yes | yes, subject to CORS and authentication (05 §4) | |
| System keychain | yes | no (session) | |
| Document browser | existing built-in browser (`ui-egui/src/file_browser.rs`) | not built for WASM | to replace with a "repository" view via the API |
| Render threads | yes (rayon) | 0 (`render/src/lib.rs:24`) | 100 pages: to measure |

## 3. Web-specific obstacles

1. **File system.** The browser only gives one file at a time. For a `.md` + `assets/`, two routes:
   - File System Access API (`showDirectoryPicker`, Chromium only) to open a **folder**;
   - archive: offer a `.zip` download (`document.md` + `assets/`) — the `zip` crate is already a dependency.
   Recommendation: folder on Chromium, `.zip` elsewhere, and **the remote repository as the Web's main mode** (it does not have this problem).
2. **Git.** No processes, no sockets: only the **forges' REST APIs** (05 §3.1). No clone, no SSH.
3. **CORS.** GitHub and GitLab.com accept authenticated API calls from the browser **[hypothesis, to test]**; Forgejo requires CORS to be enabled on the instance; GitHub's OAuth "device flow" endpoints cannot be called from the browser → personal token or relay.
4. **Secrets.** No keychain: token in memory, short lifetime.
5. **Performance.** Rendering without threads: measure editing a 100-page document (09 §6) before promising smoothness.
6. **Size.** The WASM binary embeds the whole engine; adding a Git client in WASM is ruled out (the REST API is enough).

## 4. Abstractions to introduce

| Abstraction | Native | Web |
|---|---|---|
| `HttpTransport` (request → response) | Rust HTTP client (rustls) | `fetch` |
| `SecretStore` | system keychain | session memory |
| `DocumentStore` (read/write `.md` + assets) | file system | folder picker / zip / forge |
| `VersionStore` | `MemoryStore`, `LocalGitStore`, `ForgeStore` | `MemoryStore`, `ForgeStore` |

They join the `Services` mechanism already used by the UI for dialogs (`apps/wordcraft-web/src/web.rs:176`), with no network dependency in `engine`.

## 5. Priorities

| Platform | Specification priority | Extra project effort |
|---|---|---|
| macOS Apple Silicon | P0 | low (identity, signing, application id) |
| macOS Intel, Windows x64, Linux x64 | P1 | low |
| Web | P1 | **medium to high** (storage, forges, CORS, auth) |
| Windows ARM64, Linux ARM64 | P2 | low (existing cross-compilation to verify) |
