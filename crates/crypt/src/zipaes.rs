//! ZIP archives with encrypted entries.
//!
//! Written with WinZip AES-256 (AE-2: PBKDF2-HMAC-SHA1 key, AES-256-CTR, HMAC-SHA1 per entry),
//! which 7-Zip, WinZip, macOS's Archive Utility (recent versions), Keka and others open. Read
//! with WinZip AES (128, 192 or 256 bits) or the legacy ZipCrypto, which is weak and only ever
//! read, so an old archive can be opened and saved again properly. The work is done by the
//! `zip` crate's WinZip AES implementation.
//!
//! An encrypted ZIP keeps its file names, sizes and dates in clear: only the contents are
//! protected. Callers say so to the user (an AES Crypt file around a ZIP hides them).

use std::io::Cursor;
#[cfg(feature = "encryption")]
use std::io::{Read, Write};

use zip::ZipArchive;
#[cfg(feature = "encryption")]
use {
    zeroize::Zeroizing,
    zip::write::SimpleFileOptions,
    zip::{AesMode, CompressionMethod, ZipWriter},
};

use crate::{Error, MAX_SIZE, Result};

/// Most entries an archive may hold.
pub const MAX_ENTRIES: usize = 20_000;

/// How a ZIP's entries are encrypted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encryption {
    None,
    /// WinZip AES (some entries at least, and no legacy ones).
    Aes,
    /// Some entries with the legacy ZipCrypto.
    Legacy,
}

/// Do `bytes` start like a ZIP (a local file header, or the end record of an empty archive)?
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

fn open(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>> {
    if bytes.len() > MAX_SIZE {
        return Err(Error::TooLarge);
    }
    let zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| Error::Malformed(format!("ZIP: {e}")))?;
    if zip.len() > MAX_ENTRIES {
        return Err(Error::Malformed(format!("the ZIP holds more than {MAX_ENTRIES} entries")));
    }
    Ok(zip)
}

/// How the archive's entries are encrypted.
pub fn encryption(bytes: &[u8]) -> Result<Encryption> {
    let mut zip = open(bytes)?;
    let mut found = Encryption::None;
    for i in 0..zip.len() {
        let encrypted = zip.by_index_raw(i).map(|f| f.encrypted()).map_err(|e| Error::Malformed(format!("ZIP: {e}")))?;
        if !encrypted {
            continue;
        }
        #[cfg(feature = "encryption")]
        match zip.get_aes_verification_key_and_salt(i) {
            Ok(Some(_)) => found = Encryption::Aes,
            _ => return Ok(Encryption::Legacy),
        }
        // Without the ciphers, an encrypted entry is told apart by its compression method: 99
        // marks WinZip AES.
        #[cfg(not(feature = "encryption"))]
        if aes_entries(bytes) {
            found = Encryption::Aes;
        } else {
            return Ok(Encryption::Legacy);
        }
    }
    Ok(found)
}

/// Without the ciphers: does a central directory entry name compression method 99 (WinZip AES)?
#[cfg(not(feature = "encryption"))]
fn aes_entries(bytes: &[u8]) -> bool {
    bytes.windows(12).any(|w| w.starts_with(b"PK\x01\x02") && w.get(10..12) == Some(&[99, 0][..]))
}

#[cfg(feature = "encryption")]
/// One entry, read.
struct Entry {
    name: String,
    dir: bool,
    method: CompressionMethod,
    modified: Option<zip::DateTime>,
    mode: Option<u32>,
    data: Zeroizing<Vec<u8>>,
}

#[cfg(feature = "encryption")]
/// Every entry of `bytes`, decrypted with `password` where encrypted. Each entry's integrity is
/// checked (WinZip AES's HMAC; for legacy and plain entries, their CRC-32).
fn entries(bytes: &[u8], password: Option<&str>) -> Result<Vec<Entry>> {
    let mut zip = open(bytes)?;
    let mut out = Vec::with_capacity(zip.len());
    let mut total = 0usize;
    for i in 0..zip.len() {
        let (encrypted, dir) = {
            let f = zip.by_index_raw(i).map_err(|e| Error::Malformed(format!("ZIP: {e}")))?;
            (f.encrypted(), f.is_dir())
        };
        let file = match (encrypted, password) {
            (false, _) => zip.by_index(i),
            (true, None) => return Err(Error::PasswordRequired),
            (true, Some(pw)) => zip.by_index_decrypt(i, pw.as_bytes()),
        };
        let mut file = file.map_err(|e| match e {
            zip::result::ZipError::InvalidPassword => Error::WrongPassword,
            zip::result::ZipError::UnsupportedArchive(m) => Error::Unsupported(m.to_string()),
            e => Error::Malformed(format!("ZIP: {e}")),
        })?;
        let name = file.name().to_string();
        let method = file.compression();
        let modified = file.last_modified();
        let mode = file.unix_mode();
        let room = MAX_SIZE.saturating_sub(total);
        let mut data = Zeroizing::new(Vec::with_capacity(usize::try_from(file.size()).unwrap_or(0).min(room).min(64 << 20)));
        // An integrity failure (HMAC or CRC) surfaces as a read error at the entry's end.
        (&mut file)
            .take(room as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| if encrypted { Error::Altered } else { Error::Malformed(format!("ZIP: entry {name} is damaged")) })?;
        if data.len() > room {
            return Err(Error::TooLarge);
        }
        total += data.len();
        out.push(Entry { name, dir, method, modified, mode, data });
    }
    Ok(out)
}

#[cfg(feature = "encryption")]
fn write(entries: &[Entry], password: Option<&str>) -> Result<Vec<u8>> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    let err = |e: zip::result::ZipError| Error::Malformed(format!("ZIP: {e}"));
    for e in entries {
        let method = if e.method == CompressionMethod::Stored { CompressionMethod::Stored } else { CompressionMethod::Deflated };
        let mut o = SimpleFileOptions::default().compression_method(method).large_file(e.data.len() as u64 >= 0xFFFF_FFFF);
        if let Some(t) = e.modified {
            o = o.last_modified_time(t);
        }
        if let Some(m) = e.mode {
            o = o.unix_permissions(m & 0o7777);
        }
        if e.dir {
            w.add_directory(e.name.trim_end_matches('/'), o).map_err(err)?;
            continue;
        }
        let o = match password {
            Some(pw) => o.with_aes_encryption(AesMode::Aes256, pw),
            None => o,
        };
        w.start_file(e.name.as_str(), o).map_err(err)?;
        w.write_all(&e.data).map_err(|e| Error::Malformed(format!("ZIP: {e}")))?;
    }
    Ok(w.finish().map_err(err)?.into_inner())
}

/// The ZIP `plain` with every file entry encrypted with WinZip AES-256 under `password`. Names,
/// folders, dates and permissions are kept, and each entry stays stored or compressed as it was;
/// already encrypted entries are refused (decrypt first).
#[cfg(feature = "encryption")]
pub fn encrypt(plain: &[u8], password: &str) -> Result<Vec<u8>> {
    if password.is_empty() {
        return Err(Error::EmptyPassword);
    }
    let entries = entries(plain, None)?;
    write(&entries, Some(password))
}

/// The ZIP `bytes` with its entries decrypted with `password` (WinZip AES or legacy ZipCrypto):
/// an ordinary ZIP of the same files. Every entry's integrity is checked first.
#[cfg(feature = "encryption")]
pub fn decrypt(bytes: &[u8], password: &str) -> Result<Vec<u8>> {
    if password.is_empty() {
        return Err(Error::PasswordRequired);
    }
    let entries = entries(bytes, Some(password))?;
    write(&entries, None)
}

/// Without the `encryption` feature: [`Error::Unavailable`].
#[cfg(not(feature = "encryption"))]
pub fn encrypt(_plain: &[u8], _password: &str) -> Result<Vec<u8>> {
    Err(Error::Unavailable)
}

/// Without the `encryption` feature: [`Error::Unavailable`].
#[cfg(not(feature = "encryption"))]
pub fn decrypt(_bytes: &[u8], _password: &str) -> Result<Vec<u8>> {
    Err(Error::Unavailable)
}

#[cfg(test)]
#[cfg(feature = "encryption")]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = ZipWriter::new(Cursor::new(Vec::new()));
        for (n, b) in files {
            w.start_file(*n, SimpleFileOptions::default()).unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn files(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut z = ZipArchive::new(Cursor::new(bytes)).unwrap();
        (0..z.len())
            .map(|i| {
                let mut f = z.by_index(i).unwrap();
                let mut b = Vec::new();
                f.read_to_end(&mut b).unwrap();
                (f.name().to_string(), b)
            })
            .collect()
    }

    #[test]
    fn encrypts_every_entry_and_decrypts_them_back() {
        let plain = zip_of(&[("report.md", b"# Report\n\n![](images/a.png)\n"), ("images/a.png", b"\x89PNG fake"), ("unused.txt", b"kept")]);
        let enc = encrypt(&plain, "s3cret").unwrap();
        assert_eq!(encryption(&enc).unwrap(), Encryption::Aes);
        let mut z = ZipArchive::new(Cursor::new(enc.as_slice())).unwrap();
        assert!((0..z.len()).all(|i| z.by_index_raw(i).unwrap().encrypted()));
        // Names stay readable; contents don't.
        assert!(z.file_names().any(|n| n == "images/a.png"));
        assert!(!enc.windows(8).any(|w| w == b"# Report"));
        assert!(z.by_index(0).is_err());
        let back = decrypt(&enc, "s3cret").unwrap();
        assert_eq!(files(&back), files(&plain));
        assert_eq!(encryption(&back).unwrap(), Encryption::None);
    }

    #[test]
    fn wrong_password_and_tampering_are_errors() {
        let enc = encrypt(&zip_of(&[("a.md", b"secret text, long enough to matter")]), "right").unwrap();
        assert!(matches!(decrypt(&enc, "wrong"), Err(Error::WrongPassword | Error::Altered)));
        assert_eq!(entries(&enc, None).err(), Some(Error::PasswordRequired));
        // Flip a byte of the encrypted contents (after the local header and the salt).
        let at = enc.windows(4).position(|w| w == b"a.md").unwrap() + 4 + 11 + 16 + 2 + 3;
        let mut bad = enc.clone();
        bad[at] ^= 0x40;
        assert!(decrypt(&bad, "right").is_err());
    }

    #[test]
    fn legacy_zipcrypto_is_read_but_never_written() {
        // `a.md` holding `# Old`, password `old`, written by Info-ZIP's `zip -P`.
        let hex = "504b03040a0009000000044e555de0462018110000000500000004001c00612e6d645554090003578ad86a578ad86a75780b000104000000000400000000f8545b34a853b4296530b2ffd38b87116c504b0708e04620181100000005000000504b01021e030a0009000000044e555de04620181100000005000000040018000000000001000000a48100000000612e6d645554050003578ad86a75780b000104000000000400000000504b050600000000010001004a0000005f0000000000";
        let legacy: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        assert_eq!(encryption(&legacy).unwrap(), Encryption::Legacy);
        let plain = decrypt(&legacy, "old").unwrap();
        assert_eq!(files(&plain), vec![("a.md".to_string(), b"# Old".to_vec())]);
        assert_eq!(encryption(&encrypt(&plain, "new").unwrap()).unwrap(), Encryption::Aes);
    }

    #[test]
    fn hostile_archives_never_panic() {
        assert!(encrypt(b"PK\x03\x04junk", "pw").is_err());
        assert!(decrypt(b"", "pw").is_err());
        assert_eq!(encrypt(&zip_of(&[]), "").unwrap_err(), Error::EmptyPassword);
        let ok = zip_of(&[("a", b"b")]);
        for n in 0..ok.len() {
            let _ = encryption(&ok[..n]);
            let _ = decrypt(&ok[..n], "pw");
        }
    }
}
