//! Plume: the fields the diagram editors show, described once: a diagram's own settings (its
//! title, its direction…), the properties of a shape or a connector on a canvas, the columns of
//! a table. The editors draw a field from its [`FieldSpec`]; values are kept as text.

use std::collections::BTreeMap;

/// What a field holds and how it is edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldType {
    Text,
    /// A number (kept as typed; checked when written).
    Number,
    /// `true` or `false`.
    Bool,
    /// One of these values (`value`, `label`).
    Choice(&'static [(&'static str, &'static str)]),
    /// The id of a row of another table of the same sheet (its first column), or of a shape
    /// of the same canvas: the editor offers them.
    Ref(usize),
}

/// One field: its key, its label in the editor, its type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub ty: FieldType,
}

pub const fn text(key: &'static str, label: &'static str) -> FieldSpec {
    FieldSpec { key, label, ty: FieldType::Text }
}
pub const fn number(key: &'static str, label: &'static str) -> FieldSpec {
    FieldSpec { key, label, ty: FieldType::Number }
}
pub const fn flag(key: &'static str, label: &'static str) -> FieldSpec {
    FieldSpec { key, label, ty: FieldType::Bool }
}
pub const fn choice(key: &'static str, label: &'static str, values: &'static [(&'static str, &'static str)]) -> FieldSpec {
    FieldSpec { key, label, ty: FieldType::Choice(values) }
}
pub const fn reference(key: &'static str, label: &'static str, table: usize) -> FieldSpec {
    FieldSpec { key, label, ty: FieldType::Ref(table) }
}

/// Longest value kept.
pub const MAX_VALUE: usize = 500;

/// A field's value, `""` when unset.
pub fn get<'a>(m: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    m.get(key).map_or("", String::as_str)
}

/// Set a field, removing it when empty.
pub fn set(m: &mut BTreeMap<String, String>, key: &str, value: impl Into<String>) {
    let v: String = value.into();
    if v.is_empty() {
        m.remove(key);
    } else {
        m.insert(key.to_string(), v.chars().take(MAX_VALUE).collect());
    }
}

/// A `true` flag.
pub fn is(m: &BTreeMap<String, String>, key: &str) -> bool {
    get(m, key) == "true"
}

/// A number written without a needless `.0`.
pub fn num(v: f64) -> String {
    if !v.is_finite() {
        "0".into()
    } else if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}

/// `s` on one line, at most [`MAX_VALUE`] characters.
pub fn one_line(s: &str) -> String {
    s.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ").chars().take(MAX_VALUE).collect()
}

/// `s` as a Mermaid double-quoted string (`"` written `#quot;`).
pub fn quoted(s: &str) -> String {
    format!("\"{}\"", one_line(s).replace('"', "#quot;"))
}

/// An identifier made from `s`: letters, digits and `_`; `fallback` when nothing is left.
pub fn ident(s: &str, fallback: &str) -> String {
    let id: String = s.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    let id = id.trim_matches('_').chars().take(60).collect::<String>();
    if id.is_empty() || id.starts_with(|c: char| c.is_ascii_digit()) { format!("{fallback}{id}") } else { id }
}

/// `base`, or `base_2`, `base_3`… the first not in `taken`.
pub fn unique(base: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..10_000).map(|k| format!("{base}_{k}")).find(|c| !taken(c)).unwrap_or_else(|| format!("{base}_x"))
}

/// A shape's or a cell's text: on one line, at most [`MAX_VALUE`] characters.
pub fn clip_text(s: &str) -> String {
    one_line(s)
}

/// Largest canvas coordinate.
const MAX_COORD: f32 = 20_000.0;

/// A canvas coordinate: finite and bounded.
pub fn finite(v: f32) -> f32 {
    if v.is_finite() { v.clamp(-MAX_COORD, MAX_COORD) } else { 0.0 }
}
