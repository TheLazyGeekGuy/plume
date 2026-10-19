//! Plume: Word's SmartArt graphics and charts written as Mermaid, so a converted document keeps
//! them as text: a process as a flow left to right, a cycle closing on itself, a hierarchy as a
//! tree, lists side by side; a pie chart as a `pie`, columns, bars and lines as an `xychart`.

use wordcraft_doc::chart::{ChartSpec, ChartType};
use wordcraft_doc::smart_art::{SmartArtLayout, SmartArtSpec};

/// Longest label written.
const MAX_LABEL: usize = 120;
/// Most items or points written.
const MAX_ITEMS: usize = 200;

/// `s` as a quoted Mermaid label.
fn label(s: &str) -> String {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_LABEL).collect();
    format!("\"{}\"", t.replace('"', "#quot;"))
}

/// A SmartArt graphic as a Mermaid flowchart.
pub fn from_smart_art(spec: &SmartArtSpec) -> Option<String> {
    let items: Vec<(usize, &str, u8)> = spec
        .items
        .iter()
        .enumerate()
        .take(MAX_ITEMS)
        .filter(|(_, i)| !i.text.trim().is_empty())
        .map(|(k, i)| (k, i.text.as_str(), i.level))
        .collect();
    if items.is_empty() {
        return None;
    }
    let dir = match spec.layout {
        SmartArtLayout::Hierarchy | SmartArtLayout::Pyramid | SmartArtLayout::Radial => "TD",
        _ => "LR",
    };
    let mut out = vec![format!("flowchart {dir}")];
    for (k, t, _) in &items {
        out.push(format!("    n{k}[{}]", label(t)));
    }
    // Each item under the item one level up before it.
    let mut parents: Vec<usize> = Vec::new();
    let mut tops: Vec<usize> = Vec::new();
    for (k, _, level) in &items {
        parents.truncate(*level as usize);
        match parents.last() {
            Some(p) => out.push(format!("    n{p} {} n{k}", if spec.layout == SmartArtLayout::Hierarchy { "-->" } else { "---" })),
            None => tops.push(*k),
        }
        parents.push(*k);
    }
    match spec.layout {
        SmartArtLayout::Process | SmartArtLayout::Cycle => {
            for w in tops.windows(2) {
                out.push(format!("    n{} --> n{}", w[0], w[1]));
            }
            if spec.layout == SmartArtLayout::Cycle
                && tops.len() > 1
                && let (Some(last), Some(first)) = (tops.last(), tops.first())
            {
                out.push(format!("    n{last} --> n{first}"));
            }
        }
        // The first item in the middle, the others around it.
        SmartArtLayout::Radial => {
            if let Some((first, rest)) = tops.split_first() {
                for k in rest {
                    out.push(format!("    n{first} --- n{k}"));
                }
            }
        }
        _ => {}
    }
    Some(out.join("\n"))
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{v:.0}") } else { format!("{v}") }
}

/// A chart as a Mermaid `pie` or `xychart`; `None` for a scatter chart (no Mermaid form).
pub fn from_chart(spec: &ChartSpec) -> Option<String> {
    let first = spec.series.first()?;
    let title = spec.title.as_deref().filter(|t| !t.trim().is_empty());
    match spec.kind {
        ChartType::Pie | ChartType::Doughnut => {
            let mut out = vec![match title {
                Some(t) => format!("pie title {}", t.split_whitespace().collect::<Vec<_>>().join(" ")),
                None => "pie".to_string(),
            }];
            for (c, v) in spec.categories.iter().zip(&first.values).take(MAX_ITEMS) {
                if let Some(v) = v.filter(|v| *v > 0.0) {
                    out.push(format!("    {} : {}", label(c), num(v)));
                }
            }
            (out.len() > 1).then(|| out.join("\n"))
        }
        ChartType::Scatter => None,
        kind => {
            let horizontal = matches!(kind, ChartType::Bar | ChartType::BarStacked);
            let mut out = vec![if horizontal { "xychart-beta horizontal".to_string() } else { "xychart-beta".to_string() }];
            if let Some(t) = title {
                out.push(format!("    title {}", label(t)));
            }
            let cats: Vec<String> = spec.categories.iter().take(MAX_ITEMS).map(|c| label(c)).collect();
            out.push(format!("    x-axis [{}]", cats.join(", ")));
            let mark = if matches!(kind, ChartType::Line | ChartType::Area) { "line" } else { "bar" };
            for s in spec.series.iter().take(16) {
                let vals: Vec<String> = s.values.iter().take(cats.len()).map(|v| num(v.unwrap_or(0.0))).collect();
                out.push(format!("    {mark} [{}]", vals.join(", ")));
            }
            Some(out.join("\n"))
        }
    }
}

/// A label read back: quotes and `#quot;` undone.
fn unlabel(s: &str) -> String {
    let s = s.trim();
    s.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or(s).replace("#quot;", "\"").chars().take(MAX_LABEL).collect()
}

/// `[a, "b", 3]`'s items (commas inside quotes kept).
fn list(s: &str) -> Option<Vec<String>> {
    let inner = s.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    for c in inner.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            ',' if !quoted => out.push(unlabel(&std::mem::take(&mut cur))),
            _ => cur.push(c),
        }
        if out.len() > MAX_ITEMS {
            return None;
        }
    }
    if !cur.trim().is_empty() {
        out.push(unlabel(&cur));
    }
    Some(out)
}

/// A Mermaid `pie` or `xychart-beta` read back as a chart, for the chart editor; `None` for
/// anything else or what the editor can't show.
pub fn chart_of(src: &str) -> Option<ChartSpec> {
    let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("%%"));
    let head = lines.next()?;
    if let Some(rest) = head.strip_prefix("pie") {
        let mut rest = rest.trim();
        rest = rest.strip_prefix("showData").map_or(rest, str::trim);
        let title = rest.strip_prefix("title").map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        let (mut categories, mut values) = (Vec::new(), Vec::new());
        for l in lines.take(MAX_ITEMS + 1) {
            let (c, v) = l.rsplit_once(':')?;
            categories.push(unlabel(c));
            values.push(Some(v.trim().parse::<f64>().ok().filter(|f| f.is_finite())?));
        }
        return Some(ChartSpec {
            kind: ChartType::Pie,
            categories,
            series: vec![wordcraft_doc::chart::ChartSeries { name: String::new(), values }],
            title,
            ..Default::default()
        });
    }
    let horizontal = match head {
        "xychart-beta" | "xychart" => false,
        "xychart-beta horizontal" | "xychart horizontal" => true,
        _ => return None,
    };
    let (mut title, mut categories, mut series, mut lines_only) = (None, Vec::new(), Vec::new(), true);
    for l in lines.take(40) {
        if let Some(t) = l.strip_prefix("title") {
            title = Some(unlabel(t)).filter(|t| !t.is_empty());
        } else if let Some(x) = l.strip_prefix("x-axis") {
            categories = list(x)?;
        } else if l.starts_with("y-axis") {
        } else if let Some((mark, vals)) = l.split_once(' ') {
            if !matches!(mark, "bar" | "line") {
                return None;
            }
            lines_only &= mark == "line";
            let values = list(vals)?.iter().map(|v| v.parse::<f64>().ok().filter(|f| f.is_finite())).collect::<Vec<_>>();
            series.push(wordcraft_doc::chart::ChartSeries { name: format!("Series {}", series.len() + 1), values });
        } else {
            return None;
        }
    }
    if series.is_empty() {
        return None;
    }
    let kind = if lines_only {
        ChartType::Line
    } else if horizontal {
        ChartType::Bar
    } else {
        ChartType::Column
    };
    Some(ChartSpec { kind, categories, series, title, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use wordcraft_doc::chart::ChartSeries;
    use wordcraft_doc::smart_art::SmartArtItem;

    use super::*;

    #[test]
    fn smart_art_and_charts_become_mermaid_that_draws() {
        let process = SmartArtSpec {
            layout: SmartArtLayout::Process,
            items: vec![SmartArtItem::new("Write", 0), SmartArtItem::new("Review \"twice\"", 0), SmartArtItem::new("Publish", 0)],
            ..Default::default()
        };
        let src = from_smart_art(&process).unwrap();
        assert_eq!(src, "flowchart LR\n    n0[\"Write\"]\n    n1[\"Review #quot;twice#quot;\"]\n    n2[\"Publish\"]\n    n0 --> n1\n    n1 --> n2");
        let tree = SmartArtSpec {
            layout: SmartArtLayout::Hierarchy,
            items: vec![SmartArtItem::new("Lead", 0), SmartArtItem::new("Design", 1), SmartArtItem::new("Build", 1)],
            ..Default::default()
        };
        let pie = ChartSpec {
            kind: ChartType::Pie,
            categories: vec!["Paper".into(), "Ink".into()],
            series: vec![ChartSeries { name: "Costs".into(), values: vec![Some(30.0), Some(12.5)] }],
            title: Some("Costs".into()),
            ..Default::default()
        };
        assert_eq!(from_chart(&pie).unwrap(), "pie title Costs\n    \"Paper\" : 30\n    \"Ink\" : 12.5");
        let columns = ChartSpec { kind: ChartType::Column, ..pie.clone() };
        for src in [src, from_smart_art(&tree).unwrap(), from_chart(&pie).unwrap(), from_chart(&columns).unwrap()] {
            assert!(crate::draw(&src).is_ok(), "{src}");
        }
        for c in [pie.clone(), columns.clone(), ChartSpec { kind: ChartType::Line, ..pie.clone() }] {
            let back = chart_of(&from_chart(&c).unwrap()).unwrap();
            assert_eq!((back.kind, &back.categories, &back.series[0].values, &back.title), (c.kind, &c.categories, &c.series[0].values, &c.title));
        }
        assert!(
            chart_of("pie\n \"a\" : x").is_none()
                && chart_of("gantt").is_none()
                && chart_of("xychart-beta\n x-axis [a, \"b, c\"]\n bar [1, 2]").is_some()
        );
        assert!(from_chart(&ChartSpec { kind: ChartType::Scatter, ..pie }).is_none());
    }
}
