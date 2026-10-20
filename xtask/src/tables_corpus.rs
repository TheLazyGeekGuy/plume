//! `cargo xtask corpus --tables`: Word documents with nested and layout tables, from open
//! source test suites, pinned by commit and checked by SHA-256. They go to `corpus/tables/`
//! (git-ignored): the files are third-party and were made with Word, so they are never
//! committed. Used to measure how Plume turns old Word documents into Markdown, with and
//! without an AI model's help.

use std::path::Path;
use std::process::Command;

/// (published path, source repository, commit, path there, SHA-256, licence, why it's here).
pub const FILES: &[(&str, &str, &str, &str, &str, &str, &str)] = &[
    (
        "poi-60329.docx",
        "apache/poi",
        POI,
        "test-data/document/60329.docx",
        "e5b4e66038f92ef589e12ea870b24cc7a3e0005cd7a53817abff1307404f7aa3",
        "Apache-2.0",
        "clinical protocol cover: form fields laid out in nested tables",
    ),
    (
        "poi-drawing.docx",
        "apache/poi",
        POI,
        "test-data/document/drawing.docx",
        "5e83462dd7ac3cf0a333fe09cf0837602dbd41775cdcec813a6e288b5d55dff8",
        "Apache-2.0",
        "press review (Russian), 7,500 words, layout tables nested four deep",
    ),
    (
        "poi-form-footnotes.docx",
        "apache/poi",
        POI,
        "test-data/document/form_footnotes.docx",
        "a890294bfbdc63f6d03eab61efc1e98cb51d59f0920541150dd7082f9bdf31f1",
        "Apache-2.0",
        "a form: merged cells, footnotes in cells",
    ),
    (
        "poi-bug65649.docx",
        "apache/poi",
        POI,
        "test-data/document/bug65649.docx",
        "f98d2bfcca5d091cda26539b00318bb2b2b1e6d8aa644e370175e99ca9c1b4ad",
        "Apache-2.0",
        "large data tables, thousands of merged cells",
    ),
    (
        "poi-deep-table-cell.docx",
        "apache/poi",
        POI,
        "test-data/document/deep-table-cell.docx",
        "31c5bfe1f68c98699cb356891de2e02fe223f633706d666c430ca79fd2cdf6bb",
        "Apache-2.0",
        "hostile: tables nested 5,000 deep (must not crash or hang)",
    ),
    (
        "lo-fdo48718-1.docx",
        "LibreOffice/core",
        LO,
        "sw/qa/extras/layout/data/fdo48718-1.docx",
        "158005c960c26f00006bc292834c279ddf6c2b2b5510fac82aa63106f75e50fa",
        "MPL-2.0",
        "attendance and evaluation report (Spanish): an administrative form in a table",
    ),
    (
        "lo-fdo43573-2-min.docx",
        "LibreOffice/core",
        LO,
        "sw/qa/extras/layout/data/fdo43573-2-min.docx",
        "eb12e7d4ce90ffa6648433b365922ab4727e63bd709f439adda6560e31677e1f",
        "MPL-2.0",
        "book chapter (Dutch), 5,500 words, tables in tables",
    ),
    (
        "lo-floattable-del-empty.docx",
        "LibreOffice/core",
        LO,
        "sw/qa/core/layout/data/floattable-del-empty.docx",
        "015dbec5c738261ffa4f3e287a9c6eb4363b8caa8671ce4fda88c6a0d556ec9a",
        "MPL-2.0",
        "long text (Spanish) laid out in floating and nested tables",
    ),
    (
        "lo-floattable-hidemark.docx",
        "LibreOffice/core",
        LO,
        "sw/qa/writerfilter/dmapper/data/floattable-hidemark.docx",
        "536b79cef636c6642be3266416c2c59fd1914fafa836529bd527b5b145f5a634",
        "MPL-2.0",
        "a newsletter letter laid out with a table in a table",
    ),
    (
        "lo-floattable-nested-rowspan.docx",
        "LibreOffice/core",
        LO,
        "sw/qa/core/layout/data/floattable-nested-rowspan.docx",
        "5c5eb1f7d326d2218f1c941cb60805d143d326013d30c9397df09368f2da93f2",
        "MPL-2.0",
        "exercise sheet: three levels of tables with merged rows",
    ),
];

const POI: &str = "ebf602f1bc748f9ec9b29553070ff62170e3243b";
const LO: &str = "a80284e2ae8951c9ef9fa3332a1b8949ce01f9eb";

fn sha256(path: &Path) -> Result<String, String> {
    let out = Command::new("shasum").args(["-a", "256"]).arg(path).output().map_err(|e| format!("shasum: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace().next().map(str::to_string).ok_or_else(|| format!("shasum gave nothing for {}", path.display()))
}

pub fn download(corpus: &Path) -> Result<(), String> {
    let dest = corpus.join("tables");
    std::fs::create_dir_all(&dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut index = String::from("file\tsource\tlicence\twhy\n");
    for (name, repo, commit, path, sum, licence, why) in FILES {
        let file = dest.join(name);
        if !(file.exists() && sha256(&file)? == *sum) {
            let url = format!("https://raw.githubusercontent.com/{repo}/{commit}/{path}");
            let mut curl = Command::new("curl");
            curl.args(["-fsSL", "-o"]).arg(&file).arg(&url);
            crate::run(curl, &format!("curl {url}"))?;
            let got = sha256(&file)?;
            if got != *sum {
                let _ = std::fs::remove_file(&file);
                return Err(format!("{name}: SHA-256 {got}, expected {sum}"));
            }
        }
        index.push_str(&format!("{name}\thttps://github.com/{repo}/blob/{commit}/{path}\t{licence}\t{why}\n"));
    }
    std::fs::write(dest.join("SOURCES.tsv"), index).map_err(|e| format!("write SOURCES.tsv: {e}"))?;
    println!("Tables corpus: {} Word documents in {}", FILES.len(), dest.display());
    Ok(())
}
