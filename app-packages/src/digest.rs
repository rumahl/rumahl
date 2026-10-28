use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

pub(crate) const SHA256_HEX_LENGTH: usize = 64;

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from_digit(u32::from(byte >> 4), 16).expect("nibble is a hex digit"));
        output.push(char::from_digit(u32::from(byte & 0x0f), 16).expect("nibble is a hex digit"));
    }
    output
}

/// Parses a canonical, lowercase 64-character SHA-256 hex digest.
pub(crate) fn parse_sha256_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != SHA256_HEX_LENGTH {
        return None;
    }

    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        digest[index] = (high << 4) | low;
    }

    Some(digest)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub(crate) fn sha256_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&hasher.finalize());
    Ok(digest)
}

#[cfg(test)]
pub(crate) fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&Sha256::digest(bytes));
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_digest() {
        let value = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(parse_sha256_hex(value).unwrap(), sha256_bytes(b""),);
    }

    #[test]
    fn rejects_non_canonical_digests() {
        assert!(parse_sha256_hex("").is_none());
        assert!(parse_sha256_hex(&"A".repeat(64)).is_none());
        assert!(parse_sha256_hex(&"g".repeat(64)).is_none());
        assert!(parse_sha256_hex(&"a".repeat(63)).is_none());
    }

    #[test]
    fn hashes_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payload");
        std::fs::write(&path, b"hello").unwrap();
        assert_eq!(
            to_hex(&sha256_file(&path).unwrap()),
            to_hex(&sha256_bytes(b"hello")),
        );
    }
}
