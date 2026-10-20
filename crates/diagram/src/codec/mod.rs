//! Plume: every kind of Mermaid diagram, edited without writing Mermaid. A diagram's source is
//! read with `mermaid-svg`'s parser into the model its editor shows (a canvas [`Graph`] or a
//! [`Sheet`] of tables), and the model is written back as Mermaid. What an editor can't show is
//! never dropped silently: [`open`] writes the model back and compares the two parsed diagrams;
//! when they differ, the diagram opens as [`Opened::Partial`] and the editor says so.
//!
//! The source's preamble (front matter, `%%{init}%%` directives) and accessibility lines
//! (`accTitle`, `accDescr`) are kept as written.

pub mod graphs;
pub mod sheets;

use serde::{Deserialize, Serialize};

use crate::graph::{Graph, GraphSpec};
use crate::sheet::{Sheet, SheetSpec};

/// The kinds of Mermaid diagrams.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Flowchart,
    Sequence,
    State,
    Class,
    Er,
    Requirement,
    C4,
    Architecture,
    Block,
    Gantt,
    Journey,
    Timeline,
    Kanban,
    Mindmap,
    GitGraph,
    Pie,
    XyChart,
    Radar,
    Quadrant,
    Sankey,
    Treemap,
    Packet,
    ZenUml,
}

impl Kind {
    pub const ALL: [Kind; 23] = [
        Kind::Flowchart,
        Kind::Sequence,
        Kind::State,
        Kind::Class,
        Kind::Er,
        Kind::Requirement,
        Kind::C4,
        Kind::Architecture,
        Kind::Block,
        Kind::Gantt,
        Kind::Journey,
        Kind::Timeline,
        Kind::Kanban,
        Kind::Mindmap,
        Kind::GitGraph,
        Kind::Pie,
        Kind::XyChart,
        Kind::Radar,
        Kind::Quadrant,
        Kind::Sankey,
        Kind::Treemap,
        Kind::Packet,
        Kind::ZenUml,
    ];

    /// The gallery's groups, in order.
    pub const CATEGORIES: [&'static str; 4] = ["Process", "Structure", "Planning", "Data"];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Flowchart => "Flowchart",
            Kind::Sequence => "Sequence Diagram",
            Kind::State => "State Diagram",
            Kind::Class => "Class Diagram",
            Kind::Er => "Entity Relationship",
            Kind::Requirement => "Requirement Diagram",
            Kind::C4 => "C4 Architecture",
            Kind::Architecture => "Cloud Architecture",
            Kind::Block => "Block Diagram",
            Kind::Gantt => "Gantt Chart",
            Kind::Journey => "User Journey",
            Kind::Timeline => "Timeline",
            Kind::Kanban => "Kanban Board",
            Kind::Mindmap => "Mind Map",
            Kind::GitGraph => "Git History",
            Kind::Pie => "Pie Chart",
            Kind::XyChart => "Bar and Line Chart",
            Kind::Radar => "Radar Chart",
            Kind::Quadrant => "Quadrant Chart",
            Kind::Sankey => "Sankey Flows",
            Kind::Treemap => "Treemap",
            Kind::Packet => "Packet Layout",
            Kind::ZenUml => "ZenUML Sequence",
        }
    }

    /// The gallery group it shows in.
    pub fn category(self) -> &'static str {
        match self {
            Kind::Flowchart | Kind::Sequence | Kind::State | Kind::Journey | Kind::GitGraph | Kind::ZenUml => "Process",
            Kind::Class | Kind::Er | Kind::Requirement | Kind::C4 | Kind::Architecture | Kind::Block | Kind::Packet => "Structure",
            Kind::Gantt | Kind::Timeline | Kind::Kanban | Kind::Mindmap => "Planning",
            Kind::Pie | Kind::XyChart | Kind::Radar | Kind::Quadrant | Kind::Sankey | Kind::Treemap => "Data",
        }
    }

    /// The kind a source's first statement names.
    pub fn of(body: &str) -> Option<Kind> {
        let head = body.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("%%"))?;
        let word = head.split(|c: char| c.is_whitespace() || c == ':' || c == ';').next()?;
        Some(match word {
            "flowchart" | "graph" | "flowchart-elk" => Kind::Flowchart,
            "sequenceDiagram" => Kind::Sequence,
            "stateDiagram" | "stateDiagram-v2" => Kind::State,
            "classDiagram" | "classDiagram-v2" => Kind::Class,
            "erDiagram" => Kind::Er,
            "requirementDiagram" => Kind::Requirement,
            "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment" => Kind::C4,
            "architecture-beta" | "architecture" => Kind::Architecture,
            "block-beta" | "block" => Kind::Block,
            "gantt" => Kind::Gantt,
            "journey" => Kind::Journey,
            "timeline" => Kind::Timeline,
            "kanban" => Kind::Kanban,
            "mindmap" => Kind::Mindmap,
            "gitGraph" => Kind::GitGraph,
            "pie" => Kind::Pie,
            "xychart-beta" | "xychart" => Kind::XyChart,
            "radar-beta" | "radar" => Kind::Radar,
            "quadrantChart" => Kind::Quadrant,
            "sankey-beta" | "sankey" => Kind::Sankey,
            "treemap-beta" | "treemap" => Kind::Treemap,
            "packet-beta" | "packet" => Kind::Packet,
            "zenuml" => Kind::ZenUml,
            _ => return None,
        })
    }

    /// Its canvas, when it is drawn on one.
    pub fn graph(self) -> Option<&'static GraphSpec> {
        Some(match self {
            Kind::Flowchart => &graphs::FLOWCHART,
            Kind::State => &graphs::STATE,
            Kind::Class => &graphs::CLASS,
            Kind::Er => &graphs::ER,
            Kind::Requirement => &graphs::REQUIREMENT,
            Kind::C4 => &graphs::C4,
            Kind::Architecture => &graphs::ARCHITECTURE,
            Kind::Block => &graphs::BLOCK,
            _ => return None,
        })
    }

    /// Its tables, when it is filled in as tables.
    pub fn sheet(self) -> Option<&'static SheetSpec> {
        Some(match self {
            Kind::Sequence | Kind::ZenUml => &sheets::SEQUENCE,
            Kind::Gantt => &sheets::GANTT,
            Kind::Journey => &sheets::JOURNEY,
            Kind::Timeline => &sheets::TIMELINE,
            Kind::Kanban => &sheets::KANBAN,
            Kind::Mindmap => &sheets::MINDMAP,
            Kind::GitGraph => &sheets::GITGRAPH,
            Kind::Pie => &sheets::PIE,
            Kind::XyChart => &sheets::XYCHART,
            Kind::Radar => &sheets::RADAR,
            Kind::Quadrant => &sheets::QUADRANT,
            Kind::Sankey => &sheets::SANKEY,
            Kind::Treemap => &sheets::TREEMAP,
            Kind::Packet => &sheets::PACKET,
            _ => return None,
        })
    }

    /// A new diagram of this kind starts from this.
    pub fn template(self) -> &'static str {
        match self {
            Kind::Flowchart => {
                "flowchart LR\n    n1([\"Draft\"])\n    n2{\"Reviewed?\"}\n    n3[\"Publish\"]\n    n1 --> n2\n    n2 -->|\"yes\"| n3\n    n2 -->|\"no\"| n1"
            }
            Kind::Sequence => {
                "sequenceDiagram\n    actor Author\n    participant Reviewer\n    Author->>Reviewer: Request review\n    Reviewer-->>Author: Comments\n    Author->>Reviewer: New version"
            }
            Kind::State => {
                "stateDiagram-v2\n    direction LR\n    Draft\n    Review\n    Published\n    [*] --> Draft\n    Draft --> Review : submit\n    Review --> Draft : changes\n    Review --> Published : approve\n    Published --> [*]"
            }
            Kind::Class => {
                "classDiagram\n    direction LR\n    class Document {\n        +String title\n        +save() bool\n    }\n    class Version {\n        +String message\n    }\n    Document \"1\" --o \"many\" Version : keeps"
            }
            Kind::Er => {
                "erDiagram\n    direction LR\n    AUTHOR {\n        string name PK\n    }\n    DOCUMENT {\n        string title\n        date published\n    }\n    AUTHOR ||--o{ DOCUMENT : \"writes\""
            }
            Kind::Requirement => {
                "requirementDiagram\n    requirement offline {\n        id: 1\n        text: Works without a network\n        risk: medium\n        verifymethod: test\n    }\n    element editor {\n        type: application\n    }\n    editor - satisfies -> offline"
            }
            Kind::C4 => {
                "C4Context\n    title Writing platform\n    Person(author, \"Author\", \"Writes documents\")\n    System(plume, \"Plume\", \"Markdown editor\")\n    System_Ext(forge, \"Forge\", \"Keeps the versions\")\n    Rel(author, plume, \"Writes with\")\n    Rel(plume, forge, \"Saves to\", \"HTTPS\")"
            }
            Kind::Architecture => {
                "architecture-beta\n    group cloud(cloud)[Cloud]\n    service web(internet)[Web] in cloud\n    service api(server)[API] in cloud\n    service db(database)[Database] in cloud\n    web:R --> L:api\n    api:R --> L:db"
            }
            Kind::Block => "block-beta\n    columns 3\n    a[\"Input\"] b(\"Process\") c[(\"Store\")]\n    a --> b\n    b --> c",
            Kind::Gantt => {
                "gantt\n    title Handbook\n    dateFormat YYYY-MM-DD\n    section Writing\n    Draft :done, draft, 2026-11-02, 10d\n    Review :active, review, after draft, 5d\n    section Release\n    Publish :milestone, 2026-11-20, 0d"
            }
            Kind::Journey => {
                "journey\n    title Writing a chapter\n    section Draft\n      Outline: 4: Author\n      Write: 3: Author\n    section Review\n      Comment: 4: Reviewer, Author"
            }
            Kind::Timeline => "timeline\n    title Releases\n    section 2026\n        Spring : First draft\n        Autumn : Review : Publication",
            Kind::Kanban => {
                "kanban\n  todo[To do]\n    t1[Outline the chapter]@{ assigned: 'Ann', priority: 'High' }\n  doing[In progress]\n    t2[Write the introduction]\n  done[Done]"
            }
            Kind::Mindmap => "mindmap\n  n1((Handbook))\n    Space\n      Rooms\n      Tools\n    People\n      Members\n      Visitors",
            Kind::GitGraph => {
                "gitGraph\n    commit id: \"draft\"\n    branch review\n    checkout review\n    commit id: \"edits\"\n    checkout main\n    merge review tag: \"v1\""
            }
            Kind::Pie => "pie title Time spent\n    \"Writing\" : 60\n    \"Review\" : 25\n    \"Layout\" : 15",
            Kind::XyChart => {
                "xychart-beta\n    title \"Pages written\"\n    x-axis [\"Jan\", \"Feb\", \"Mar\"]\n    bar \"Pages\" [12, 18, 25]\n    line \"Goal\" [15, 15, 20]"
            }
            Kind::Radar => {
                "radar-beta\n    title Skills\n    axis writing[\"Writing\"], review[\"Review\"], layout[\"Layout\"]\n    curve ann[\"Ann\"]{4, 3, 5}\n    curve bob[\"Bob\"]{3, 5, 2}\n    max 5"
            }
            Kind::Quadrant => {
                "quadrantChart\n    title Priorities\n    x-axis Low effort --> High effort\n    y-axis Low value --> High value\n    quadrant-1 Plan\n    quadrant-2 Do now\n    quadrant-3 Drop\n    quadrant-4 Delegate\n    Index: [0.2, 0.8]\n    Glossary: [0.7, 0.3]"
            }
            Kind::Sankey => "sankey-beta\n\nBudget,Writing,50\nBudget,Review,20\nBudget,Layout,30",
            Kind::Treemap => {
                "treemap-beta\n\"Book\"\n    \"Part 1\"\n        \"Chapter 1\": 12\n        \"Chapter 2\": 8\n    \"Part 2\"\n        \"Chapter 3\": 15"
            }
            Kind::Packet => "packet-beta\n    title Header\n    0-15: \"Source\"\n    16-31: \"Destination\"\n    32-63: \"Sequence number\"",
            Kind::ZenUml => {
                "zenuml\n    title Review\n    @Actor Author\n    Reviewer\n    Author->Reviewer: Request review\n    if(changes) {\n        Reviewer->Author: Comments\n    }"
            }
        }
    }
}

/// A diagram as its editor holds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "editor", rename_all = "camelCase")]
pub enum Model {
    Graph(Graph),
    Sheet(Sheet),
}

/// A diagram being edited: its kind, its model, and what is kept as written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagram {
    pub kind: Kind,
    pub model: Model,
    /// Front matter and `%%{init}%%` lines before the header.
    #[serde(default)]
    pub preamble: Vec<String>,
    /// `accTitle` and `accDescr` lines.
    #[serde(default)]
    pub acc: Vec<String>,
}

/// What [`open`] makes of a source.
#[derive(Clone, Debug, PartialEq)]
pub enum Opened {
    /// The editor shows all of it.
    Visual(Diagram),
    /// The editor shows it, but editing it there would lose what it can't show.
    Partial(Diagram),
    /// No editor reads it: edited as text.
    Text,
}

/// The source's preamble (front matter, `%%{init}%%` lines) and the rest.
fn split_preamble(src: &str) -> (Vec<String>, String) {
    let lines: Vec<&str> = src.lines().collect();
    let mut k = 0;
    while lines.get(k).is_some_and(|l| l.trim().is_empty()) {
        k += 1;
    }
    let mut pre = Vec::new();
    if lines.get(k).is_some_and(|l| l.trim() == "---")
        && let Some(end) = lines.iter().skip(k + 1).position(|l| l.trim() == "---")
    {
        pre.extend(lines.iter().skip(k).take(end + 2).map(|l| l.to_string()));
        k += end + 2;
    }
    while let Some(l) = lines.get(k) {
        let t = l.trim();
        if t.is_empty() || t.starts_with("%%{") {
            if !t.is_empty() {
                pre.push(l.to_string());
            }
            k += 1;
        } else {
            break;
        }
    }
    (pre, lines.iter().skip(k).copied().collect::<Vec<_>>().join("\n"))
}

/// The body's `accTitle`/`accDescr` lines, and the body without them.
fn split_acc(body: &str) -> (Vec<String>, String) {
    let (mut acc, mut rest) = (Vec::new(), Vec::new());
    let mut in_block = false;
    for l in body.lines() {
        let t = l.trim();
        if in_block {
            acc.push(t.to_string());
            in_block = !t.contains('}');
        } else if t.starts_with("accTitle") || t.starts_with("accDescr") {
            acc.push(t.to_string());
            in_block = t.contains('{') && !t.contains('}');
        } else {
            rest.push(l);
        }
    }
    (acc, rest.join("\n"))
}

/// Parse with `mermaid-svg`, a panic in it being a failure, never a crash.
fn parse(src: &str) -> Option<(mermaid_svg::ast::Diagram, mermaid_svg::ast::DiagramMeta)> {
    if src.len() > crate::MAX_SOURCE {
        return None;
    }
    std::panic::catch_unwind(|| mermaid_svg::parse_with_meta(src)).ok()?.ok()
}

/// Whether two sources say the same: their parsed diagrams are equal, or hold the same
/// statements in another order (shapes declared before their links, say).
pub fn same(a: &str, b: &str) -> bool {
    let (Some(x), Some(y)) = (parse(a), parse(b)) else { return false };
    let (dx, dy) = (format!("{x:#?}"), format!("{y:#?}"));
    if dx == dy {
        return true;
    }
    let sorted = |s: &str| {
        let mut v: Vec<String> = s.lines().map(|l| l.trim().to_string()).collect();
        v.sort();
        v
    };
    sorted(&dx) == sorted(&dy)
}

/// The model of `src`'s diagram for its editor.
fn read(kind: Kind, ast: &mermaid_svg::ast::Diagram, body: &str) -> Option<Model> {
    use mermaid_svg::ast::Diagram as D;
    Some(match (kind, ast) {
        (Kind::Flowchart, _) => Model::Graph(graphs::flowchart_read(body)?),
        (Kind::State, D::State(d)) => Model::Graph(graphs::state_read(d, body)),
        (Kind::Class, D::Class(d)) => Model::Graph(graphs::class_read(d, body)),
        (Kind::Er, D::Er(d)) => Model::Graph(graphs::er_read(d, body)),
        (Kind::Requirement, D::Requirement(d)) => Model::Graph(graphs::requirement_read(d, body)),
        (Kind::C4, D::C4(d)) => Model::Graph(graphs::c4_read(d, body)),
        (Kind::Architecture, D::Architecture(d)) => Model::Graph(graphs::architecture_read(d, body)),
        (Kind::Block, D::Block(d)) => Model::Graph(graphs::block_read(d)),
        (Kind::Sequence | Kind::ZenUml, D::Sequence(d)) => Model::Sheet(sheets::sequence_read(d)),
        (Kind::Gantt, D::Gantt(d)) => Model::Sheet(sheets::gantt_read(d)),
        (Kind::Journey, D::Journey(d)) => Model::Sheet(sheets::journey_read(d)),
        (Kind::Timeline, D::Timeline(d)) => Model::Sheet(sheets::timeline_read(d)),
        (Kind::Kanban, D::Kanban(d)) => Model::Sheet(sheets::kanban_read(d)),
        (Kind::Mindmap, D::Mindmap(d)) => Model::Sheet(sheets::mindmap_read(d)),
        (Kind::GitGraph, D::GitGraph(d)) => Model::Sheet(sheets::gitgraph_read(d)),
        (Kind::Pie, D::Pie(d)) => Model::Sheet(sheets::pie_read(d)),
        (Kind::XyChart, D::XyChart(d)) => Model::Sheet(sheets::xychart_read(d)),
        (Kind::Radar, D::Radar(d)) => Model::Sheet(sheets::radar_read(d)),
        (Kind::Quadrant, D::Quadrant(d)) => Model::Sheet(sheets::quadrant_read(d)),
        (Kind::Sankey, D::Sankey(d)) => Model::Sheet(sheets::sankey_read(d)),
        (Kind::Treemap, D::Treemap(d)) => Model::Sheet(sheets::treemap_read(d)),
        (Kind::Packet, D::Packet(d)) => Model::Sheet(sheets::packet_read(d)),
        _ => return None,
    })
}

/// Open `src` in the editor of its kind.
pub fn open(src: &str) -> Opened {
    let (preamble, body) = split_preamble(src);
    let (acc, body) = split_acc(&body);
    let Some(kind) = Kind::of(&body) else { return Opened::Text };
    let Some((ast, _)) = parse(src) else { return Opened::Text };
    let Some(mut model) = read(kind, &ast, &body) else { return Opened::Text };
    if let (Model::Sheet(s), Some(spec)) = (&mut model, kind.sheet()) {
        s.fit(spec);
    }
    let d = Diagram { kind, model, preamble, acc };
    if same(src, &write(&d)) { Opened::Visual(d) } else { Opened::Partial(d) }
}

/// A new diagram of `kind`, from its template.
pub fn new(kind: Kind) -> Option<Diagram> {
    match open(kind.template()) {
        Opened::Visual(d) | Opened::Partial(d) => Some(d),
        Opened::Text => None,
    }
}

/// The diagram's statements (its header first), without preamble.
fn body(d: &Diagram) -> String {
    match (&d.model, d.kind) {
        (Model::Graph(g), Kind::Flowchart) => graphs::flowchart_write(g),
        (Model::Graph(g), Kind::Requirement) => {
            [graphs::requirement_write(g)].into_iter().chain(graphs::requirement_positions(g)).collect::<Vec<_>>().join("\n")
        }
        (Model::Graph(g), Kind::Block) => graphs::block_write(g),
        (Model::Graph(g), kind) => {
            let main = match kind {
                Kind::State => graphs::state_write(g),
                Kind::Class => graphs::class_write(g),
                Kind::Er => graphs::er_write(g),
                Kind::C4 => graphs::c4_write(g),
                Kind::Architecture => graphs::architecture_write(g),
                _ => graphs::flowchart_write(g),
            };
            [main].into_iter().chain(g.positions()).collect::<Vec<_>>().join("\n")
        }
        (Model::Sheet(s), kind) => match kind {
            Kind::ZenUml => sheets::zenuml_write(s),
            Kind::Gantt => sheets::gantt_write(s),
            Kind::Journey => sheets::journey_write(s),
            Kind::Timeline => sheets::timeline_write(s),
            Kind::Kanban => sheets::kanban_write(s),
            Kind::Mindmap => sheets::mindmap_write(s),
            Kind::GitGraph => sheets::gitgraph_write(s),
            Kind::Pie => sheets::pie_write(s),
            Kind::XyChart => sheets::xychart_write(s),
            Kind::Radar => sheets::radar_write(s),
            Kind::Quadrant => sheets::quadrant_write(s),
            Kind::Sankey => sheets::sankey_write(s),
            Kind::Treemap => sheets::treemap_write(s),
            Kind::Packet => sheets::packet_write(s),
            _ => sheets::sequence_write(s),
        },
    }
}

/// The diagram as Mermaid: its preamble, its header, its accessibility lines, its statements.
pub fn write(d: &Diagram) -> String {
    let main = body(d);
    let mut out: Vec<String> = d.preamble.clone();
    let mut lines = main.lines();
    if let Some(head) = lines.next() {
        out.push(head.to_string());
    }
    out.extend(d.acc.iter().map(|a| format!("    {a}")));
    out.extend(lines.map(str::to_string));
    out.join("\n")
}

#[cfg(test)]
mod tests;
