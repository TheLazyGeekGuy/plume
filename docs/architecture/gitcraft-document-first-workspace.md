# GitCraft: document-first workspace and review panel

Status: interface contract / implementation plan. **The layout described here is not yet implemented.**

## Layout

The editor is the primary workspace. On a typical desktop window, start with:

| Region | Initial width | Behavior |
| --- | ---: | --- |
| GitCraft branches and commit graph (left) | 16% | resizable, collapsible, min/max bounded |
| WYSIWYG Markdown document (center) | 64% | priority region, never covered by sidebars |
| Review (right) | 20% | resizable, collapsible, contextual |

The percentages are preferences, not hardcoded constraints. At narrower widths collapse the panels automatically rather than forcing the document to become unreadable. Persist visibility and user-adjusted widths per workspace. A focus-writing mode collapses both panels. Preserve the existing WordCraft comment balloons, with an option to move review conversations into the right panel rather than showing the same comment twice.

## Left: GitCraft

- Persistent repository identity, current local branch and configured upstream.
- True Git **commit graph** with merge parents, branch labels, divergence points, and selectable commits (not a flat branch list).
- Working-tree dirty marker, uncommitted changes, unpublished commits, remote updates, diverged status; counts computed using merge-base/ahead-behind against the upstream.
- Refresh/fetch asynchronously; never imply local status is current if remote status has not been fetched. Explicit timestamp and offline state.
- Branch switching must guard unsaved editor state and dirty working trees. Never discard changes automatically.

## Center: editing

- The active Markdown document remains the focus and receives keyboard focus by default.
- Clicking a Git commit compares content but does not replace the active document without explicit confirmation.
- Clicking a reliably anchored comment scrolls to the content and marks its range.
- Unsaved edits always remain visually distinct from committed Git changes.

## Right: Review

Three tabs:

1. **Discussions:** unified local document comments and GitHub/GitLab/Forgejo commit and merge request threads. Show source, author, status, replies, and the upstream link. Filters for open/resolved, current file and source. Keep source ownership: a reply to a remote thread must go to the forge, not silently into Markdown.
2. **Differences:** Git changed hunks with optional Markdown block-aware segmentation (heading, paragraph, list, table). Only offer selective application after validating the base revision and conflict-free mapping.
3. **Activity:** newer commits, push/fetch events, revision and branch status. A remote comment is not evidence of a coeditor actively typing.

Anchor strategy: document markers can be linked immediately; remote diff positions are revision-specific and MUST be remapped and validated before marking text. Unmapped/outdated threads remain visible as unanchored conversations.

## Progressive delivery

- Stage A: review data model and unit tests (started in PR #7).
- Stage B: adapter from `Document.comments`, render collapsible/right egui panel, document navigation and tests.
- Stage C: forge discussion adapters (GitHub, GitLab, Forgejo), read/reply/resolve with source-specific permission handling.
- Stage D: diff panel, graph/sidebar, asynchronous refresh, ahead/behind and merge-base.
- Stage E: notifications by Markdown block and safe selective integration.
- Stage F: presence / simultaneous editing requires a dedicated collaboration protocol (for example, CRDT), not just Git.

## Validation before a testable macOS release

- Unit tests and clippy for the VCS crate.
- UI smoke tests for narrow/wide windows, dragging sidebars, comment navigation and keyboard-only operation.
- End-to-end comparison of document and forge threads, including stale anchors and permission failures.
- Build on macOS Apple Silicon; publish a clearly labeled test artifact with commit SHA and signing status.

## Acceptance criterion

The document must remain the largest region by far. Neither GitCraft nor Review may obstruct writing, and their collapsed state must survive reopening the workspace.
