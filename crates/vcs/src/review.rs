//! Forge-neutral review threads for GitCraft's contextual review pane.
//!
//! Keep document comments, commit discussions and merge-request review threads
//! distinguishable even when presented together. No UI or network code lives here.

/// Where a discussion originated. The source is never silently rewritten.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReviewSource {
    /// Comment stored within the Markdown document.
    Document,
    /// Discussion tied to a Git commit; may not have a text range.
    Commit { revision: String },
    /// Discussion linked to a pull or merge request on the remote forge.
    MergeRequest { number: u64 },
}

/// Where the discussion belongs in the document, if known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewAnchor {
    /// Visible in the global discussions list, not attached to document text.
    General,
    /// A durable document comment marker.
    DocumentMarker(String),
    /// Position in a particular immutable revision, pending a safe remap.
    DiffLine {
        path: String,
        revision: String,
        line: u32,
    },
}

/// Resolution is source-dependent; unknown is never treated as resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewStatus {
    Open,
    Resolved,
    Unknown,
}

/// A normalized view of a comment thread, not an alternative source of truth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewThread {
    /// Namespace-qualified ID, e.g. "document:c1" or "forge:mr:12:discussion:34".
    pub id: String,
    pub source: ReviewSource,
    pub anchor: ReviewAnchor,
    pub status: ReviewStatus,
    pub author: String,
    pub body: String,
    pub external_url: Option<String>,
}

impl ReviewThread {
    /// Only actual document markers may be displayed inline unconditionally.
    /// Diff lines must be remapped against the currently open document first.
    pub fn can_show_inline(&self) -> bool {
        matches!(self.anchor, ReviewAnchor::DocumentMarker(_))
    }
}

/// Combine feeds without collapsing distinct threads that happen to contain
/// identical text. An existing ID keeps its position; the newer entry wins.
pub fn reconcile_threads(
    existing: impl IntoIterator<Item = ReviewThread>,
    incoming: impl IntoIterator<Item = ReviewThread>,
) -> Vec<ReviewThread> {
    let mut threads: Vec<ReviewThread> = existing.into_iter().collect();
    for update in incoming {
        if let Some(index) = threads.iter().position(|item| item.id == update.id) {
            threads[index] = update;
        } else {
            threads.push(update);
        }
    }
    threads
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, source: ReviewSource, anchor: ReviewAnchor) -> ReviewThread {
        ReviewThread {
            id: id.into(),
            source,
            anchor,
            status: ReviewStatus::Open,
            author: "Alice".into(),
            body: "Check wording".into(),
            external_url: None,
        }
    }

    #[test]
    fn only_document_markers_are_immediately_inline() {
        assert!(sample("doc:c1", ReviewSource::Document, ReviewAnchor::DocumentMarker("c1".into())).can_show_inline());
        assert!(!sample("forge:commit:1", ReviewSource::Commit { revision: "abc1234".into() }, ReviewAnchor::General).can_show_inline());
        assert!(!sample("forge:mr:2", ReviewSource::MergeRequest { number: 2 }, ReviewAnchor::DiffLine {
            path: "guide.md".into(), revision: "abc1234".into(), line: 42,
        }).can_show_inline());
    }

    #[test]
    fn same_text_from_two_sources_does_not_collapse() {
        let doc = sample("doc:c1", ReviewSource::Document, ReviewAnchor::DocumentMarker("c1".into()));
        let mr = sample("forge:mr:12:discussion:3", ReviewSource::MergeRequest { number: 12 }, ReviewAnchor::General);
        assert_eq!(reconcile_threads([doc], [mr]).len(), 2);
    }

    #[test]
    fn remote_status_refresh_replaces_same_thread() {
        let open = sample("forge:mr:12:discussion:3", ReviewSource::MergeRequest { number: 12 }, ReviewAnchor::General);
        let mut resolved = open.clone();
        resolved.status = ReviewStatus::Resolved;
        let result = reconcile_threads([open], [resolved.clone()]);
        assert_eq!(result, vec![resolved]);
    }
}
