//! Document templates in pandoc's template language (so templates such as Eisvogel work as they
//! are), and the YAML their settings are written in.
//!
//! **Templates**: `$name$` and `$name.field$` insert a value, `$if(name)$ … $elseif(other)$ …
//! $else$ … $endif$` test one, `$for(name)$ … $sep$ … $endfor$` repeat for each item of a list
//! (`$name$` or `$it$` is the item), `$-- comment` runs to the end of the line and `$$` is a
//! dollar sign. A keyword alone on its line takes the whole line with it. The `pairs` pipe
//! (`$for(map/pairs)$`) turns a map into `key`/`value` items; other pipes are ignored.
//!
//! **YAML**: the subset documents and templates use: `key: value` (plain, quoted, `true`/`false`,
//! flow lists `[a, b]`), lists of `- item` lines, nested maps by indentation, `|` and `>` blocks,
//! `#` comments. Anything else is skipped, so a hand-edited file never stops an export.
//!
//! Both are lenient and bounded: nesting depth, sizes and output length are capped.

use std::collections::BTreeMap;

/// A template variable's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Str(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl Value {
    /// True for `$if$`: not false, not empty.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => l.iter().any(Value::truthy),
            Value::Map(m) => !m.is_empty(),
        }
    }

    fn render(&self, out: &mut String) {
        match self {
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) | Value::Map(_) => {}
            Value::Str(s) => out.push_str(s),
            Value::List(l) => l.iter().for_each(|v| v.render(out)),
        }
    }

    /// The value as text (lists joined with `, `).
    pub fn text(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::List(l) => l.iter().map(Value::text).collect::<Vec<_>>().join(", "),
            Value::Map(_) => String::new(),
        }
    }

    fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.get(key),
            _ => None,
        }
    }
}

/// Largest template or YAML text read (bytes).
pub const MAX_SOURCE: usize = 4 << 20;
/// Largest output written (bytes).
const MAX_OUTPUT: usize = 64 << 20;
/// Nesting limit (`$if$`/`$for$`, YAML indentation).
const MAX_DEPTH: usize = 64;
/// Most items one `$for$` repeats.
const MAX_ITEMS: usize = 10_000;

// =============================================================================================
// Templates

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Text(String),
    Var(Vec<String>),
    If(Vec<(Vec<String>, Vec<Node>)>, Vec<Node>),
    For { path: Vec<String>, pairs: bool, body: Vec<Node>, sep: Vec<Node> },
}

#[derive(Debug, PartialEq)]
enum Tok {
    Text(String),
    Var(String),
    If(String),
    ElseIf(String),
    Else,
    EndIf,
    For(String),
    Sep,
    EndFor,
}

/// A parsed template.
#[derive(Clone, Debug)]
pub struct Template {
    nodes: Vec<Node>,
}

/// Why a template couldn't be read.
#[derive(Clone, Debug, PartialEq)]
pub struct TemplateError(pub String);

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Template {
    pub fn parse(src: &str) -> Result<Template, TemplateError> {
        if src.len() > MAX_SOURCE {
            return Err(TemplateError("the template is too large".into()));
        }
        let toks = tokenize(src);
        let mut it = toks.into_iter().peekable();
        let nodes = parse_nodes(&mut it, 0)?;
        match it.next() {
            None => Ok(Template { nodes }),
            Some(t) => Err(TemplateError(format!("unexpected {}", tok_name(&t)))),
        }
    }

    /// The template filled with `vars` (a map).
    pub fn render(&self, vars: &Value) -> String {
        let mut out = String::new();
        let mut scope: Vec<(String, Value)> = Vec::new();
        render_nodes(&self.nodes, vars, &mut scope, &mut out, 0);
        out
    }
}

fn tok_name(t: &Tok) -> &'static str {
    match t {
        Tok::ElseIf(_) => "$elseif$",
        Tok::Else => "$else$",
        Tok::EndIf => "$endif$",
        Tok::Sep => "$sep$",
        Tok::EndFor => "$endfor$",
        _ => "text",
    }
}

/// Text before `at` on its line is blank.
fn line_blank_before(s: &str) -> bool {
    s.rsplit('\n').next().is_some_and(|l| l.chars().all(|c| c == ' ' || c == '\t'))
}

fn tokenize(src: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut text = String::new();
    let mut rest = src;
    while let Some(k) = rest.find('$') {
        text.push_str(rest.get(..k).unwrap_or(""));
        let after = rest.get(k + 1..).unwrap_or("");
        if let Some(r) = after.strip_prefix('$') {
            text.push('$');
            rest = r;
            continue;
        }
        if after.starts_with("--") {
            // A comment: to the end of the line, newline included; a whole-line comment leaves
            // no blank line.
            if line_blank_before(&text) {
                let keep = text.trim_end_matches([' ', '\t']).len();
                text.truncate(keep);
            }
            rest = match after.find('\n') {
                Some(n) => after.get(n + 1..).unwrap_or(""),
                None => "",
            };
            continue;
        }
        let Some(end) = after.find('$') else {
            text.push('$');
            rest = after;
            continue;
        };
        let inner = after.get(..end).unwrap_or("").trim();
        let tail = after.get(end + 1..).unwrap_or("");
        let tok = directive(inner);
        let Some(tok) = tok else {
            // Not a directive (a stray dollar in TeX): keep it as text.
            text.push('$');
            rest = after;
            continue;
        };
        let control = !matches!(tok, Tok::Var(_));
        rest = tail;
        if control && line_blank_before(&text) {
            let line_end = tail.find('\n');
            let alone = match line_end {
                Some(n) => tail.get(..n).is_some_and(|l| l.trim().is_empty()),
                None => tail.trim().is_empty(),
            };
            if alone {
                let keep = text.trim_end_matches([' ', '\t']).len();
                text.truncate(keep);
                rest = match line_end {
                    Some(n) => tail.get(n + 1..).unwrap_or(""),
                    None => "",
                };
            }
        }
        if !text.is_empty() {
            toks.push(Tok::Text(std::mem::take(&mut text)));
        }
        toks.push(tok);
    }
    text.push_str(rest);
    if !text.is_empty() {
        toks.push(Tok::Text(text));
    }
    toks
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

fn directive(inner: &str) -> Option<Tok> {
    let call = |kw: &str| {
        inner
            .strip_prefix(kw)
            .and_then(|r| r.strip_prefix('('))
            .and_then(|r| r.strip_suffix(')'))
            .map(|n| n.trim().to_string())
            .filter(|n| is_name(n))
    };
    if let Some(n) = call("if") {
        return Some(Tok::If(n));
    }
    if let Some(n) = call("elseif") {
        return Some(Tok::ElseIf(n));
    }
    if let Some(n) = call("for") {
        return Some(Tok::For(n));
    }
    match inner {
        "else" => Some(Tok::Else),
        "endif" => Some(Tok::EndIf),
        "sep" => Some(Tok::Sep),
        "endfor" => Some(Tok::EndFor),
        n if is_name(n) && n.chars().next().is_some_and(char::is_alphabetic) => Some(Tok::Var(n.to_string())),
        _ => None,
    }
}

/// `name.field/pipe` → (path, pairs pipe).
fn path_of(name: &str) -> (Vec<String>, bool) {
    let mut parts = name.split('/');
    let path = parts.next().unwrap_or("").split('.').map(str::to_string).collect();
    let pairs = parts.any(|p| p == "pairs");
    (path, pairs)
}

type Toks = std::iter::Peekable<std::vec::IntoIter<Tok>>;

fn parse_nodes(it: &mut Toks, depth: usize) -> Result<Vec<Node>, TemplateError> {
    if depth > MAX_DEPTH {
        return Err(TemplateError("the template nests too deeply".into()));
    }
    let mut nodes = Vec::new();
    while let Some(t) = it.peek() {
        if matches!(t, Tok::ElseIf(_) | Tok::Else | Tok::EndIf | Tok::Sep | Tok::EndFor) {
            break;
        }
        let Some(t) = it.next() else { break };
        match t {
            Tok::Text(s) => nodes.push(Node::Text(s)),
            Tok::Var(n) => nodes.push(Node::Var(path_of(&n).0)),
            Tok::If(n) => {
                let mut branches = vec![(path_of(&n).0, parse_nodes(it, depth + 1)?)];
                let mut otherwise = Vec::new();
                loop {
                    match it.next() {
                        Some(Tok::ElseIf(n)) => branches.push((path_of(&n).0, parse_nodes(it, depth + 1)?)),
                        Some(Tok::Else) => otherwise = parse_nodes(it, depth + 1)?,
                        Some(Tok::EndIf) => break,
                        _ => return Err(TemplateError(format!("$if({n})$ has no $endif$"))),
                    }
                }
                nodes.push(Node::If(branches, otherwise));
            }
            Tok::For(n) => {
                let (path, pairs) = path_of(&n);
                let body = parse_nodes(it, depth + 1)?;
                let mut sep = Vec::new();
                if it.peek() == Some(&Tok::Sep) {
                    it.next();
                    sep = parse_nodes(it, depth + 1)?;
                }
                if it.next() != Some(Tok::EndFor) {
                    return Err(TemplateError(format!("$for({n})$ has no $endfor$")));
                }
                nodes.push(Node::For { path, pairs, body, sep });
            }
            other => return Err(TemplateError(format!("unexpected {}", tok_name(&other)))),
        }
    }
    Ok(nodes)
}

fn lookup<'a>(path: &[String], vars: &'a Value, scope: &'a [(String, Value)]) -> Option<&'a Value> {
    let (first, rest) = path.split_first()?;
    let mut v = scope.iter().rev().find(|(n, _)| n == first || (first == "it" && n.is_empty())).map(|(_, v)| v);
    if v.is_none() && first == "it" {
        v = scope.last().map(|(_, v)| v);
    }
    let mut v = match v {
        Some(v) => v,
        None => vars.get(first)?,
    };
    for k in rest {
        v = v.get(k)?;
    }
    Some(v)
}

fn render_nodes(nodes: &[Node], vars: &Value, scope: &mut Vec<(String, Value)>, out: &mut String, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for n in nodes {
        if out.len() > MAX_OUTPUT {
            return;
        }
        match n {
            Node::Text(s) => out.push_str(s),
            Node::Var(p) => {
                if let Some(v) = lookup(p, vars, scope) {
                    v.render(out);
                }
            }
            Node::If(branches, otherwise) => {
                let hit = branches.iter().find(|(p, _)| lookup(p, vars, scope).is_some_and(Value::truthy));
                let body = hit.map(|(_, b)| b).unwrap_or(otherwise);
                render_nodes(body, vars, scope, out, depth + 1);
            }
            Node::For { path, pairs, body, sep } => {
                let items: Vec<Value> = match lookup(path, vars, scope) {
                    None => Vec::new(),
                    Some(Value::Map(m)) if *pairs => m
                        .iter()
                        .map(|(k, v)| Value::Map(BTreeMap::from([("key".to_string(), Value::Str(k.clone())), ("value".to_string(), v.clone())])))
                        .collect(),
                    Some(Value::List(l)) => l.iter().filter(|v| v.truthy()).cloned().collect(),
                    Some(v) if v.truthy() => vec![v.clone()],
                    Some(_) => Vec::new(),
                };
                let name = path.first().cloned().unwrap_or_default();
                for (i, item) in items.into_iter().take(MAX_ITEMS).enumerate() {
                    if i > 0 {
                        render_nodes(sep, vars, scope, out, depth + 1);
                    }
                    scope.push((name.clone(), item));
                    render_nodes(body, vars, scope, out, depth + 1);
                    scope.pop();
                }
            }
        }
    }
}

// =============================================================================================
// YAML

/// Read a YAML document (the subset described above) as a map. Never fails: what can't be read
/// is skipped.
pub fn parse_yaml(src: &str) -> BTreeMap<String, Value> {
    let src = src.get(..src.len().min(MAX_SOURCE)).unwrap_or("");
    let lines: Vec<&str> = src.lines().filter(|l| !l.trim_start().starts_with('#')).collect();
    let mut i = 0;
    match yaml_block(&lines, &mut i, 0, 0) {
        Value::Map(m) => m,
        _ => BTreeMap::new(),
    }
}

fn indent_of(l: &str) -> usize {
    l.len() - l.trim_start_matches(' ').len()
}

fn next_content(lines: &[&str], mut i: usize) -> Option<usize> {
    while let Some(l) = lines.get(i) {
        if !l.trim().is_empty() {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// The block (map or list) starting at line `*i`, indented `indent`.
fn yaml_block(lines: &[&str], i: &mut usize, indent: usize, depth: usize) -> Value {
    let Some(first) = next_content(lines, *i) else { return Value::Map(BTreeMap::new()) };
    let is_list = lines.get(first).is_some_and(|l| l.trim_start().starts_with("- ") || l.trim() == "-");
    if is_list {
        let mut items = Vec::new();
        while let Some(k) = next_content(lines, *i) {
            let Some(l) = lines.get(k) else { break };
            if indent_of(l) != indent || !(l.trim_start().starts_with("- ") || l.trim() == "-") {
                break;
            }
            *i = k + 1;
            let item = l.trim_start().trim_start_matches('-').trim();
            items.push(yaml_value(item, lines, i, indent, depth));
        }
        return Value::List(items);
    }
    let mut map = BTreeMap::new();
    while let Some(k) = next_content(lines, *i) {
        let Some(l) = lines.get(k) else { break };
        if indent_of(l) != indent {
            if indent_of(l) > indent {
                // Stray deeper line: skip it.
                *i = k + 1;
                continue;
            }
            break;
        }
        *i = k + 1;
        let t = l.trim();
        let Some((key, rest)) = split_key(t) else { continue };
        let v = yaml_value(rest, lines, i, indent, depth);
        map.insert(key, v);
    }
    Value::Map(map)
}

/// `key: rest` (a key never holds `: `, and may be quoted).
fn split_key(t: &str) -> Option<(String, &str)> {
    let (k, rest) = if let Some(r) = t.strip_suffix(':') { (r, "") } else { t.split_once(": ")? };
    let k = k.trim().trim_matches(['"', '\'']);
    if k.is_empty() {
        return None;
    }
    Some((k.to_string(), rest.trim()))
}

/// The value after `key:` or `- ` (`rest`), reading the lines below it when it continues there.
fn yaml_value(rest: &str, lines: &[&str], i: &mut usize, indent: usize, depth: usize) -> Value {
    let rest = strip_comment(rest);
    if rest == "|" || rest == ">" || rest.starts_with("|-") || rest.starts_with(">-") || rest.starts_with("|+") || rest.starts_with(">+") {
        let fold = rest.starts_with('>');
        let mut body: Vec<&str> = Vec::new();
        let mut block_indent = None;
        while let Some(l) = lines.get(*i) {
            if l.trim().is_empty() {
                body.push("");
                *i += 1;
                continue;
            }
            let n = indent_of(l);
            if n <= indent {
                break;
            }
            let bi = *block_indent.get_or_insert(n);
            body.push(l.get(bi.min(n)..).unwrap_or(""));
            *i += 1;
        }
        while body.last() == Some(&"") {
            body.pop();
        }
        let text = if fold { body.join(" ") } else { body.join("\n") };
        return Value::Str(text);
    }
    if !rest.is_empty() {
        // `- key: value` starts a map item in a list.
        if let Some((k, r)) = split_key(rest).filter(|_| !rest.starts_with(['"', '\'', '['])) {
            let mut m = BTreeMap::new();
            m.insert(k, scalar(r));
            if let Some(next) = next_content(lines, *i)
                && let Some(l) = lines.get(next)
                && indent_of(l) > indent
                && depth < MAX_DEPTH
                && let Value::Map(more) = yaml_block(lines, i, indent_of(l), depth + 1)
            {
                m.extend(more);
            }
            return Value::Map(m);
        }
        return scalar(rest);
    }
    // Nothing after the colon: a nested block below, if indented deeper (or a list at the same
    // indentation, as YAML allows under a map key).
    let Some(next) = next_content(lines, *i) else { return Value::Str(String::new()) };
    let Some(l) = lines.get(next) else { return Value::Str(String::new()) };
    let n = indent_of(l);
    let list_here = n == indent && l.trim_start().starts_with("- ");
    if (n > indent || list_here) && depth < MAX_DEPTH {
        return yaml_block(lines, i, n, depth + 1);
    }
    Value::Str(String::new())
}

fn strip_comment(s: &str) -> &str {
    if s.starts_with(['"', '\'']) {
        return s;
    }
    match s.find(" #") {
        Some(k) => s.get(..k).unwrap_or(s).trim_end(),
        None => s,
    }
}

/// A scalar or a flow list.
fn scalar(v: &str) -> Value {
    let v = strip_comment(v.trim());
    if let Some(inner) = v.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return Value::List(split_flow(inner).into_iter().map(|x| scalar(&x)).filter(|x| x.truthy()).collect());
    }
    if let Some(inner) = v.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(n) => out.push(n),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        return Value::Str(out);
    }
    if let Some(inner) = v.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return Value::Str(inner.replace("''", "'"));
    }
    match v {
        "true" | "True" | "TRUE" | "yes" => Value::Bool(true),
        "false" | "False" | "FALSE" | "no" => Value::Bool(false),
        "null" | "~" => Value::Str(String::new()),
        _ => Value::Str(v.to_string()),
    }
}

/// Items of a flow list, commas inside quotes kept.
fn split_flow(s: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match (quote, c) {
            (None, '"' | '\'') => {
                quote = Some(c);
                cur.push(c);
            }
            (Some(q), _) if c == q => {
                quote = None;
                cur.push(c);
            }
            (None, ',') => items.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        items.push(cur.trim().to_string());
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, Value)]) -> Value {
        Value::Map(pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
    }
    fn s(v: &str) -> Value {
        Value::Str(v.into())
    }

    #[test]
    fn variables_conditions_and_loops() {
        let t = Template::parse("A $title$.\n$if(toc)$\n\\tableofcontents\n$else$\nno toc\n$endif$\n$for(author)$$author$$sep$ \\and $endfor$\n$-- a comment\n$$5 $pdf.version$\n").unwrap();
        let v = map(&[
            ("title", s("Hi")),
            ("toc", Value::Bool(true)),
            ("author", Value::List(vec![s("A"), s("B")])),
            ("pdf", map(&[("version", s("1.7"))])),
        ]);
        assert_eq!(t.render(&v), "A Hi.\n\\tableofcontents\nA \\and B\n$5 1.7\n");
        let v = map(&[("title", s("Hi")), ("toc", Value::Bool(false))]);
        assert_eq!(t.render(&v), "A Hi.\nno toc\n\n$5 \n");
    }

    #[test]
    fn elseif_pairs_and_it() {
        let t = Template::parse("$if(a)$A$elseif(b)$B$else$C$endif$|$for(m/pairs)$$m.key$=$m.value$$sep$,$endfor$|$for(l)$<$it$>$endfor$").unwrap();
        let v = map(&[("b", s("x")), ("m", map(&[("k1", s("1")), ("k2", s("2"))])), ("l", Value::List(vec![s("x"), s("y")]))]);
        assert_eq!(t.render(&v), "B|k1=1,k2=2|<x><y>");
    }

    #[test]
    fn broken_templates_are_errors_not_panics() {
        assert!(Template::parse("$if(a)$ no end").is_err());
        assert!(Template::parse("$endfor$").is_err());
        assert!(Template::parse(&"$if(a)$".repeat(200)).is_err());
        // A lone dollar is text.
        assert_eq!(Template::parse("costs $5 and $x y$").unwrap().render(&map(&[])), "costs $5 and $x y$");
    }

    #[test]
    fn yaml_subset() {
        let y = parse_yaml(
            "# comment\ntitle: \"Rapport : 2026\"\nauthor: [Ada, \"B, C\"]\ntitlepage: true\nheader-includes:\n  - |\n    \\usepackage{a}\n    \\usepackage{b}\n  - \\usepackage{c}\npdf:\n  version: 1.7 # note\nlist:\n- x\n- y\nempty:\n",
        );
        assert_eq!(y.get("title"), Some(&s("Rapport : 2026")));
        assert_eq!(y.get("author"), Some(&Value::List(vec![s("Ada"), s("B, C")])));
        assert_eq!(y.get("titlepage"), Some(&Value::Bool(true)));
        assert_eq!(y.get("header-includes"), Some(&Value::List(vec![s("\\usepackage{a}\n\\usepackage{b}"), s("\\usepackage{c}")])));
        assert_eq!(y.get("pdf").and_then(|p| p.get("version")), Some(&s("1.7")));
        assert_eq!(y.get("list"), Some(&Value::List(vec![s("x"), s("y")])));
        assert_eq!(y.get("empty"), Some(&s("")));
        // Junk never panics.
        let _ = parse_yaml("::\n  - - -\n\t:\n\"\n[\n  | \n");
    }
}
