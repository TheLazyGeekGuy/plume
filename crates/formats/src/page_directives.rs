//! Typed, bounded parser for Plume page-layout fenced directives.
//!
//! This is a source-level scanner, not a paginator. The document model and exporters
//! must consume these events before page orientation can affect rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation { Portrait, Landscape }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper { A3, A4, A5, Letter, Legal }

#[derive(Clone, Debug, PartialEq)]
pub struct PageSpec {
    pub orientation: Option<Orientation>,
    pub paper: Option<Paper>,
    /// Uniform margin, in points.
    pub margin_pt: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageEvent {
    Begin { line: usize, spec: PageSpec },
    End { line: usize },
    Break { line: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: &'static str,
}

const MAX_DIRECTIVE_LENGTH: usize = 1024;
const MAX_DEPTH: usize = 32;
const MAX_EVENTS: usize = 10_000;

/// Reads page directives while ignoring ordinary fenced code blocks.
/// Returns a precise error rather than silently changing page geometry.
pub fn scan(source: &str) -> Result<Vec<PageEvent>, ParseError> {
    let mut events = Vec::new();
    let mut depth = 0usize;
    let mut code_fence: Option<(char, usize)> = None;
    for (index, raw) in source.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if let Some((mark, count)) = code_fence {
            let length = trimmed.chars().take_while(|c| *c == mark).count();
            if length >= count && trimmed[length..].trim().is_empty() {
                code_fence = None;
            }
            continue;
        }
        let ticks = trimmed.chars().take_while(|c| *c == '`').count();
        let tildes = trimmed.chars().take_while(|c| *c == '~').count();
        if ticks >= 3 || tildes >= 3 {
            code_fence = Some(if ticks >= 3 { ('`', ticks) } else { ('~', tildes) });
            continue;
        }
        if !trimmed.starts_with(":::") { continue; }
        if trimmed.len() > MAX_DIRECTIVE_LENGTH {
            return Err(ParseError { line, message: "page directive too long" });
        }
        if trimmed == ":::" {
            if depth > 0 {
                depth -= 1;
                events.push(PageEvent::End { line });
            }
            continue;
        }
        let Some(inner) = trimmed.strip_prefix("::: {").and_then(|s| s.strip_suffix('}')) else { continue; };
        let mut fields = inner.split_whitespace();
        let Some(kind) = fields.next() else { continue; };
        if kind == ".page-break" {
            if fields.next().is_some() { return Err(ParseError { line, message: "page break takes no attributes" }); }
            events.push(PageEvent::Break { line });
        } else if kind == ".page" {
            if depth >= MAX_DEPTH { return Err(ParseError { line, message: "page directive nesting limit exceeded" }); }
            let mut spec = PageSpec { orientation: None, paper: None, margin_pt: None };
            for field in fields {
                let Some((key, value)) = field.split_once('=') else {
                    return Err(ParseError { line, message: "invalid page attribute" });
                };
                let value = value.trim_matches('"').trim_matches('\'');
                match key {
                    "orientation" => {
                        spec.orientation = Some(match value {
                            "portrait" => Orientation::Portrait,
                            "landscape" => Orientation::Landscape,
                            _ => return Err(ParseError { line, message: "invalid orientation" }),
                        });
                    }
                    "size" => {
                        spec.paper = Some(match value {
                            "A3" => Paper::A3, "A4" => Paper::A4, "A5" => Paper::A5,
                            "Letter" => Paper::Letter, "Legal" => Paper::Legal,
                            _ => return Err(ParseError { line, message: "invalid paper size" }),
                        });
                    }
                    "margin" => {
                        let Some(amount) = value.strip_suffix("mm") else {
                            return Err(ParseError { line, message: "margin must be in mm" });
                        };
                        let Ok(mm) = amount.parse::<f32>() else {
                            return Err(ParseError { line, message: "invalid margin" });
                        };
                        if !mm.is_finite() || !(0.0..=100.0).contains(&mm) {
                            return Err(ParseError { line, message: "margin out of range" });
                        }
                        spec.margin_pt = Some(mm * 72.0 / 25.4);
                    }
                    _ => return Err(ParseError { line, message: "unsupported page attribute" }),
                }
            }
            depth += 1;
            events.push(PageEvent::Begin { line, spec });
        }
        if events.len() > MAX_EVENTS { return Err(ParseError { line, message: "too many page directives" }); }
    }
    if depth != 0 {
        return Err(ParseError { line: source.lines().count(), message: "unclosed page directive" });
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portrait_landscape_portrait() {
        let input = "# Intro\n::: {.page orientation=\"landscape\" size=\"A3\" margin=\"10mm\"}\nWide\n:::\n# End";
        let events = scan(input).unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], PageEvent::Begin { line: 2, spec: PageSpec { orientation: Some(Orientation::Landscape), paper: Some(Paper::A3), .. } }));
        assert_eq!(events[1], PageEvent::End { line: 4 });
    }
    #[test]
    fn code_is_not_a_directive() {
        assert!(scan("```markdown\n::: {.page orientation=\"landscape\"}\n```").unwrap().is_empty());
    }
    #[test]
    fn explicit_page_break() {
        assert_eq!(scan("::: {.page-break}").unwrap(), vec![PageEvent::Break { line: 1 }]);
    }
    #[test]
    fn invalid_values_are_rejected() {
        assert!(scan("::: {.page orientation=\"sideways\"}\n:::").is_err());
        assert!(scan("::: {.page margin=\"NaNmm\"}\n:::").is_err());
        assert!(scan("::: {.page size=\"A0\"}\n:::").is_err());
    }
    #[test]
    fn unclosed_section_is_rejected() {
        assert!(scan("::: {.page orientation=\"landscape\"}").is_err());
    }
}


/// Import a Markdown document containing page directives into real document sections.
///
/// Returns None for ordinary Markdown or malformed directives, leaving legacy import intact.
/// Each fenced page span begins on a new physical page. The closing fence restores the
/// previous section setup. A section break lives on the preceding paragraph, as in OOXML.
pub fn import_paged(text: &str, base: &wordcraft_doc::SectionProps) -> Option<wordcraft_doc::Document> {
    use wordcraft_doc::{Block, Paragraph, SectionProps, para_block};
    use std::sync::Arc;

    let events = scan(text).ok()?;
    if events.is_empty() {
        return None;
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks: Vec<(String, SectionProps)> = Vec::new();
    let mut current = base.clone();
    let mut stack: Vec<SectionProps> = Vec::new();
    let mut from = 0usize;
    for event in &events {
        let line = match event {
            PageEvent::Begin { line, .. } | PageEvent::End { line } | PageEvent::Break { line } => *line,
        };
        let before = line.saturating_sub(1).min(lines.len());
        if before > from {
            chunks.push((lines[from..before].join("\n"), current.clone()));
        }
        match event {
            PageEvent::Begin { spec, .. } => {
                stack.push(current.clone());
                apply_spec(&mut current, spec);
            }
            PageEvent::End { .. } => {
                current = stack.pop().unwrap_or_else(|| base.clone());
            }
            PageEvent::Break { .. } => {}
        }
        from = line.min(lines.len());
    }
    if from < lines.len() {
        chunks.push((lines[from..].join("\n"), current));
    }
    if chunks.is_empty() {
        return None;
    }
    let mut combined: Option<wordcraft_doc::Document> = None;
    for (source, settings) in chunks {
        if source.trim().is_empty() {
            continue;
        }
        let mut part = crate::model::to_doc_in(&crate::markdown::parse(&source), settings.clone());
        if let Some(document) = combined.as_mut() {
            // Attach a real section boundary to the previous section's last paragraph.
            if !matches!(document.body.last().map(|b| b.as_ref()), Some(Block::Para(_))) {
                document.body.push(para_block(Paragraph::new()));
            }
            if let Some(last) = document.body.last_mut() {
                if let Block::Para(p) = Arc::make_mut(last) {
                    p.section = Some(Box::new(document.last_section.clone()));
                }
            }
            document.body.append(&mut part.body);
            document.last_section = settings;
        } else {
            combined = Some(part);
        }
    }
    combined
}

fn apply_spec(section: &mut wordcraft_doc::SectionProps, spec: &PageSpec) {
    if let Some(paper) = spec.paper {
        let (w, h) = match paper {
            Paper::A3 => (841.89, 1190.55),
            Paper::A4 => (595.28, 841.89),
            Paper::A5 => (419.53, 595.28),
            Paper::Letter => (612.0, 792.0),
            Paper::Legal => (612.0, 1008.0),
        };
        section.page_w = w;
        section.page_h = h;
    }
    if let Some(orientation) = spec.orientation {
        section.landscape = orientation == Orientation::Landscape;
    }
    let needs_swap = (section.page_w > section.page_h) != section.landscape;
    if needs_swap {
        std::mem::swap(&mut section.page_w, &mut section.page_h);
    }
    if let Some(m) = spec.margin_pt {
        section.margin_top = m;
        section.margin_bottom = m;
        section.margin_left = m;
        section.margin_right = m;
    }
    section.start = wordcraft_doc::section::SectionStart::NextPage;
}
