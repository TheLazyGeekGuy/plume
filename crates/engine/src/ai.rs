//! Plume: help from a language model, through any server that speaks the OpenAI chat API —
//! Ollama and LM Studio on this machine, or a service with a key (Albert, Mistral…). Off unless
//! the user turns it on; nothing leaves the machine unless they point it elsewhere.
//!
//! The engine prepares requests and checks answers; it never opens a connection itself. The
//! network goes through a [`Transport`] the app (or the CLI) provides, so the engine stays the
//! same on the web and in tests.

pub mod tables;

use serde_json::{Value, json};
use wordcraft_vcs::forge::{Method, Request, Transport};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Ollama's OpenAI-compatible address.
pub const OLLAMA: &str = "http://localhost:11434/v1";
/// LM Studio's local server.
pub const LM_STUDIO: &str = "http://localhost:1234/v1";

/// Largest answer accepted from a model, in bytes.
const MAX_ANSWER: usize = 4 << 20;

/// The user's choice of model (File › Options › Artificial intelligence), saved with the
/// preferences. Off by default.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    /// The server's address ([`OLLAMA`], [`LM_STUDIO`] or another).
    pub url: String,
    pub model: String,
    /// `""` (the model's default), `low`, `medium` or `high`.
    pub reasoning: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { enabled: false, url: OLLAMA.into(), model: String::new(), reasoning: String::new() }
    }
}

impl Settings {
    pub fn provider(&self) -> Provider {
        Provider {
            base_url: self.url.clone(),
            model: self.model.clone(),
            api_key: None,
            reasoning: Some(self.reasoning.clone()).filter(|r| !r.is_empty()),
        }
    }
}

/// Where to send requests and with which model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Provider {
    /// The API's base address (`…/v1`); `/v1` is added when missing.
    pub base_url: String,
    pub model: String,
    /// Bearer token for services that need one (never for Ollama or LM Studio).
    pub api_key: Option<String>,
    /// `low`, `medium` or `high` for reasoning models that take it (gpt-oss); `None` = the
    /// model's default.
    pub reasoning: Option<String>,
}

/// `base` + `path`, with `/v1` in between when `base` doesn't end with it.
pub fn endpoint(base: &str, path: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let base = if base.ends_with("/v1") { base.to_string() } else { format!("{base}/v1") };
    format!("{base}/{}", path.trim_start_matches('/'))
}

fn headers(p: &Provider) -> Vec<(String, String)> {
    let mut h = vec![("Content-Type".to_string(), "application/json".to_string()), ("Accept".to_string(), "application/json".to_string())];
    if let Some(k) = p.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        h.push(("Authorization".to_string(), format!("Bearer {k}")));
    }
    h
}

/// What went wrong, in words for the user.
fn http_error(p: &Provider, status: u16, body: &[u8]) -> String {
    let said = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().or_else(|| v["error"].as_str()).map(str::to_string))
        .unwrap_or_else(|| String::from_utf8_lossy(body.get(..body.len().min(300)).unwrap_or_default()).into_owned());
    match status {
        401 | 403 => format!("the server refused the key ({status}): {said}"),
        404 => format!("model `{}` not found on the server: {said}", p.model),
        _ => format!("the server answered {status}: {said}"),
    }
}

fn send(t: &dyn Transport, p: &Provider, req: &Request) -> Result<Vec<u8>, String> {
    let resp = t.send(req).map_err(|e| format!("can't reach {} ({e}); is Ollama or LM Studio running?", p.base_url))?;
    if !(200..300).contains(&resp.status) {
        return Err(http_error(p, resp.status, &resp.body));
    }
    if resp.body.len() > MAX_ANSWER {
        return Err("the answer is too large".into());
    }
    Ok(resp.body)
}

/// The models the server offers (Ollama: the ones pulled; LM Studio: the ones downloaded).
pub fn list_models(t: &dyn Transport, p: &Provider) -> Result<Vec<String>, String> {
    let req = Request { method: Method::Get, url: endpoint(&p.base_url, "models"), headers: headers(p), body: None };
    let body = send(t, p, &req)?;
    let v: Value = serde_json::from_slice(&body).map_err(|e| format!("not a model list: {e}"))?;
    let mut names: Vec<String> = v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str().map(str::to_string)).collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// The chat request asking for an answer shaped by `schema` (JSON Schema).
pub fn chat_request(p: &Provider, system: &str, user: &str, schema: &Value) -> Request {
    let mut body = json!({
        "model": p.model,
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
        "temperature": 0,
        "stream": false,
        "response_format": {"type": "json_schema", "json_schema": {"name": "plume_answer", "schema": schema}},
    });
    if let Some(r) = p.reasoning.as_deref().filter(|r| matches!(*r, "low" | "medium" | "high")) {
        body["reasoning_effort"] = json!(r);
    }
    Request { method: Method::Post, url: endpoint(&p.base_url, "chat/completions"), headers: headers(p), body: Some(body.to_string()) }
}

/// The JSON object in a model's reply: reasoning (`<think>…</think>`) and code fences around it
/// are dropped.
pub fn reply_json(content: &str) -> Result<Value, String> {
    let mut text = content;
    if let Some(end) = text.rfind("</think>") {
        text = text.get(end + "</think>".len()..).unwrap_or_default();
    }
    let start = text.find('{').ok_or("the model gave no JSON")?;
    let end = text.rfind('}').ok_or("the model gave no JSON")?;
    let json = text.get(start..=end).filter(|_| start < end).ok_or("the model gave no JSON")?;
    serde_json::from_str(json).map_err(|e| format!("the model's JSON doesn't parse: {e}"))
}

/// Ask the model and return its JSON answer.
pub fn chat_json(t: &dyn Transport, p: &Provider, system: &str, user: &str, schema: &Value) -> Result<Value, String> {
    if p.model.trim().is_empty() {
        return Err("no model chosen".into());
    }
    let body = send(t, p, &chat_request(p, system, user, schema))?;
    let v: Value = serde_json::from_slice(&body).map_err(|e| format!("not a chat answer: {e}"))?;
    let content = v["choices"][0]["message"]["content"].as_str().ok_or("the answer has no message")?;
    reply_json(content)
}

/// The commands. They never reach the network: the caller (the app, the CLI, an agent over MCP)
/// sends `ai.tableRequest`'s request to the model of its choice and hands the answers to
/// `ai.restructureTables`.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("ai.tables", "Tables for AI", "Review › AI", |s, _v| {
            let tables: Vec<Value> = tables::candidates(&s.doc)
                .into_iter()
                .map(|c| json!({"index": c.index, "rows": c.rows, "cols": c.cols, "depth": c.depth, "merged": c.merged, "words": c.words, "blocked": c.blocked}))
                .collect();
            Ok(json!({"tables": tables}))
        })
        .params("{} → tables: [{index (body block), rows, cols, depth (2 = tables in its cells), merged, words, blocked?: why it stays as is}]: the tables Markdown can't hold as they are")
        .pure(),
        CommandSpec::new("ai.tableRequest", "AI Request for a Table", "Review › AI", |s, v| {
            let index = p::u64(v, "index").ok_or_else(|| CmdError::Params("`index` (number) is required".into()))?;
            let (system, user, schema) = tables::request(&s.doc, usize::try_from(index).unwrap_or(usize::MAX)).map_err(CmdError::Failed)?;
            Ok(json!({"system": system, "user": user, "schema": schema}))
        })
        .params("{index} → {system, user, schema}: the chat request asking a model to restructure that table (send it with response_format json_schema)")
        .pure(),
        CommandSpec::new("ai.restructureTables", "Restructure Tables", "Review › AI", restructure)
            .params("{proposals: [{index, answer: the model's JSON}], force?: bool} → results: [{index, applied, kind, check, missing, invented, reason?}]: each table becomes the answer's headings, paragraphs, lists and tables when every word is kept and none is added (force: apply anyway); one undo step"),
    ]
}

fn restructure(s: &mut Session, v: &Value) -> CmdResult {
    let proposals = v["proposals"].as_array().ok_or_else(|| CmdError::Params("`proposals` (array) is required".into()))?;
    let force = p::bool(v, "force").unwrap_or(false);
    let mut results = Vec::new();
    let mut ready: Vec<(usize, Vec<tables::Out>)> = Vec::new();
    for prop in proposals.iter().take(10_000) {
        let index = p::u64(prop, "index").and_then(|i| usize::try_from(i).ok()).unwrap_or(usize::MAX);
        let answer = &prop["answer"];
        let kind = answer["kind"].as_str().unwrap_or("");
        let table = s.doc.body.get(index).and_then(|b| b.as_table());
        let outcome = match (table, tables::parse(answer)) {
            (None, _) => Err(format!("no table at block {index}")),
            (_, Err(e)) => Err(e),
            (Some(_), Ok(_)) if ready.iter().any(|(i, _)| *i == index) => Err("this table has two proposals".into()),
            (Some(t), Ok(blocks)) => Ok((tables::check(t, &blocks), blocks)),
        };
        match outcome {
            Err(reason) => results.push(json!({"index": index, "applied": false, "kind": kind, "reason": reason})),
            Ok((c, blocks)) => {
                let applied = c.passed() || force;
                results.push(
                    json!({"index": index, "applied": applied, "kind": kind, "check": c.summary(), "missing": c.missing, "invented": c.invented}),
                );
                if applied {
                    ready.push((index, blocks));
                }
            }
        }
    }
    // From the end, so earlier indices stay right.
    ready.sort_by_key(|r| std::cmp::Reverse(r.0));
    for (index, blocks) in ready {
        let frag = crate::cmd::paste::adopt(s, wordcraft_formats::html::import(&tables::to_html(&blocks)));
        if frag.blocks.is_empty() || index >= s.doc.body.len() {
            continue;
        }
        s.doc.body.splice(index..=index, frag.blocks.into_iter().map(std::sync::Arc::new));
    }
    s.clamp_selection();
    Ok(json!({"results": results}))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use wordcraft_vcs::forge::Response;

    /// Answers every request with `reply`, keeping what it was sent.
    pub(crate) struct Fake {
        pub reply: Box<dyn Fn(&Request) -> Response>,
        pub sent: RefCell<Vec<Request>>,
    }

    impl Transport for Fake {
        fn send(&self, req: &Request) -> Result<Response, String> {
            self.sent.borrow_mut().push(req.clone());
            Ok((self.reply)(req))
        }
    }

    fn ollama() -> Provider {
        Provider { base_url: "http://localhost:11434".into(), model: "gpt-oss:20b".into(), api_key: None, reasoning: Some("low".into()) }
    }

    /// A paragraph, then a form table with a table in a cell.
    fn session_with_form() -> Session {
        use std::sync::Arc;
        use wordcraft_doc::{Block, Table, table::Cell};
        let mut outer = Table::new(2, 2, 400.0);
        outer.rows[0].cells[0] = Cell::with_text("Name:");
        outer.rows[0].cells[1] = Cell::with_text("Jane Doe");
        let mut inner = Table::new(1, 2, 200.0);
        inner.rows[0].cells[0] = Cell::with_text("Lyon");
        inner.rows[0].cells[1] = Cell::with_text("France");
        outer.rows[1].cells[0] = Cell::with_text("City:");
        outer.rows[1].cells[1].blocks.push(Arc::new(Block::Table(inner)));
        let mut d = wordcraft_doc::Document::from_text("Before the form");
        d.body.push(Arc::new(Block::Table(outer)));
        Session::new(d)
    }

    #[test]
    fn restructures_a_table_when_every_word_is_kept() {
        let mut s = session_with_form();
        let found = s.run("ai.tables", &json!({})).unwrap();
        assert_eq!(found["tables"][0]["index"], 1);
        assert_eq!(found["tables"][0]["depth"], 2);
        let req = s.run("ai.tableRequest", &json!({"index": 1})).unwrap();
        assert!(req["user"].as_str().unwrap().contains("France"));
        // A lossy answer is refused and changes nothing.
        let lossy = json!({"proposals": [{"index": 1, "answer": {"kind": "layout", "blocks": [{"type": "paragraph", "text": "Name: Jane Doe"}]}}]});
        let r = s.run("ai.restructureTables", &lossy).unwrap();
        assert_eq!(r["results"][0]["applied"], false);
        assert_eq!(r["results"][0]["missing"], json!(["city", "france", "lyon"]));
        assert!(s.doc.body[1].as_table().is_some());
        let good = json!({"proposals": [{"index": 1, "answer": {"kind": "layout", "blocks": [
            {"type": "list", "items": ["Name: Jane Doe", "City: Lyon, France"]}
        ]}}]});
        let r = s.run("ai.restructureTables", &good).unwrap();
        assert_eq!(r["results"][0]["applied"], true, "{r}");
        assert!(s.doc.body.iter().all(|b| b.as_table().is_none()), "the table became a list");
        let text = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
        assert!(text.contains("Before the form") && text.contains("City: Lyon, France"), "{text}");
        s.run("edit.undo", &json!({})).unwrap();
        assert!(s.doc.body[1].as_table().is_some(), "one undo brings the table back");
        assert!(s.run("ai.restructureTables", &json!({"proposals": [{"index": 0, "answer": {}}]})).unwrap()["results"][0]["applied"] == false);
    }

    #[test]
    fn endpoints_add_v1_once() {
        assert_eq!(endpoint("http://localhost:11434", "models"), "http://localhost:11434/v1/models");
        assert_eq!(endpoint("http://localhost:1234/v1/", "/chat/completions"), "http://localhost:1234/v1/chat/completions");
    }

    #[test]
    fn lists_models_and_sends_the_schema() {
        let fake = Fake {
            reply: Box::new(|req| {
                let body = if req.url.ends_with("/models") {
                    json!({"data": [{"id": "qwen3:4b"}, {"id": "gpt-oss:20b"}]})
                } else {
                    json!({"choices": [{"message": {"content": "<think>hmm {not json}</think>```json\n{\"ok\": true}\n```"}}]})
                };
                Response { status: 200, body: body.to_string().into_bytes() }
            }),
            sent: RefCell::default(),
        };
        assert_eq!(list_models(&fake, &ollama()).unwrap(), ["gpt-oss:20b", "qwen3:4b"]);
        let answer = chat_json(&fake, &ollama(), "sys", "user", &json!({"type": "object"})).unwrap();
        assert_eq!(answer, json!({"ok": true}));
        let sent = fake.sent.borrow();
        let body: Value = serde_json::from_str(sent[1].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["reasoning_effort"], "low");
        assert!(sent[1].headers.iter().all(|(k, _)| k != "Authorization"), "no key for a local server");
    }

    #[test]
    fn errors_say_what_happened() {
        let fake = Fake {
            reply: Box::new(|_| Response { status: 404, body: br#"{"error": {"message": "model 'x' not found"}}"#.to_vec() }),
            sent: RefCell::default(),
        };
        let e = chat_json(&fake, &ollama(), "s", "u", &json!({})).unwrap_err();
        assert!(e.contains("gpt-oss:20b") && e.contains("not found"), "{e}");
        assert!(reply_json("no json here").is_err());
        assert!(reply_json("}{").is_err());
    }
}
