//! Plume: the folder side of moving document packages ([`crate::transport`]): reading every file
//! of a TextBundle folder, and writing a package's files under a folder. Every path is checked
//! against the automation roots ([`crate::file_access`]) before it is touched.

use std::path::{Path, PathBuf};

use super::textbundle::{MAX_ENTRIES, MAX_ENTRY, MAX_TOTAL};

/// Is there anything (a file, a folder, a link) at `path`?
pub fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Every regular file under `dir`, by path relative to it (`/`-separated), sorted by name.
/// Links are not followed, `.DS_Store` files are left out, and the count, each file's size and
/// the total are capped, as for a TextPack.
pub fn files_under(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    crate::file_access::check_read(&dir.to_string_lossy())?;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total = 0u64;
    let mut stack = vec![(dir.to_path_buf(), String::new(), 0usize)];
    while let Some((d, prefix, depth)) = stack.pop() {
        if depth > 16 {
            continue;
        }
        let mut entries: Vec<_> = std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let fname = e.file_name().to_string_lossy().to_string();
            let rel = format!("{prefix}{fname}");
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() || fname == ".DS_Store" {
                continue;
            }
            if ft.is_dir() {
                stack.push((e.path(), format!("{rel}/"), depth + 1));
            } else if ft.is_file() {
                if files.len() >= MAX_ENTRIES {
                    return Err(format!("{}: more than {MAX_ENTRIES} files", dir.display()));
                }
                let bytes = super::read_file_capped(&e.path().to_string_lossy(), MAX_ENTRY, "file")?;
                total += bytes.len() as u64;
                if total > MAX_TOTAL {
                    return Err(format!("{}: larger than {} MB", dir.display(), MAX_TOTAL >> 20));
                }
                files.push((rel, bytes));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Write `entries` (paths already validated as inside their destination), creating folders.
pub fn write_all(entries: &[(PathBuf, &[u8])]) -> Result<(), String> {
    for (path, bytes) in entries {
        crate::file_access::check_write(&path.to_string_lossy())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        super::write_file(&path.to_string_lossy(), bytes)?;
    }
    Ok(())
}
