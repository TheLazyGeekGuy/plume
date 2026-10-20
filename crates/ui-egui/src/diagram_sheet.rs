//! Plume: the tables of the diagram editor, for the diagrams filled in rather than drawn
//! (sequences, Gantt charts, journeys, timelines, Kanban boards, mind maps, charts…): the
//! diagram's settings, then each table, one row per item, as Office's chart data sheet. Outlines
//! (mind maps, treemaps) indent their rows; charts with series are a grid of values.

use egui::{Ui, vec2};
use wordcraft_engine::diagram::form::{FieldType, get, set};
use wordcraft_engine::diagram::sheet::{MAX_COLS, MAX_LEVEL, MAX_ROWS, Sheet, SheetSpec, cell, level};

/// What a row's buttons asked for.
enum RowAction {
    Up(usize),
    Down(usize),
    Insert(usize),
    Remove(usize),
    Indent(usize, bool),
}

/// The sheet.
pub fn show(ui: &mut Ui, spec: &SheetSpec, s: &mut Sheet) {
    s.fit(spec);
    ui.set_width(640.0);
    egui::ScrollArea::both().id_salt("diagram_sheet").max_width(640.0).max_height(470.0).auto_shrink([false, false]).show(ui, |ui| {
        if !spec.fields.is_empty() {
            egui::Grid::new("diagram_sheet_fields").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                for f in spec.fields {
                    ui.label(tl!(f.label));
                    let mut v = get(&s.fields, f.key).to_string();
                    if crate::diagram_editor::field(ui, f, &mut v, &[], ("sheet", 0, f.key), 260.0) {
                        set(&mut s.fields, f.key, v);
                    }
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
        }
        for (t, ts) in spec.tables.iter().enumerate() {
            // A reference column offers the first column of the table it names (its ids), shown
            // with its second (names).
            let refs: Vec<Vec<(String, String)>> = spec
                .tables
                .iter()
                .enumerate()
                .map(|(k, _)| {
                    s.rows(k)
                        .iter()
                        .map(|r| {
                            let id = cell(r, 0).to_string();
                            let name = cell(r, 1);
                            (id.clone(), if name.is_empty() || name == id { id } else { format!("{name} ({id})") })
                        })
                        .collect()
                })
                .collect();
            ui.strong(tl!(ts.label));
            let visible: Vec<usize> = (0..ts.columns.len()).filter(|&c| !ts.columns.get(c).is_some_and(|f| f.key.starts_with('_'))).collect();
            let mut action: Option<RowAction> = None;
            let rows = s.tables.get_mut(t).map(std::mem::take).unwrap_or_default();
            let mut rows = rows;
            egui::Grid::new(("diagram_table", t)).num_columns(visible.len() + 1).spacing(vec2(6.0, 4.0)).striped(true).show(ui, |ui| {
                for &c in &visible {
                    if ts.tree && c == 0 {
                        continue;
                    }
                    ui.weak(ts.columns.get(c).map_or("", |f| tl!(f.label)));
                }
                ui.end_row();
                for (k, row) in rows.iter_mut().enumerate() {
                    for &c in &visible {
                        let Some(f) = ts.columns.get(c) else { continue };
                        if ts.tree && c == 0 {
                            continue;
                        }
                        if ts.hidden.is_some_and(|h| h(row, c)) {
                            ui.label("");
                            continue;
                        }
                        let width = match f.ty {
                            FieldType::Number => 70.0,
                            FieldType::Bool => 20.0,
                            FieldType::Choice(_) | FieldType::Ref(_) => 120.0,
                            FieldType::Text => {
                                if c == 1 || (ts.tree && c == 1) {
                                    170.0
                                } else {
                                    130.0
                                }
                            }
                        };
                        let mut v = cell(row, c).to_string();
                        let refs = match f.ty {
                            FieldType::Ref(other) => refs.get(other).map_or(&[][..], Vec::as_slice),
                            _ => &[],
                        };
                        let edited = if ts.tree && c == 1 {
                            // An outline: the text indented by its level.
                            let lvl = level(row);
                            ui.horizontal(|ui| {
                                ui.add_space(lvl as f32 * 18.0);
                                crate::diagram_editor::field(
                                    ui,
                                    f,
                                    &mut v,
                                    refs,
                                    ("cell", t * 100_000 + k, f.key),
                                    (width - lvl as f32 * 18.0).max(80.0),
                                )
                            })
                            .inner
                        } else {
                            crate::diagram_editor::field(ui, f, &mut v, refs, ("cell", t * 100_000 + k, f.key), width)
                        };
                        if edited && let Some(slot) = row.get_mut(c) {
                            *slot = v;
                        }
                    }
                    ui.horizontal(|ui| {
                        if ts.tree {
                            if ui.small_button("←").on_hover_text(tl!("Move Left")).clicked() {
                                action = Some(RowAction::Indent(k, false));
                            }
                            if ui.small_button("→").on_hover_text(tl!("Move Right")).clicked() {
                                action = Some(RowAction::Indent(k, true));
                            }
                        }
                        if ui.small_button("↑").on_hover_text(tl!("Move Up")).clicked() {
                            action = Some(RowAction::Up(k));
                        }
                        if ui.small_button("↓").on_hover_text(tl!("Move Down")).clicked() {
                            action = Some(RowAction::Down(k));
                        }
                        if ui.small_button("+").on_hover_text(tl!("Insert Row Below")).clicked() {
                            action = Some(RowAction::Insert(k));
                        }
                        if ui.small_button("×").on_hover_text(tl!("Remove")).clicked() {
                            action = Some(RowAction::Remove(k));
                        }
                    });
                    ui.end_row();
                }
            });
            let blank = |rows: &Vec<Vec<String>>, at: Option<usize>| -> Vec<String> {
                // A new row continues the one before: same section, level, participants.
                let mut r: Vec<String> = vec![String::new(); ts.columns.len()];
                if let Some(prev) = at.and_then(|k| rows.get(k)) {
                    for (c, f) in ts.columns.iter().enumerate() {
                        if matches!(f.key, "section" | "level" | "column" | "_column" | "step" | "action" | "from" | "to" | "arrow" | "type")
                            && let (Some(slot), Some(v)) = (r.get_mut(c), prev.get(c))
                        {
                            slot.clone_from(v);
                        }
                    }
                }
                r
            };
            match action {
                Some(RowAction::Up(k)) if k > 0 && k < rows.len() => rows.swap(k, k - 1),
                Some(RowAction::Down(k)) if k + 1 < rows.len() => rows.swap(k, k + 1),
                Some(RowAction::Insert(k)) if rows.len() < MAX_ROWS => {
                    let r = blank(&rows, Some(k));
                    rows.insert((k + 1).min(rows.len()), r);
                }
                Some(RowAction::Remove(k)) if k < rows.len() => {
                    rows.remove(k);
                }
                Some(RowAction::Indent(k, deeper)) => {
                    if let Some(r) = rows.get_mut(k)
                        && let Some(slot) = r.get_mut(0)
                    {
                        let lvl = slot.trim().parse::<usize>().unwrap_or(0);
                        *slot = if deeper { (lvl + 1).min(MAX_LEVEL) } else { lvl.saturating_sub(1) }.to_string();
                    }
                }
                _ => {}
            }
            if rows.len() < MAX_ROWS && ui.button(tl!("Add Row")).clicked() {
                let at = rows.len().checked_sub(1);
                let r = blank(&rows, at);
                rows.push(r);
            }
            if let Some(slot) = s.tables.get_mut(t) {
                *slot = rows;
            }
            ui.add_space(10.0);
        }
        if let Some(gs) = spec.grid {
            grid(ui, gs, s);
        }
    });
}

/// A grid of values: rows (categories) by columns (series), each column with its name and
/// kind.
fn grid(ui: &mut Ui, gs: wordcraft_engine::diagram::sheet::GridSpec, s: &mut Sheet) {
    let g = &mut s.grid;
    let mut remove_row = None;
    let mut remove_col = None;
    egui::Grid::new("diagram_grid").spacing(vec2(6.0, 4.0)).striped(true).show(ui, |ui| {
        ui.weak(tl!(gs.row_label));
        for (k, (name, kind)) in g.cols.iter_mut().enumerate() {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(90.0, 20.0), egui::TextEdit::singleline(name).char_limit(100).hint_text(tl!(gs.col_label)));
                    if ui.small_button("×").on_hover_text(tl!("Remove")).clicked() {
                        remove_col = Some(k);
                    }
                });
                if let Some(kinds) = gs.col_kinds {
                    let shown = kinds.iter().find(|(v, _)| v == kind).or(kinds.first()).map_or("", |(_, l)| tl!(l));
                    egui::ComboBox::from_id_salt(("grid_kind", k)).width(90.0).selected_text(shown).show_ui(ui, |ui| {
                        for (v, l) in kinds {
                            ui.selectable_value(kind, v.to_string(), tl!(l));
                        }
                    });
                }
            });
        }
        ui.end_row();
        for (k, (label, values)) in g.rows.iter_mut().enumerate() {
            ui.add_sized(vec2(140.0, 20.0), egui::TextEdit::singleline(label).char_limit(100));
            for v in values.iter_mut() {
                let bad = !v.trim().is_empty() && v.trim().replace(',', ".").parse::<f64>().is_err();
                let mut te = egui::TextEdit::singleline(v).char_limit(30);
                if bad {
                    te = te.text_color(ui.visuals().error_fg_color);
                }
                ui.add_sized(vec2(90.0, 20.0), te);
            }
            if ui.small_button("×").on_hover_text(tl!("Remove")).clicked() {
                remove_row = Some(k);
            }
            ui.end_row();
        }
    });
    if let Some(k) = remove_row.filter(|k| *k < g.rows.len()) {
        g.rows.remove(k);
        if k < g.row_ids.len() {
            g.row_ids.remove(k);
        }
    }
    if let Some(k) = remove_col.filter(|k| *k < g.cols.len()) {
        g.cols.remove(k);
        if k < g.col_ids.len() {
            g.col_ids.remove(k);
        }
        for (_, v) in &mut g.rows {
            if k < v.len() {
                v.remove(k);
            }
        }
    }
    ui.horizontal(|ui| {
        if g.rows.len() < MAX_ROWS && ui.button(tl!("Add Row")).clicked() {
            g.rows.push((String::new(), vec![String::new(); g.cols.len()]));
        }
        if g.cols.len() < MAX_COLS && ui.button(tl!("Add Column")).clicked() {
            let kind = gs.col_kinds.and_then(|k| k.first()).map_or(String::new(), |(v, _)| v.to_string());
            g.cols.push((format!("{} {}", tl!(gs.col_label), g.cols.len() + 1), kind));
            for (_, v) in &mut g.rows {
                v.push(String::new());
            }
        }
    });
}
