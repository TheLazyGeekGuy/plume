//! Plume: what a SmartArt graphic or a chart says, read from its parts whatever program made
//! them: the SmartArt's items and their levels (its data model, ECMA-376 Part 1 §21.4.3) and
//! the kind of layout; a chart's type, categories and series (its cached values, §21.2). Enough
//! to write the graphic again as a diagram in text (Mermaid); not enough to edit it in Word's
//! terms, which is why these are not [`crate::smart_art`]'s exact readers.

use std::collections::{BTreeMap, HashMap};

use wordcraft_doc::chart::{ChartSeries, ChartSpec, ChartType};
use wordcraft_doc::smart_art::{MAX_LEVEL, SmartArtItem, SmartArtLayout, SmartArtSpec};

use crate::xml::{El, MAX_DEPTH, parse};

/// Most items or points read.
const MAX_ITEMS: usize = 500;

/// The descendants of `e` named `local` (any prefix), depth-first, bounded.
fn all<'a>(e: &'a El, local: &str, out: &mut Vec<&'a El>, depth: usize) {
    if depth > MAX_DEPTH || out.len() >= MAX_ITEMS * 4 {
        return;
    }
    for c in e.els() {
        if c.local() == local {
            out.push(c);
        }
        all(c, local, out, depth + 1);
    }
}

fn first<'a>(e: &'a El, local: &str) -> Option<&'a El> {
    let mut v = Vec::new();
    all(e, local, &mut v, 0);
    v.into_iter().next()
}

fn kids<'a>(e: &'a El, local: &'a str) -> impl Iterator<Item = &'a El> + 'a {
    e.els().filter(move |c| c.local() == local)
}

/// A text element's paragraphs, joined with spaces.
fn text_of(e: &El) -> String {
    let mut ts = Vec::new();
    all(e, "t", &mut ts, 0);
    ts.iter().map(|t| t.text()).collect::<Vec<_>>().join("").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A SmartArt graphic's outline from its data part (`diagrams/dataN.xml`).
pub fn smart_art_outline(data: &[u8]) -> Option<SmartArtSpec> {
    let root = parse(data).ok()?;
    let pt_list = first(&root, "ptLst")?;
    let mut text: HashMap<String, String> = HashMap::new();
    let mut doc_id = None;
    let mut layout = SmartArtLayout::BasicList;
    for pt in kids(pt_list, "pt").take(MAX_ITEMS * 4) {
        let Some(id) = pt.attr("modelId") else { continue };
        match pt.attr("type").unwrap_or("node") {
            "doc" => {
                doc_id = Some(id.to_string());
                let lo = first(pt, "prSet").and_then(|p| p.attr("loTypeId")).unwrap_or("").to_ascii_lowercase();
                layout = layout_of(&lo);
            }
            "node" => {
                text.insert(id.to_string(), first(pt, "t").map(text_of).unwrap_or_default());
            }
            _ => {}
        }
    }
    let doc_id = doc_id?;
    // Parent → children in order (`parOf` connections, the default type).
    let mut children: HashMap<String, BTreeMap<(u32, usize), String>> = HashMap::new();
    if let Some(cxns) = first(&root, "cxnLst") {
        for (k, c) in kids(cxns, "cxn").take(MAX_ITEMS * 4).enumerate() {
            if c.attr("type").unwrap_or("parOf") != "parOf" {
                continue;
            }
            let (Some(src), Some(dest)) = (c.attr("srcId"), c.attr("destId")) else { continue };
            let ord = c.attr("srcOrd").and_then(|o| o.parse().ok()).unwrap_or(0);
            children.entry(src.to_string()).or_default().insert((ord, k), dest.to_string());
        }
    }
    let mut items = Vec::new();
    let mut stack: Vec<(String, u8)> = children.get(&doc_id).map(|m| m.values().rev().map(|d| (d.clone(), 0)).collect()).unwrap_or_default();
    while let Some((id, level)) = stack.pop() {
        if items.len() >= MAX_ITEMS {
            break;
        }
        let Some(t) = text.get(&id) else { continue };
        items.push(SmartArtItem::new(t.clone(), level.min(MAX_LEVEL)));
        if level < MAX_LEVEL
            && let Some(m) = children.get(&id)
        {
            stack.extend(m.values().rev().map(|d| (d.clone(), level + 1)));
        }
    }
    items.retain(|i| !i.text.trim().is_empty());
    (!items.is_empty()).then(|| SmartArtSpec { layout, items, ..Default::default() })
}

/// The layout a SmartArt layout id (`urn:microsoft.com/office/officeart/2005/8/layout/process1`)
/// is closest to.
fn layout_of(id: &str) -> SmartArtLayout {
    let has = |w: &str| id.contains(w);
    if has("cycle") {
        SmartArtLayout::Cycle
    } else if has("hierarchy") || has("orgchart") {
        SmartArtLayout::Hierarchy
    } else if has("pyramid") {
        SmartArtLayout::Pyramid
    } else if has("radial") {
        SmartArtLayout::Radial
    } else if has("matrix") {
        SmartArtLayout::Matrix
    } else if has("venn") {
        SmartArtLayout::Venn
    } else if has("process") || has("chevron") || has("arrow") {
        SmartArtLayout::Process
    } else {
        SmartArtLayout::BasicList
    }
}

/// A cache's points (`c:pt idx` with `c:v`), by index.
fn points(e: &El) -> Vec<(usize, String)> {
    let mut pts = Vec::new();
    all(e, "pt", &mut pts, 0);
    pts.iter().take(MAX_ITEMS).filter_map(|p| Some((p.attr("idx")?.parse().ok()?, first(p, "v").map(|v| v.text()).unwrap_or_default()))).collect()
}

/// A chart's data from its chart part (`charts/chartN.xml`): the first plot's type, its
/// categories and series, and the title.
pub fn chart_outline(chart: &[u8]) -> Option<ChartSpec> {
    let root = parse(chart).ok()?;
    let plot_area = first(&root, "plotArea")?;
    let plot = plot_area.els().find(|e| e.local().ends_with("Chart"))?;
    let name = plot.local();
    let stacked = kids(plot, "grouping").next().and_then(|g| g.attr("val")).is_some_and(|v| v.contains("tacked"));
    let horizontal = kids(plot, "barDir").next().and_then(|g| g.attr("val")) == Some("bar");
    let kind = match name {
        n if n.starts_with("bar") => match (horizontal, stacked) {
            (true, true) => ChartType::BarStacked,
            (true, false) => ChartType::Bar,
            (false, true) => ChartType::ColumnStacked,
            (false, false) => ChartType::Column,
        },
        n if n.starts_with("line") => ChartType::Line,
        n if n.starts_with("area") => ChartType::Area,
        n if n.starts_with("pie") || n.starts_with("ofPie") => ChartType::Pie,
        n if n.starts_with("doughnut") => ChartType::Doughnut,
        n if n.starts_with("scatter") => ChartType::Scatter,
        _ => return None,
    };
    let mut categories: Vec<String> = Vec::new();
    let mut series = Vec::new();
    for ser in kids(plot, "ser").take(64) {
        let name = kids(ser, "tx").next().map(|t| first(t, "v").map(|v| v.text()).unwrap_or_else(|| text_of(t))).unwrap_or_default();
        let (cat, val) = if kind == ChartType::Scatter { ("xVal", "yVal") } else { ("cat", "val") };
        if categories.is_empty()
            && let Some(c) = kids(ser, cat).next()
        {
            let pts = points(c);
            let n = pts.iter().map(|(i, _)| i + 1).max().unwrap_or(0).min(MAX_ITEMS);
            categories = vec![String::new(); n];
            for (i, v) in pts {
                if let Some(slot) = categories.get_mut(i) {
                    *slot = v;
                }
            }
        }
        let mut values: Vec<Option<f64>> = vec![None; categories.len()];
        if let Some(v) = kids(ser, val).next() {
            for (i, x) in points(v) {
                if i >= values.len() && i < MAX_ITEMS {
                    values.resize(i + 1, None);
                }
                if let Some(slot) = values.get_mut(i) {
                    *slot = x.trim().parse::<f64>().ok().filter(|f| f.is_finite());
                }
            }
        }
        series.push(ChartSeries { name, values });
    }
    if series.is_empty() {
        return None;
    }
    let n = series.iter().map(|s| s.values.len()).max().unwrap_or(0);
    if categories.len() < n {
        categories.extend((categories.len() + 1..=n).map(|i| i.to_string()));
    }
    let title = kids(first(&root, "chart")?, "title").next().map(text_of).filter(|t| !t.is_empty());
    Some(ChartSpec { kind, categories, series, title, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_and_a_hierarchy_are_read_from_their_data() {
        let data = r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst>
<dgm:pt modelId="0" type="doc"><dgm:prSet loTypeId="urn:microsoft.com/office/officeart/2005/8/layout/hierarchy1"/></dgm:pt>
<dgm:pt modelId="1"><dgm:t><a:p><a:r><a:t>Lead</a:t></a:r></a:p></dgm:t></dgm:pt>
<dgm:pt modelId="2"><dgm:t><a:p><a:r><a:t>Design</a:t></a:r></a:p></dgm:t></dgm:pt>
<dgm:pt modelId="3"><dgm:t><a:p><a:r><a:t>Build</a:t></a:r></a:p></dgm:t></dgm:pt>
<dgm:pt modelId="9" type="parTrans"/></dgm:ptLst><dgm:cxnLst>
<dgm:cxn modelId="10" srcId="0" destId="1" srcOrd="0"/><dgm:cxn modelId="11" srcId="1" destId="3" srcOrd="1"/><dgm:cxn modelId="12" srcId="1" destId="2" srcOrd="0"/>
<dgm:cxn modelId="13" type="presOf" srcId="1" destId="9"/></dgm:cxnLst></dgm:dataModel>"#;
        let s = smart_art_outline(data.as_bytes()).unwrap();
        assert_eq!(s.layout, SmartArtLayout::Hierarchy);
        let items: Vec<(&str, u8)> = s.items.iter().map(|i| (i.text.as_str(), i.level)).collect();
        assert_eq!(items, [("Lead", 0), ("Design", 1), ("Build", 1)]);
        assert!(smart_art_outline(b"<junk/>").is_none());
    }

    #[test]
    fn a_chart_is_read_from_its_caches() {
        let chart = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart>
<c:title><c:tx><c:rich><a:p><a:r><a:t>Sales</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>
<c:ser><c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>2026</c:v></c:pt></c:strCache></c:strRef></c:tx>
<c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:strCache></c:strRef></c:cat>
<c:val><c:numRef><c:numCache><c:pt idx="0"><c:v>4.5</c:v></c:pt><c:pt idx="1"><c:v>7</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>
</c:barChart></c:plotArea></c:chart></c:chartSpace>"#;
        let c = chart_outline(chart.as_bytes()).unwrap();
        assert_eq!(c.kind, ChartType::Column);
        assert_eq!(c.title.as_deref(), Some("Sales"));
        assert_eq!(c.categories, ["Q1", "Q2"]);
        assert_eq!(c.series[0].name, "2026");
        assert_eq!(c.series[0].values, [Some(4.5), Some(7.0)]);
    }
}
