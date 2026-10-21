//! Plume: password encryption of whole files, in published formats other tools open too.
//!
//! - [`aescrypt`]: AES Crypt files (`.aes`), written in its stream format version 3
//!   (PBKDF2-HMAC-SHA512, AES-256-CBC, HMAC-SHA256), read in versions 2 and 3.
//! - [`zipaes`]: ZIP archives whose entries are encrypted with WinZip AES-256 (AE-2), written and
//!   read; ZIP's legacy "ZipCrypto" is read only, never written (it is weak).
//!
//! Nothing here is a cipher of Plume's own: the primitives are RustCrypto's (AES, SHA-2, HMAC,
//! PBKDF2) and the `zip` crate's WinZip AES. Integrity is checked before any plaintext is
//! returned: a wrong password, or a file altered after it was encrypted, is an error, never
//! garbage. Passwords and keys are wiped from memory when dropped and never appear in an error
//! message. Whole files are processed in memory, up to [`MAX_SIZE`].
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod aescrypt;
pub mod zipaes;

#[cfg(test)]
#[cfg(feature = "encryption")]
mod vectors;

/// Is encryption compiled into this build (the `encryption` feature)?
pub const ENCRYPTION: bool = cfg!(feature = "encryption");

/// What a build without encryption says when asked to encrypt or decrypt.
pub const NO_ENCRYPTION: &str = "this build of Plume has no encryption: encrypted files can't be opened or saved";

/// The largest file encrypted or decrypted, bytes.
pub const MAX_SIZE: usize = 1 << 30;

/// Why a file couldn't be encrypted or decrypted. The messages name no password or key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The file is encrypted and no password was given.
    PasswordRequired,
    /// The password is wrong (or the file's key block was altered: the two can't be told apart).
    WrongPassword,
    /// The password opened the file but its contents were altered, truncated or corrupted.
    Altered,
    /// Not a file of the expected format, or a damaged one.
    Malformed(String),
    /// A version or method this module doesn't read.
    Unsupported(String),
    /// Larger than [`MAX_SIZE`].
    TooLarge,
    /// An empty password: encryption with it would protect nothing.
    EmptyPassword,
    /// The system gave no secure random numbers.
    NoRandom,
    /// This build has no encryption (the `encryption` feature is off).
    Unavailable,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::PasswordRequired => f.write_str("this file is encrypted: a password is required"),
            Error::WrongPassword => f.write_str("the password is incorrect"),
            Error::Altered => f.write_str("the encrypted file was altered or damaged: its integrity check failed"),
            Error::Malformed(m) => write!(f, "not a valid encrypted file: {m}"),
            Error::Unsupported(m) => write!(f, "unsupported encryption: {m}"),
            Error::TooLarge => write!(f, "the file is larger than {} MB", MAX_SIZE >> 20),
            Error::EmptyPassword => f.write_str("the password is empty"),
            Error::NoRandom => f.write_str("no secure random numbers are available"),
            Error::Unavailable => f.write_str(NO_ENCRYPTION),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// What a file is, by its first bytes (and, for a ZIP, its entries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// AES Crypt, stream format `version`.
    AesCrypt(u8),
    /// A ZIP whose entries are encrypted with WinZip AES.
    ZipAes,
    /// A ZIP with entries encrypted with the legacy ZipCrypto (weak).
    ZipLegacy,
    /// A ZIP without encryption.
    Zip,
    /// An age file (binary or ASCII-armored).
    Age,
    /// A 7-Zip archive (it may or may not be encrypted).
    SevenZip,
    /// None of these.
    Other,
}

impl Kind {
    /// Is the file encrypted (as far as can be told without a password)?
    pub fn is_encrypted(self) -> bool {
        matches!(self, Kind::AesCrypt(_) | Kind::ZipAes | Kind::ZipLegacy | Kind::Age)
    }

    /// A short English name for messages and reports.
    pub fn name(self) -> &'static str {
        match self {
            Kind::AesCrypt(_) => "AES Crypt",
            Kind::ZipAes => "ZIP AES-256",
            Kind::ZipLegacy => "ZIP (legacy ZipCrypto)",
            Kind::Zip => "ZIP",
            Kind::Age => "age",
            Kind::SevenZip => "7-Zip",
            Kind::Other => "unencrypted",
        }
    }
}

/// What `bytes` are.
pub fn detect(bytes: &[u8]) -> Kind {
    if let Some(v) = aescrypt::version(bytes) {
        return Kind::AesCrypt(v);
    }
    if bytes.starts_with(b"age-encryption.org/v1\n") || bytes.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----") {
        return Kind::Age;
    }
    if bytes.starts_with(&[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C]) {
        return Kind::SevenZip;
    }
    if zipaes::is_zip(bytes) {
        return match zipaes::encryption(bytes) {
            Ok(zipaes::Encryption::Aes) => Kind::ZipAes,
            Ok(zipaes::Encryption::Legacy) => Kind::ZipLegacy,
            _ => Kind::Zip,
        };
    }
    Kind::Other
}

/// `n` secure random bytes.
#[cfg(feature = "encryption")]
pub(crate) fn random<const N: usize>() -> Result<zeroize::Zeroizing<[u8; N]>> {
    let mut v = zeroize::Zeroizing::new([0u8; N]);
    getrandom::fill(v.as_mut()).map_err(|_| Error::NoRandom)?;
    Ok(v)
}

/// Are `a` and `b` equal, in time that doesn't depend on where they differ?
#[cfg(feature = "encryption")]
pub(crate) fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without the `encryption` feature nothing encrypts or decrypts, and files are still told apart.
    #[cfg(not(feature = "encryption"))]
    #[test]
    fn without_the_feature_nothing_is_encrypted_or_decrypted() {
        assert_eq!(aescrypt::encrypt(b"x", "pw"), Err(Error::Unavailable));
        assert_eq!(aescrypt::decrypt(b"AES\x03\x00", "pw").map(|_| ()), Err(Error::Unavailable));
        assert_eq!(zipaes::encrypt(b"PK\x05\x06", "pw"), Err(Error::Unavailable));
        assert_eq!(zipaes::decrypt(b"PK\x05\x06", "pw"), Err(Error::Unavailable));
        assert_eq!(detect(b"AES\x03\x00rest"), Kind::AesCrypt(3));
        assert_eq!(detect(b"age-encryption.org/v1\n"), Kind::Age);
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn detects_each_kind() {
        let aes = aescrypt::encrypt_with(b"x", "pw", 2).unwrap();
        assert_eq!(detect(&aes), Kind::AesCrypt(3));
        assert_eq!(detect(b"age-encryption.org/v1\n-> scrypt"), Kind::Age);
        assert_eq!(detect(&[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C, 0, 4]), Kind::SevenZip);
        let plain = zipaes::tests::zip_of(&[("a.md", b"# A")]);
        assert_eq!(detect(&plain), Kind::Zip);
        assert_eq!(detect(&zipaes::encrypt(&plain, "pw").unwrap()), Kind::ZipAes);
        assert_eq!(detect(b"# Title"), Kind::Other);
        assert!(Kind::ZipAes.is_encrypted() && !Kind::Zip.is_encrypted() && !Kind::SevenZip.is_encrypted());
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn messages_never_show_the_password() {
        let secret = "correct horse battery staple";
        let aes = aescrypt::encrypt_with(b"text", secret, 2).unwrap();
        let mut bad = aes.clone();
        if let Some(b) = bad.last_mut() {
            *b ^= 1;
        }
        for e in [aescrypt::decrypt(&aes, "nope").unwrap_err(), aescrypt::decrypt(&bad, secret).unwrap_err()] {
            assert!(!e.to_string().contains(secret) && !format!("{e:?}").contains(secret));
        }
    }
}
