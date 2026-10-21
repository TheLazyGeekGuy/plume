//! AES Crypt files (`.aes`), as the AES Crypt tools write and read them.
//!
//! Stream format version 3, written:
//!
//! | Bytes | Field |
//! |---|---|
//! | 3 | `AES` |
//! | 1 | version, `0x03` |
//! | 1 | reserved, `0x00` |
//! | 2 + n… | extensions: each a big-endian length and that many bytes; length 0 ends them |
//! | 4 | PBKDF2 iterations, big-endian |
//! | 16 | public IV: random, the PBKDF2 salt and the IV of the next field |
//! | 48 | session IV (16) and session key (32), AES-256-CBC with the password's key |
//! | 32 | HMAC-SHA256, with the password's key, of the previous 48 bytes and the version byte |
//! | 16·n | the file, AES-256-CBC with the session key and IV, PKCS#7 padded |
//! | 32 | HMAC-SHA256, with the session key, of the encrypted file |
//!
//! The password's key is PBKDF2-HMAC-SHA512 of the password (UTF-8) with the public IV as salt.
//! Version 2 (read only) has no iteration field, derives the key with 8 192 rounds of SHA-256
//! over the key so far and the password in UTF-16LE (the first key being the IV, zero-padded),
//! leaves the version byte out of the first HMAC, and ends the file with the length of its last
//! block (one byte, 0 meaning 16) before the last HMAC.

#[cfg(feature = "encryption")]
use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
#[cfg(feature = "encryption")]
use hmac::{Hmac, Mac};
#[cfg(feature = "encryption")]
use sha2::{Digest, Sha256, Sha512};
use zeroize::Zeroizing;

use crate::{Error, Result};
#[cfg(feature = "encryption")]
use crate::{MAX_SIZE, random, same};

/// The PBKDF2 iterations AES Crypt itself uses by default.
pub const DEFAULT_ITERATIONS: u32 = 300_000;
/// The most PBKDF2 iterations a file may ask for when read (more would only stall the reader).
pub const MAX_ITERATIONS: u32 = 5_000_000;
#[cfg(feature = "encryption")]
/// Most extensions skipped before giving up on a header.
const MAX_EXTENSIONS: usize = 256;
#[cfg(feature = "encryption")]
/// Version 2's key derivation rounds.
const ACKDF_ROUNDS: usize = 8192;

#[cfg(feature = "encryption")]
type HmacSha256 = Hmac<Sha256>;

/// Without the `encryption` feature: [`Error::Unavailable`].
#[cfg(not(feature = "encryption"))]
pub fn encrypt(_plain: &[u8], _password: &str) -> Result<Vec<u8>> {
    Err(Error::Unavailable)
}

/// Without the `encryption` feature: [`Error::Unavailable`].
#[cfg(not(feature = "encryption"))]
pub fn encrypt_with(_plain: &[u8], _password: &str, _iterations: u32) -> Result<Vec<u8>> {
    Err(Error::Unavailable)
}

/// Without the `encryption` feature: [`Error::Unavailable`].
#[cfg(not(feature = "encryption"))]
pub fn decrypt(_bytes: &[u8], _password: &str) -> Result<Zeroizing<Vec<u8>>> {
    Err(Error::Unavailable)
}

/// The stream format version of an AES Crypt file, if `bytes` is one this module reads (2 or 3).
pub fn version(bytes: &[u8]) -> Option<u8> {
    match bytes {
        [b'A', b'E', b'S', v @ (2 | 3), 0, ..] => Some(*v),
        _ => None,
    }
}

/// Encrypt `plain` with `password` in AES Crypt's format version 3, with its default iterations.
#[cfg(feature = "encryption")]
pub fn encrypt(plain: &[u8], password: &str) -> Result<Vec<u8>> {
    encrypt_with(plain, password, DEFAULT_ITERATIONS)
}

/// [`encrypt`] with `iterations` PBKDF2 iterations (1 to [`MAX_ITERATIONS`]).
#[cfg(feature = "encryption")]
pub fn encrypt_with(plain: &[u8], password: &str, iterations: u32) -> Result<Vec<u8>> {
    if password.is_empty() {
        return Err(Error::EmptyPassword);
    }
    if plain.len() > MAX_SIZE {
        return Err(Error::TooLarge);
    }
    let iterations = iterations.clamp(1, MAX_ITERATIONS);
    let public_iv = random::<16>()?;
    let session_iv = random::<16>()?;
    let session_key = random::<32>()?;
    let setup_key = pbkdf2_key(password, &public_iv[..], iterations)?;

    let mut out = Vec::with_capacity(plain.len() + 160);
    out.extend_from_slice(b"AES\x03\x00");
    // No extensions: nothing about the file or the tool travels in clear.
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&iterations.to_be_bytes());
    out.extend_from_slice(public_iv.as_ref());

    let mut session = Zeroizing::new([0u8; 48]);
    session[..16].copy_from_slice(session_iv.as_ref());
    session[16..].copy_from_slice(session_key.as_ref());
    let block = cbc_encrypt(&setup_key[..], &public_iv[..], session.as_ref())?;
    let mut mac = hmac(&setup_key[..])?;
    mac.update(&block);
    mac.update(&[3]);
    out.extend_from_slice(&block);
    out.extend_from_slice(&mac.finalize().into_bytes());

    let pad = 16 - plain.len() % 16;
    let mut padded = Zeroizing::new(Vec::with_capacity(plain.len() + pad));
    padded.extend_from_slice(plain);
    padded.extend(std::iter::repeat_n(pad as u8, pad));
    let body = cbc_encrypt(&session_key[..], &session_iv[..], &padded)?;
    let mut mac = hmac(&session_key[..])?;
    mac.update(&body);
    out.extend_from_slice(&body);
    out.extend_from_slice(&mac.finalize().into_bytes());
    Ok(out)
}

/// Decrypt an AES Crypt file (format version 2 or 3) with `password`. Both HMACs are checked
/// before anything is decrypted: a wrong password is [`Error::WrongPassword`], a file altered
/// after it was encrypted [`Error::Altered`].
#[cfg(feature = "encryption")]
pub fn decrypt(bytes: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.len() > MAX_SIZE.saturating_add(4096) {
        return Err(Error::TooLarge);
    }
    let malformed = |m: &str| Error::Malformed(m.to_string());
    let v = match bytes {
        [b'A', b'E', b'S', v, ..] if *v > 3 || *v < 2 => return Err(Error::Unsupported(format!("AES Crypt format version {v}"))),
        _ => version(bytes).ok_or_else(|| malformed("no AES Crypt header"))?,
    };
    let mut r = Reader { bytes, at: 5 };
    for n in 0.. {
        if n >= MAX_EXTENSIONS {
            return Err(malformed("too many header extensions"));
        }
        let len = u16::from_be_bytes(r.take::<2>()?);
        if len == 0 {
            break;
        }
        r.skip(usize::from(len))?;
    }
    let iterations = if v == 3 { u32::from_be_bytes(r.take::<4>()?) } else { 0 };
    if v == 3 && !(1..=MAX_ITERATIONS).contains(&iterations) {
        return Err(malformed("unreasonable key derivation iterations"));
    }
    let public_iv: [u8; 16] = r.take()?;
    let block: [u8; 48] = r.take()?;
    let tag: [u8; 32] = r.take()?;
    if password.is_empty() {
        return Err(Error::PasswordRequired);
    }
    let setup_key = if v == 3 { pbkdf2_key(password, &public_iv, iterations)? } else { ackdf_key(password, &public_iv) };
    let mut mac = hmac(&setup_key[..])?;
    mac.update(&block);
    if v == 3 {
        mac.update(&[3]);
    }
    if !same(&mac.finalize().into_bytes(), &tag) {
        return Err(Error::WrongPassword);
    }
    let session = cbc_decrypt(&setup_key[..], &public_iv, &block)?;
    let (session_iv, session_key) = session.split_at(16);

    let rest = bytes.get(r.at..).unwrap_or_default();
    let trailer = if v == 3 { 32 } else { 33 };
    let body_len = rest.len().checked_sub(trailer).ok_or(Error::Altered)?;
    let (body, end) = rest.split_at(body_len);
    if body.len() % 16 != 0 || (v == 3 && body.is_empty()) {
        return Err(Error::Altered);
    }
    let (last_len, tag) = if v == 3 { (None, end) } else { (end.first().copied(), end.get(1..).unwrap_or_default()) };
    let mut mac = hmac(session_key)?;
    mac.update(body);
    if !same(&mac.finalize().into_bytes(), tag) {
        return Err(Error::Altered);
    }
    let mut plain = cbc_decrypt(session_key, session_iv, body)?;
    let keep = match last_len {
        // Version 3: PKCS#7 padding, checked whole.
        None => {
            let pad = usize::from(plain.last().copied().unwrap_or(0));
            let start = plain.len().checked_sub(pad).filter(|_| (1..=16).contains(&pad)).ok_or(Error::Altered)?;
            if !plain.get(start..).unwrap_or_default().iter().all(|b| usize::from(*b) == pad) {
                return Err(Error::Altered);
            }
            start
        }
        // Version 2: the last block's length.
        Some(_) if plain.is_empty() => 0,
        Some(m) => {
            let m = usize::from(m & 0x0F);
            plain.len().saturating_sub(16) + if m == 0 { 16 } else { m }
        }
    };
    plain.truncate(keep);
    Ok(plain)
}

#[cfg(feature = "encryption")]
/// A cursor over a header.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

#[cfg(feature = "encryption")]
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or_else(|| Error::Malformed("truncated header".into()))?;
        let b = self.bytes.get(self.at..end).and_then(|s| <[u8; N]>::try_from(s).ok()).ok_or_else(|| Error::Malformed("truncated header".into()))?;
        self.at = end;
        Ok(b)
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.bytes.len()).ok_or_else(|| Error::Malformed("truncated header".into()))?;
        self.at = end;
        Ok(())
    }
}

#[cfg(feature = "encryption")]
/// Version 3's key: PBKDF2-HMAC-SHA512 of the password with the public IV as salt.
fn pbkdf2_key(password: &str, salt: &[u8], iterations: u32) -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2::<Hmac<Sha512>>(password.as_bytes(), salt, iterations, key.as_mut()).map_err(|_| Error::Malformed("key derivation".into()))?;
    Ok(key)
}

#[cfg(feature = "encryption")]
/// Version 2's key: SHA-256, 8 192 times, of the key so far and the password in UTF-16LE.
fn ackdf_key(password: &str, iv: &[u8; 16]) -> Zeroizing<[u8; 32]> {
    let pw: Zeroizing<Vec<u8>> = Zeroizing::new(password.encode_utf16().flat_map(u16::to_le_bytes).collect());
    let mut key = Zeroizing::new([0u8; 32]);
    key[..16].copy_from_slice(iv);
    for _ in 0..ACKDF_ROUNDS {
        let mut h = Sha256::new();
        h.update(key.as_ref());
        h.update(pw.as_slice());
        key.copy_from_slice(&h.finalize());
    }
    key
}

#[cfg(feature = "encryption")]
fn hmac(key: &[u8]) -> Result<HmacSha256> {
    <HmacSha256 as hmac::KeyInit>::new_from_slice(key).map_err(|_| Error::Malformed("bad key".into()))
}

#[cfg(feature = "encryption")]
fn cipher(key: &[u8]) -> Result<aes::Aes256> {
    aes::Aes256::new_from_slice(key).map_err(|_| Error::Malformed("bad key".into()))
}

#[cfg(feature = "encryption")]
/// AES-256-CBC of whole blocks.
fn cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let aes = cipher(key)?;
    let mut prev = aes::Block::default();
    prev.copy_from_slice(iv.get(..16).ok_or_else(|| Error::Malformed("bad IV".into()))?);
    let mut out = Vec::with_capacity(data.len());
    for b in data.chunks_exact(16) {
        let mut x = aes::Block::default();
        for ((o, p), v) in x.iter_mut().zip(b).zip(prev.iter()) {
            *o = p ^ v;
        }
        aes.encrypt_block(&mut x);
        prev = x;
        out.extend_from_slice(&x);
    }
    Ok(out)
}

#[cfg(feature = "encryption")]
/// AES-256-CBC decryption of whole blocks.
fn cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let aes = cipher(key)?;
    let mut prev = aes::Block::default();
    prev.copy_from_slice(iv.get(..16).ok_or_else(|| Error::Malformed("bad IV".into()))?);
    let mut out = Zeroizing::new(Vec::with_capacity(data.len()));
    for b in data.chunks_exact(16) {
        let mut x = aes::Block::default();
        x.copy_from_slice(b);
        aes.decrypt_block(&mut x);
        for (o, v) in x.iter_mut().zip(prev.iter()) {
            *o ^= v;
        }
        prev.copy_from_slice(b);
        out.extend_from_slice(&x);
    }
    Ok(out)
}

#[cfg(test)]
#[cfg(feature = "encryption")]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Files the AES Crypt tool wrote decrypt to their text, in both versions.
    #[test]
    fn reads_files_the_aes_crypt_tool_wrote() {
        for (v, plain, file) in crate::vectors::VECTORS {
            let bytes = hex(file);
            assert_eq!(version(&bytes), Some(*v));
            assert_eq!(decrypt(&bytes, "Hello").unwrap().as_slice(), plain.as_bytes(), "v{v} {plain:?}");
            assert_eq!(decrypt(&bytes, "hello").unwrap_err(), Error::WrongPassword);
        }
    }

    #[test]
    fn round_trips_every_length() {
        for n in [0usize, 1, 15, 16, 17, 31, 32, 33, 1000] {
            let plain: Vec<u8> = (0..n).map(|i| (i * 7) as u8).collect();
            let file = encrypt_with(&plain, "mot de passe é", 3).unwrap();
            assert_eq!(version(&file), Some(3));
            assert_eq!(file.len(), 5 + 2 + 4 + 16 + 48 + 32 + (n / 16 + 1) * 16 + 32);
            assert_eq!(decrypt(&file, "mot de passe é").unwrap().as_slice(), plain.as_slice());
        }
        // Fresh salt, IV and key each time.
        assert_ne!(encrypt_with(b"same", "pw", 1).unwrap(), encrypt_with(b"same", "pw", 1).unwrap());
    }

    #[test]
    fn the_header_says_its_iterations_and_carries_nothing_else() {
        let file = encrypt(b"x", "pw").unwrap();
        assert_eq!(&file[..7], b"AES\x03\x00\x00\x00");
        assert_eq!(u32::from_be_bytes(file[7..11].try_into().unwrap()), DEFAULT_ITERATIONS);
    }

    #[test]
    fn any_change_is_caught_before_decrypting() {
        let file = encrypt_with(b"The quarterly figures, confidential.", "pw", 2).unwrap();
        for i in 0..file.len() {
            let mut bad = file.clone();
            bad[i] ^= 0x01;
            assert!(decrypt(&bad, "pw").is_err(), "byte {i} flipped");
        }
        // Truncated or extended.
        for n in [0, 5, 20, 100, file.len() - 1] {
            assert!(decrypt(&file[..n], "pw").is_err(), "{n} bytes");
        }
        let mut longer = file.clone();
        longer.push(0);
        assert_eq!(decrypt(&longer, "pw").unwrap_err(), Error::Altered);
    }

    #[test]
    fn hostile_headers_never_panic() {
        let mut huge = b"AES\x03\x00\x00\x00".to_vec();
        huge.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(decrypt(&huge, "pw"), Err(Error::Malformed(_))));
        assert!(matches!(decrypt(b"AES\x09\x00", "pw"), Err(Error::Unsupported(_))));
        let mut ext = b"AES\x03\x00".to_vec();
        for _ in 0..300 {
            ext.extend_from_slice(&[0, 1, 0]);
        }
        assert!(decrypt(&ext, "pw").is_err());
        for n in 0..64 {
            let junk: Vec<u8> = b"AES\x02\x00".iter().copied().chain((0..n).map(|i| i as u8)).collect();
            let _ = decrypt(&junk, "pw");
        }
        assert_eq!(encrypt(b"x", "").unwrap_err(), Error::EmptyPassword);
        let file = encrypt_with(b"x", "pw", 1).unwrap();
        assert_eq!(decrypt(&file, "").unwrap_err(), Error::PasswordRequired);
    }
}
