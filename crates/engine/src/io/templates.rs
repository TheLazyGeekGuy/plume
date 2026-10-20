//! File access for PDF templates ([`crate::pdf_template`]): template folders, pictures named by
//! settings, the work folder a TeX engine runs in, and the exported PDF. Paths that come from
//! outside Plume (template folders, pictures, the PDF) go through the automation roots
//! ([`crate::file_access`]); the private work folder under the temporary directory doesn't.

use std::path::{Path, PathBuf};

fn err(path: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", path.display())
}

fn readable(path: &Path) -> Result<(), String> {
    crate::file_access::check_read(&path.to_string_lossy())
}

fn writable(path: &Path) -> Result<(), String> {
    crate::file_access::check_write(&path.to_string_lossy())
}

/// The text of a file of at most `max` bytes.
pub fn read_text(path: &Path, max: u64) -> Result<String, String> {
    readable(path)?;
    let meta = std::fs::metadata(path).map_err(|e| err(path, e))?;
    if meta.len() > max {
        return Err(err(path, "too large"));
    }
    std::fs::read_to_string(path).map_err(|e| err(path, e))
}

/// Write `text`, creating the folders above it.
pub fn write_text(path: &Path, text: &str) -> Result<(), String> {
    writable(path)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err(dir, e))?;
    }
    std::fs::write(path, text).map_err(|e| err(path, e))
}

/// Copy a folder's files (a few levels deep) into a new folder.
pub fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    readable(from)?;
    writable(to)?;
    copy_dir_in(from, to, 0)
}

fn copy_dir_in(from: &Path, to: &Path, depth: usize) -> Result<(), String> {
    if depth > 4 {
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| err(to, e))?;
    let entries = std::fs::read_dir(from).map_err(|e| err(from, e))?;
    for e in entries.flatten().take(1000) {
        let path = e.path();
        let dest = to.join(e.file_name());
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            copy_dir_in(&path, &dest, depth + 1)?;
        } else if kind.is_file() {
            std::fs::copy(&path, &dest).map_err(|e| err(&path, e))?;
        }
    }
    Ok(())
}

/// The folders inside `dir` (none when it can't be read).
pub fn subfolders(dir: &Path) -> Vec<PathBuf> {
    if readable(dir).is_err() {
        return Vec::new();
    }
    let mut v: Vec<PathBuf> =
        std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir()).take(500).collect()).unwrap_or_default();
    v.sort();
    v
}

/// A fresh private work folder (an old one of the same name is removed).
pub fn fresh_work_dir(dir: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| err(dir, e))
}

/// Remove a private work folder.
pub fn remove_work_dir(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Write bytes into the private work folder.
pub fn write_work_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err(dir, e))?;
    }
    std::fs::write(path, bytes).map_err(|e| err(path, e))
}

/// Copy a file the document or template names (at most `max` bytes) into the work folder.
pub fn copy_into_work(src: &Path, dest: &Path, max: u64) -> Result<(), String> {
    readable(src)?;
    let meta = std::fs::metadata(src).map_err(|e| err(src, e))?;
    if !meta.is_file() || meta.len() > max {
        return Err(err(src, "not a file of a usable size"));
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err(dir, e))?;
    }
    std::fs::copy(src, dest).map(|_| ()).map_err(|e| err(src, e))
}

/// Copy a file the work folder made to where the user asked.
pub fn copy_out_of_work(src: &Path, dest: &Path) -> Result<(), String> {
    writable(dest)?;
    if let Some(dir) = dest.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| err(dir, e))?;
    }
    std::fs::copy(src, dest).map(|_| ()).map_err(|e| err(dest, e))
}

/// A work folder file's text, lossily (TeX logs aren't always UTF-8).
pub fn read_work_text(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// Run `exe` with `args` in `dir` (its output to `dir/engine-output.txt`), stopping it after
/// `timeout_secs`. Whether it succeeded.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_in(exe: &Path, args: &[&str], dir: &Path, timeout_secs: u64) -> Result<bool, String> {
    use std::process::{Command, Stdio};
    let log = std::fs::File::create(dir.join("engine-output.txt")).map_err(|e| err(dir, e))?;
    let log2 = log.try_clone().map_err(|e| err(dir, e))?;
    let mut child = Command::new(exe)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log2))
        .spawn()
        .map_err(|e| format!("{} couldn't start: {e}", exe.display()))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) if start.elapsed().as_secs() > timeout_secs => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the TeX engine took too long and was stopped".into());
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    }
}
