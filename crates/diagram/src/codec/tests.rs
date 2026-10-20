use super::*;

/// Every kind's template opens in its editor with nothing lost, is drawn, and is written the
/// same way twice.
#[test]
fn every_template_opens_in_its_editor_and_draws() {
    let mut failed = Vec::new();
    for kind in Kind::ALL {
        let src = kind.template();
        assert_eq!(Kind::of(src), Some(kind), "{src}");
        match open(src) {
            Opened::Visual(d) => {
                let out = write(&d);
                if let Err(e) = crate::draw(&out) {
                    failed.push(format!("{kind:?} doesn't draw: {e}\n{out}"));
                }
                match open(&out) {
                    Opened::Visual(again) => {
                        if write(&again) != out {
                            failed.push(format!("{kind:?} isn't stable:\n{out}\n---\n{}", write(&again)));
                        }
                    }
                    other => failed.push(format!("{kind:?} written doesn't reopen: {other:?}\n{out}")),
                }
            }
            Opened::Partial(d) => failed.push(format!("{kind:?} template loses something:\n{}", write(&d))),
            Opened::Text => failed.push(format!("{kind:?} template opens as text")),
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}

/// Diagrams written by hand, using most of each kind's syntax, open with nothing lost.
#[test]
fn hand_written_diagrams_open_with_nothing_lost() {
    let samples = [
        "flowchart TD\n    A[Start] --> B{Is it?}\n    B -->|Yes| C[[Sub]]\n    B -- No --> D[(DB)]\n    C -.-> E((Circle))\n    D ==> F>Flag]\n    E --- G[/Lean/]\n    F --> H[\\Alt\\]\n    G --> I[/Trap\\]\n    H --> J[\\Inv/]\n    I --> K(((Double)))",
        "sequenceDiagram\n    title Hi\n    autonumber\n    actor A as Alice\n    participant D@{ \"type\" : \"database\" }\n    participant B\n    A->>B: m1\n    B-->>A: m2\n    A-)B: async\n    A-xB: x\n    A->B: s\n    A--xB: dx\n    A<<->>B: bi\n    Note right of A: n1\n    Note over A,B: n2\n    activate B\n    deactivate B\n    loop Every\n        A->>B: l\n    end\n    alt ok\n        A->>B: a\n    else ko\n        A->>B: b\n    end\n    A-|\\B: half",
        "stateDiagram-v2\n    state \"Long label\" as S1\n    state C <<choice>>\n    state F <<fork>>\n    [*] --> S1\n    S1 --> C : go\n    C --> [*]\n    C --> F",
        "classDiagram\n    class Animal {\n        <<interface>>\n        +String name\n        -eat(food) bool\n    }\n    class Duck[\"Le canard\"]\n    Animal <|-- Duck : is\n    A \"1\" *-- \"many\" B\n    C o-- D\n    E --> F\n    G -- H\n    I .. J\n    K ..|> L\n    M ..> N\n    O <|.. P\n    Q --|> R",
        "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER }|..|{ ITEM : \"has many\"\n    p[Person] {\n        string name PK \"the name\"\n        int age FK,UK\n    }",
        "requirementDiagram\n    requirement r1 {\n        id: 1\n        text: the text\n        risk: high\n        verifymethod: test\n    }\n    functionalRequirement r2 {\n        id: 1.2\n    }\n    element e1 {\n        type: simulation\n        docref: ref\n    }\n    e1 - satisfies -> r1\n    r1 - contains -> r2",
        "C4Container\n    title Sys\n    Person(cust, \"Customer\", \"A customer\")\n    System_Boundary(b1, \"Bank\") {\n        Container(web, \"Web\", \"Rust\", \"UI\")\n        ContainerDb(db, \"DB\", \"SQL\")\n    }\n    System_Ext(x, \"X\")\n    Rel(cust, web, \"Uses\", \"HTTPS\")\n    BiRel(web, db, \"Reads\")\n    Rel_U(web, x, \"Calls\")",
        "architecture-beta\n    group api(cloud)[API]\n    service db(database)[Database] in api\n    service srv(server)[Server]\n    junction j1\n    db:L -- R:srv\n    srv:T --> B:j1",
        "block-beta\n    columns 3\n    a[\"A\"] b:2 c((\"C\"))\n    space\n    d>\"odd\"] e[/\"lean\"/]\n    a --> b\n    c -- \"lbl\" --> d",
        "gantt\n    title Plan\n    dateFormat YYYY-MM-DD\n    axisFormat %d/%m\n    excludes weekends\n    section W\n    Draft :done, a1, 2026-11-02, 10d\n    Review :active, crit, after a1, 5d\n    End :milestone, m1, 2026-11-20, 2026-11-21\n    Last :until m1",
        "journey\n    title Day\n    section Work\n      Make tea: 5: Me, Cat\n      Go: 1: Me",
        "timeline\n    title History\n    section Early\n        2002 : LinkedIn\n        2004 : Facebook : Google\n    section Late\n        2010 : Insta",
        "kanban\n  todo[To do]\n    t1[Write]@{ assigned: 'ann', priority: 'High', ticket: 'T-1' }\n    t2[Read]\n  done[Done]",
        "mindmap\n  root((Plume))\n    A\n      B[Sq]\n      C(Rnd)\n      D))Bang((\n      E)Cloud(\n      F{{Hex}}\n        ::icon(fa fa-book)",
        "gitGraph TB:\n    commit id: \"a\" tag: \"v1\" type: HIGHLIGHT\n    branch dev order: 2\n    checkout dev\n    commit\n    checkout main\n    merge dev id: \"m\" tag: \"t\" type: REVERSE\n    cherry-pick id: \"a\" parent: \"x\" tag: \"c\"",
        "pie showData title Pets\n    \"Dogs\" : 3\n    \"Cats\" : 2.5",
        "xychart-beta horizontal\n    title \"Sales\"\n    x-axis \"Quarter\" [Q1, \"Q 2\"]\n    y-axis \"Revenue\" 0 --> 100\n    bar \"2025\" [10, 20]\n    line [5, 15]",
        "radar-beta\n    title Skills\n    axis a[\"Alpha\"], b[\"Beta\"], c\n    curve x[\"Ann\"]{1, 2, 3}\n    curve y{3, 2, 1}\n    max 5\n    min 0\n    graticule polygon",
        "quadrantChart\n    title Reach\n    x-axis Low --> High\n    y-axis Bottom --> Top\n    quadrant-1 Expand\n    quadrant-2 Promote\n    quadrant-3 Re-eval\n    quadrant-4 Improve\n    Campaign A: [0.3, 0.6]\n    B: [0.45, 0.23]",
        "sankey-beta\nA,B,10\n\"C, d\",B,2.5",
        "treemap-beta\n\"Sec\"\n    \"Leaf\": 10\n    \"Sub\"\n        \"L2\": 5",
        "packet-beta\n    title TCP\n    0-15: \"Source\"\n    16-31: \"Dest\"\n    32: \"Flag\"",
        "---\ntitle: Kept\n---\nflowchart LR\n    accTitle: Steps\n    a --> b",
    ];
    let mut failed = Vec::new();
    for src in samples {
        match open(src) {
            Opened::Visual(d) => {
                let out = write(&d);
                if crate::draw(&out).is_err() {
                    failed.push(format!("doesn't draw:\n{out}"));
                }
            }
            Opened::Partial(d) => failed.push(format!("loses something:\n{src}\n--- written ---\n{}", write(&d))),
            Opened::Text => failed.push(format!("opens as text:\n{src}")),
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}

/// What an editor can't show makes the diagram partial, never silently lost.
#[test]
fn what_an_editor_cannot_show_is_reported() {
    let notes = "stateDiagram-v2\n    A --> B\n    note right of A : careful";
    assert!(matches!(open(notes), Opened::Partial(_)));
    assert!(matches!(open("flowchart LR\n    subgraph one\n    a --> b\n    end"), Opened::Text));
    assert!(matches!(open("not a diagram"), Opened::Text));
    assert!(matches!(open(""), Opened::Text));
}

/// Editing a model and writing it gives a diagram that draws.
#[test]
fn edited_models_are_written_and_draw() {
    let Some(mut d) = new(Kind::Class) else { panic!("class template") };
    if let Model::Graph(g) = &mut d.model {
        let a = g.add("class", "Chapter", "Chapter", 400.0, 100.0).unwrap();
        g.nodes[a].lines.push("+int number".into());
        g.connect(0, a, "--*");
    }
    let out = write(&d);
    assert!(out.contains("class Chapter {") && out.contains("Document \"1\" --o \"many\" Version"), "{out}");
    crate::draw(&out).unwrap();
    let Some(mut s) = new(Kind::Gantt) else { panic!("gantt template") };
    if let Model::Sheet(sh) = &mut s.model {
        let spec = Kind::Gantt.sheet().unwrap();
        sh.push(spec, 0, &["Release", "Announce", "", "after review", "2d", "", "true", ""]);
    }
    let out = write(&s);
    assert!(out.contains("Announce :crit, after review, 2d"), "{out}");
    crate::draw(&out).unwrap();
}

/// Junk never panics.
#[test]
fn hostile_sources_never_panic() {
    for kind in Kind::ALL {
        let t = kind.template();
        for cut in [1, t.len() / 3, t.len() / 2, t.len().saturating_sub(3)] {
            let src: String = t.chars().take(cut).collect();
            if let Opened::Visual(d) | Opened::Partial(d) = open(&src) {
                let _ = write(&d);
            }
        }
    }
}
