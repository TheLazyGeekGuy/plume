//! Plume: Insert › Emoji's picker. Every emoji Unicode lists (the engine's `emoji` table), by
//! group, with a search in English and French, Recently Used, and a skin tone for those that
//! have one. Each emoji is drawn by the document renderer, in colour, as it will show in the
//! document: egui's own emoji font is monochrome and lacks most of them.

use egui::{Rect, Sense, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_engine::emoji::{self, Emoji};

use crate::WordApp;
use crate::theme::{Tokens, regular};

/// Columns of the grid.
const COLS: usize = 10;
/// A tile's side, points.
const TILE: f32 = 34.0;
/// The emoji's size in a tile, points.
const GLYPH: f32 = 20.0;
/// Height of the grid, points.
const GRID_H: f32 = 9.0 * TILE / 2.0 * 1.5;
/// Most emoji drawn (rendered for the first time) in one frame; the rest the next frames.
const DRAWS_PER_FRAME: u32 = 24;
/// The tab icons: Recently Used, then one per group.
const RECENT_ICON: &str = "🕘";
const GROUP_ICONS: [&str; 9] = ["😀", "👋", "🐻", "🍔", "✈️", "⚽", "💡", "❤️", "🏁"];
/// The skin tone button's hand, in each tone.
const TONE_HANDS: [&str; 6] = ["✋", "✋🏻", "✋🏼", "✋🏽", "✋🏾", "✋🏿"];

/// The picker's state while its menu is open.
#[derive(Clone, Default)]
struct State {
    query: String,
    /// 0 Recently Used, 1… the groups.
    tab: Option<usize>,
    focused: bool,
}

/// The group's name in the interface language.
pub fn group_name(g: usize) -> &'static str {
    emoji::GROUPS.get(g).map_or("", |n| tl!(n))
}

/// Insert › Emoji's menu.
pub fn menu(ui: &mut Ui, app: &mut WordApp) {
    let id = egui::Id::new("emoji_picker");
    let mut st = ui.data_mut(|d| d.get_temp::<State>(id)).unwrap_or_default();
    let width = COLS as f32 * TILE + 14.0;
    ui.set_width(width);
    let lang = crate::i18n::current().code().to_string();
    let tone = usize::from(app.session.prefs.emoji_tone.min(5));

    // Search and skin tone.
    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut st.query).hint_text(tl!("Search emoji")).desired_width(width - TILE - 12.0);
        let r = ui.add(edit);
        if !st.focused {
            r.request_focus();
            st.focused = true;
        }
        let hand = TONE_HANDS.get(tone).copied().unwrap_or("✋");
        if tile(ui, hand, false).on_hover_text(tl!("Skin tone")).clicked() {
            app.session.prefs.emoji_tone = ((tone + 1) % TONE_HANDS.len()) as u8;
        }
    });

    // The tabs.
    let recent: Vec<String> = app.session.prefs.recent_emoji.clone();
    let tab = st.tab.unwrap_or(if recent.is_empty() { 1 } else { 0 });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (k, icon) in std::iter::once(RECENT_ICON).chain(GROUP_ICONS).enumerate() {
            let name = if k == 0 { tl!("Recently Used") } else { group_name(k - 1) };
            if tile(ui, icon, st.query.trim().is_empty() && k == tab).on_hover_text(name).clicked() {
                st.tab = Some(k);
                st.query.clear();
            }
        }
    });
    ui.separator();

    // What the grid shows: the search's results, Recently Used, or a group.
    let query = st.query.trim().to_string();
    let list: Vec<String> = if !query.is_empty() {
        emoji::search(&query, 600).into_iter().map(|e| e.in_tone(tone).to_string()).collect()
    } else if tab == 0 {
        recent
    } else {
        emoji::group(tab - 1).map(|e| e.in_tone(tone).to_string()).collect()
    };
    let title = if !query.is_empty() {
        tl!("Search emoji")
    } else if tab == 0 {
        tl!("Recently Used")
    } else {
        group_name(tab - 1)
    };
    ui.label(egui::RichText::new(title).strong());
    let mut hovered: Option<String> = None;
    let mut chosen: Option<String> = None;
    let rows = list.len().div_ceil(COLS);
    egui::ScrollArea::vertical().id_salt(("emoji_grid", tab, query.clone())).max_height(GRID_H).auto_shrink([false, true]).show_rows(
        ui,
        TILE,
        rows,
        |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for e in list.iter().skip(row * COLS).take(COLS) {
                        let r = tile(ui, e, false);
                        if r.hovered() {
                            hovered = Some(e.clone());
                        }
                        if r.clicked() {
                            chosen = Some(e.clone());
                        }
                    }
                });
            }
        },
    );
    if list.is_empty() {
        ui.label(egui::RichText::new(if tab == 0 && query.is_empty() { tl!("No recent emoji yet") } else { tl!("No emoji found") }).weak());
    }
    // The emoji under the pointer, named.
    ui.separator();
    let name = hovered.as_deref().and_then(emoji::find).map(|e: &Emoji| e.name(&lang)).unwrap_or("");
    ui.label(egui::RichText::new(if name.is_empty() { " " } else { name }).font(regular(12.0)));

    if let Some(e) = chosen {
        let _ = app.run("insert.emoji", json!({"char": e}));
        st = State::default();
        ui.data_mut(|d| d.insert_temp(id, st));
        ui.close();
        return;
    }
    ui.data_mut(|d| d.insert_temp(id, st));
}

/// One emoji as a clickable tile, drawn in colour.
fn tile(ui: &mut Ui, text: &str, selected: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(TILE, TILE), Sense::click());
    if !ui.is_rect_visible(r) {
        return resp;
    }
    if selected {
        ui.painter().rect_filled(r.shrink(1.0), 4.0, t.checked);
    } else if resp.hovered() {
        ui.painter().rect_filled(r.shrink(1.0), 4.0, t.hover);
    }
    let fill = if selected {
        t.checked
    } else if resp.hovered() {
        t.hover
    } else {
        ui.visuals().window_fill()
    };
    match texture(ui, text, fill) {
        Some(h) => {
            let sz = h.size_vec2() / ui.ctx().pixels_per_point();
            ui.painter().image(
                h.id(),
                Rect::from_center_size(r.center(), sz),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        // Not drawn yet (a few per frame): a placeholder meanwhile (egui's font lacks most emoji).
        None => {
            ui.painter().circle_filled(r.center(), GLYPH * 0.35, t.hover);
        }
    }
    resp
}

/// The emoji's image on `fill`, drawn once and kept.
fn texture(ui: &Ui, text: &str, fill: egui::Color32) -> Option<egui::TextureHandle> {
    let ppp = ui.ctx().pixels_per_point();
    let key = egui::Id::new(("emoji_tex", text, fill, (ppp * 100.0) as u32));
    if let Some(h) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return Some(h);
    }
    let now = ui.ctx().cumulative_frame_nr();
    let n = ui.ctx().data_mut(|d| {
        let c = d.get_temp_mut_or_default::<(u64, u32)>(egui::Id::new("emoji_tex_budget"));
        if c.0 != now {
            *c = (now, 0);
        }
        c.1 += 1;
        c.1
    });
    if n > DRAWS_PER_FRAME {
        ui.ctx().request_repaint();
        return None;
    }
    let paper = wordcraft_doc::Rgb(fill.r(), fill.g(), fill.b());
    let img = crate::previews::emoji_image(text, GLYPH, ppp, paper)?;
    let h = ui.ctx().load_texture(format!("emoji:{text}"), img, egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|d| d.insert_temp(key, h.clone()));
    Some(h)
}

#[cfg(test)]
mod tests {
    /// Every group has its interface name in every language (the tabs' tooltips and titles).
    #[test]
    fn group_names_are_translated() {
        for l in crate::i18n::Lang::all().filter(|l| *l != crate::i18n::Lang::EN) {
            for g in
                wordcraft_engine::emoji::GROUPS.iter().chain(&["Recently Used", "Search emoji", "Skin tone", "No emoji found", "No recent emoji yet"])
            {
                assert!(crate::i18n::has(l, g), "{}: {g}", l.code());
            }
        }
    }
}
