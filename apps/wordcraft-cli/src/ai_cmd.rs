//! `ai-tables` and `ai-bench` (Plume): restructure a Word document's nested and merged tables with
//! a language model (Ollama, LM Studio or any OpenAI-compatible server), and compare models on a
//! folder of documents.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wordcraft_engine::Session;
use wordcraft_engine::ai::{self, Provider};
use wordcraft_vcs::forge::{Request, Response, Transport};

/// Largest answer read from a model server.
const MAX_ANSWER: u64 = 8 << 20;

/// HTTP(S) through `ureq`. Local models can think for minutes on a large table: long timeout.
pub struct Https {
    agent: ureq::Agent,
}

impl Https {
    pub fn new() -> Https {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(900)))
            .user_agent(format!("{}/{}", wordcraft_brand::APP_NAME, env!("CARGO_PKG_VERSION")))
            .build();
        Https { agent: config.into() }
    }
}

impl Transport for Https {
    fn send(&self, req: &Request) -> Result<Response, String> {
        let mut b = ureq::http::Request::builder().method(req.method.as_str()).uri(&req.url);
        for (k, v) in &req.headers {
            b = b.header(k.as_str(), v.as_str());
        }
        let resp = match &req.body {
            Some(body) => self.agent.run(b.body(body.clone()).map_err(|e| e.to_string())?),
            None => self.agent.run(b.body(()).map_err(|e| e.to_string())?),
        };
        let mut resp = resp.map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let mut body = Vec::new();
        resp.body_mut().as_reader().take(MAX_ANSWER).read_to_end(&mut body).map_err(|e| e.to_string())?;
        Ok(Response { status, body })
    }
}

/// `--url`, `--key-env`, `--reasoning`: the server; the model is added by the caller.
pub fn provider(value: impl Fn(&str) -> Option<String>) -> Result<Provider, String> {
    let base_url = match value("--url").as_deref() {
        None | Some("ollama") => ai::OLLAMA.to_string(),
        Some("lmstudio") => ai::LM_STUDIO.to_string(),
        Some(u) => u.to_string(),
    };
    // The key is read from the environment, never from the command line (shell history).
    let api_key = match value("--key-env") {
        Some(var) => Some(std::env::var(&var).map_err(|_| format!("environment variable {var} is not set"))?),
        None => None,
    };
    let reasoning = value("--reasoning");
    if let Some(r) = &reasoning
        && !matches!(r.as_str(), "low" | "medium" | "high")
    {
        return Err("--reasoning is low, medium or high".into());
    }
    Ok(Provider { base_url, model: String::new(), api_key, reasoning })
}

/// One table's outcome.
pub struct TableRun {
    pub index: usize,
    pub words: usize,
    pub seconds: f64,
    /// `applied`, `rejected` (the check failed), `error` (no usable answer) or `skipped`.
    pub status: &'static str,
    pub kind: String,
    pub detail: String,
}

/// Ask the model about each table of `s`, then apply the answers that pass the check (all of
/// them with `force`).
pub fn restructure(s: &mut Session, t: &dyn Transport, p: &Provider, force: bool, log: &mut dyn FnMut(&TableRun)) -> Result<Vec<TableRun>, String> {
    let found = s.run("ai.tables", &json!({})).map_err(|e| e.to_string())?;
    let mut runs = Vec::new();
    let mut proposals = Vec::new();
    for c in found["tables"].as_array().into_iter().flatten() {
        let index = c["index"].as_u64().unwrap_or(0) as usize;
        let words = c["words"].as_u64().unwrap_or(0) as usize;
        if let Some(why) = c["blocked"].as_str() {
            let r = TableRun { index, words, seconds: 0.0, status: "skipped", kind: String::new(), detail: why.to_string() };
            log(&r);
            runs.push(r);
            continue;
        }
        let started = Instant::now();
        let answer = s
            .run("ai.tableRequest", &json!({"index": index}))
            .map_err(|e| e.to_string())
            .and_then(|req| ai::chat_json(t, p, req["system"].as_str().unwrap_or(""), req["user"].as_str().unwrap_or(""), &req["schema"]));
        let seconds = started.elapsed().as_secs_f64();
        match answer {
            Ok(a) => proposals.push((index, words, seconds, a)),
            Err(e) => {
                let r = TableRun { index, words, seconds, status: "error", kind: String::new(), detail: e };
                log(&r);
                runs.push(r);
            }
        }
    }
    if !proposals.is_empty() {
        let list: Vec<Value> = proposals.iter().map(|(i, _, _, a)| json!({"index": i, "answer": a})).collect();
        let done = s.run("ai.restructureTables", &json!({"proposals": list, "force": force})).map_err(|e| e.to_string())?;
        for (res, (index, words, seconds, _)) in done["results"].as_array().into_iter().flatten().zip(&proposals) {
            let applied = res["applied"].as_bool().unwrap_or(false);
            let checked = res["check"].as_str().map(str::to_string);
            let r = TableRun {
                index: *index,
                words: *words,
                seconds: *seconds,
                status: if applied {
                    "applied"
                } else if checked.is_some() {
                    "rejected"
                } else {
                    "error"
                },
                kind: res["kind"].as_str().unwrap_or("").to_string(),
                detail: checked.or_else(|| res["reason"].as_str().map(str::to_string)).unwrap_or_default(),
            };
            log(&r);
            runs.push(r);
        }
    }
    runs.sort_by_key(|r| r.index);
    Ok(runs)
}

fn line(r: &TableRun) -> String {
    format!("  table at block {:<4} {:>5} words  {:>6.1}s  {:<8} {:<6} {}", r.index, r.words, r.seconds, r.status, r.kind, r.detail)
}

/// `ai-tables <in> <out> --model M [--url …] [--force]`.
pub fn ai_tables(
    input: &Path,
    output: &str,
    mut p: Provider,
    model: Option<String>,
    force: bool,
    open: impl Fn(&Path) -> Result<Session, String>,
) -> Result<(), String> {
    let mut s = open(input)?;
    p.model = model.ok_or("give the model with --model (`ollama list` shows yours)")?;
    let t = Https::new();
    eprintln!("{} with {} at {}", input.display(), p.model, p.base_url);
    let runs = restructure(&mut s, &t, &p, force, &mut |r| eprintln!("{}", line(r)))?;
    if runs.is_empty() {
        eprintln!("  no table needs a model");
    }
    s.run("file.save", &json!({"path": output})).map_err(|e| e.to_string())?;
    eprintln!("wrote {output}");
    Ok(())
}

/// The Word documents of `dir`, sorted.
fn documents(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "docx" | "doc" | "odt" | "rtf")))
        .collect();
    files.sort();
    Ok(files)
}

/// `ai-bench <dir> --model A [--model B …] [--save DIR] [--report FILE]`: every model on every
/// document; a TSV report (one line per table) and a summary per model.
pub fn ai_bench(
    dir: &Path,
    p: Provider,
    models: &[String],
    save: Option<&Path>,
    report: Option<&Path>,
    open: impl Fn(&Path) -> Result<Session, String>,
) -> Result<(), String> {
    if models.is_empty() {
        return Err("give at least one --model".into());
    }
    let files = documents(dir)?;
    if files.is_empty() {
        return Err(format!("no Word documents in {} (cargo xtask corpus --tables fetches some)", dir.display()));
    }
    let t = Https::new();
    let mut tsv = String::from("model\tdocument\ttable\twords\tseconds\tstatus\tkind\tdetail\n");
    let mut summary = Vec::new();
    for model in models {
        let mut p = p.clone();
        p.model = model.clone();
        eprintln!("== {model} at {}", p.base_url);
        let (mut sent, mut applied, mut words_ok, mut words_sent, mut secs) = (0usize, 0usize, 0usize, 0usize, 0.0f64);
        for f in &files {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            eprintln!("{name}");
            let mut s = match open(f) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("  can't open: {e}");
                    tsv.push_str(&format!("{model}\t{name}\t\t\t\terror\t\topen: {}\n", e.replace(['\t', '\n'], " ")));
                    continue;
                }
            };
            let runs = match restructure(&mut s, &t, &p, false, &mut |r| eprintln!("{}", line(r))) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("  {e}");
                    continue;
                }
            };
            for r in &runs {
                tsv.push_str(&format!(
                    "{model}\t{name}\t{}\t{}\t{:.1}\t{}\t{}\t{}\n",
                    r.index,
                    r.words,
                    r.seconds,
                    r.status,
                    r.kind,
                    r.detail.replace(['\t', '\n'], " ")
                ));
                if r.status != "skipped" {
                    sent += 1;
                    words_sent += r.words;
                    secs += r.seconds;
                }
                if r.status == "applied" {
                    applied += 1;
                    words_ok += r.words;
                }
            }
            if let Some(dir) = save {
                let stem = f.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let safe: String = model.chars().map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
                let out = dir.join(format!("{stem}.{safe}.md"));
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                s.run("file.save", &json!({"path": out.to_string_lossy()})).map_err(|e| e.to_string())?;
            }
        }
        summary.push((model.clone(), sent, applied, words_ok, words_sent, secs));
    }
    if let Some(path) = report {
        std::fs::write(path, &tsv).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("wrote {}", path.display());
    }
    println!("{:<28} {:>7} {:>9} {:>12} {:>11}", "model", "tables", "accepted", "words kept", "s / table");
    for (model, sent, applied, ok, total, secs) in summary {
        let pct = |a: usize, b: usize| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
        println!(
            "{model:<28} {sent:>7} {:>8.0}% {:>11.0}% {:>11.1}",
            pct(applied, sent),
            pct(ok, total),
            if sent == 0 { 0.0 } else { secs / sent as f64 }
        );
    }
    Ok(())
}
