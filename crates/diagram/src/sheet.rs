//! Plume: diagrams filled in as tables, as Office fills a chart's data sheet: sequence
//! diagrams (participants, then the steps), Gantt charts (tasks), journeys, timelines, Sankey
//! flows, quadrant points, packet fields, Git histories, Kanban boards, mind maps and treemaps
//! (outlines), pies, bar and line charts, radar charts (a grid of values). One model for all of
//! them ([`Sheet`]); each kind says what it holds ([`SheetSpec`]) and is written to and read from
//! Mermaid in [`crate::codec`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::form::FieldSpec;

/// Most rows in a table, and rows and columns in a grid.
pub const MAX_ROWS: usize = 1000;
pub const MAX_COLS: usize = 32;
/// Deepest outline level.
pub const MAX_LEVEL: usize = 12;

/// A table of a sheet.
#[derive(Clone, Copy, Debug)]
pub struct TableSpec {
    pub label: &'static str,
    pub columns: &'static [FieldSpec],
    /// An outline: the first column is the row's level (0, 1, 2…), shown as indentation.
    pub tree: bool,
    /// Which cells of a row don't apply (greyed out): `(row, column) → hidden`.
    pub hidden: Option<fn(&[String], usize) -> bool>,
}

impl TableSpec {
    pub const fn new(label: &'static str, columns: &'static [FieldSpec]) -> TableSpec {
        TableSpec { label, columns, tree: false, hidden: None }
    }
    pub const fn tree(mut self) -> TableSpec {
        self.tree = true;
        self
    }
    pub const fn hidden(mut self, f: fn(&[String], usize) -> bool) -> TableSpec {
        self.hidden = Some(f);
        self
    }
    /// The index of column `key`.
    pub fn col(&self, key: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.key == key)
    }
}

/// A grid of numbers: rows (categories, axes) by columns (series, curves).
#[derive(Clone, Copy, Debug)]
pub struct GridSpec {
    pub row_label: &'static str,
    pub col_label: &'static str,
    /// Each column's kind (a bar or a line…), when columns have one.
    pub col_kinds: Option<&'static [(&'static str, &'static str)]>,
}

/// What a sheet holds.
#[derive(Clone, Copy, Debug)]
pub struct SheetSpec {
    pub fields: &'static [FieldSpec],
    pub tables: &'static [TableSpec],
    pub grid: Option<GridSpec>,
}

/// A grid's values: columns (name, kind) and rows (label, one value per column).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Grid {
    pub cols: Vec<(String, String)>,
    pub rows: Vec<(String, Vec<String>)>,
    /// The Mermaid ids of the rows and columns read from a file (radar axes and curves), kept
    /// when written back; rows and columns added later get ids from their labels.
    pub row_ids: Vec<String>,
    pub col_ids: Vec<String>,
}

/// A diagram filled in as tables.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Sheet {
    pub fields: BTreeMap<String, String>,
    /// One per [`SheetSpec::tables`]: rows of cells, one per column.
    pub tables: Vec<Vec<Vec<String>>>,
    pub grid: Grid,
}

impl Sheet {
    /// An empty sheet shaped like `spec`.
    pub fn new(spec: &SheetSpec) -> Sheet {
        Sheet { tables: vec![Vec::new(); spec.tables.len()], ..Default::default() }
    }

    /// Table `t`'s rows.
    pub fn rows(&self, t: usize) -> &[Vec<String>] {
        self.tables.get(t).map_or(&[], Vec::as_slice)
    }

    /// Add a row to table `t` (cells as given, padded to the table's width).
    pub fn push(&mut self, spec: &SheetSpec, t: usize, cells: &[&str]) {
        let Some(ts) = spec.tables.get(t) else { return };
        while self.tables.len() <= t {
            self.tables.push(Vec::new());
        }
        if let Some(rows) = self.tables.get_mut(t)
            && rows.len() < MAX_ROWS
        {
            let mut row: Vec<String> = cells.iter().map(|c| crate::form::clip_text(c)).collect();
            row.resize(ts.columns.len(), String::new());
            rows.push(row);
        }
    }

    /// Make every table as wide as its spec says (after reading a sheet from elsewhere).
    pub fn fit(&mut self, spec: &SheetSpec) {
        self.tables.resize(spec.tables.len(), Vec::new());
        for (rows, ts) in self.tables.iter_mut().zip(spec.tables) {
            rows.truncate(MAX_ROWS);
            for r in rows.iter_mut() {
                r.resize(ts.columns.len(), String::new());
            }
        }
        self.grid.cols.truncate(MAX_COLS);
        self.grid.rows.truncate(MAX_ROWS);
        let n = self.grid.cols.len();
        for (_, v) in &mut self.grid.rows {
            v.resize(n, String::new());
        }
    }
}

/// Cell `i` of a row, `""` when missing.
pub fn cell(row: &[String], i: usize) -> &str {
    row.get(i).map_or("", String::as_str)
}

/// An outline row's level.
pub fn level(row: &[String]) -> usize {
    cell(row, 0).trim().parse::<usize>().unwrap_or(0).min(MAX_LEVEL)
}
