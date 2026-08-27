//! Password hashing.
//!
//! Argon2id at the `argon2` crate's defaults, which are the OWASP-recommended
//! parameters (m = 19 MiB, t = 2, p = 1). Hashing is deliberately expensive, so every
//! call site must run it off the async runtime and behind rate limiting.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use std::sync::OnceLock;

/// Argon2's recommended salt length; anything shorter weakens the per-password salt.
const SALT_BYTES: usize = 16;

/// Entropy per session token and per decoy password, in bytes.
const TOKEN_BYTES: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("could not hash password: {0}")]
    Hash(String),
}

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::encode_b64(&random_bytes::<SALT_BYTES>())
        .map_err(|e| PasswordError::Hash(e.to_string()))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| PasswordError::Hash(e.to_string()))
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// A hash of random data, verified against when the email is unknown.
///
/// Without it, "no such user" returns in microseconds while "wrong password" takes ~50 ms,
/// and that gap tells an attacker which email addresses have accounts. Verifying against
/// this instead makes the two paths cost the same.
pub fn decoy_hash() -> &'static str {
    static DECOY: OnceLock<String> = OnceLock::new();
    DECOY.get_or_init(|| {
        hash_password(&to_hex(&random_bytes::<TOKEN_BYTES>()))
            .expect("hashing random bytes cannot fail")
    })
}

pub fn random_token() -> String {
    to_hex(&random_bytes::<TOKEN_BYTES>())
}

/// Entropy straight from the OS. A failure here means the system CSPRNG is broken,
/// which is not a condition this process can sensibly continue through.
fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("OS randomness unavailable");
    bytes
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_against_its_own_hash() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
    }

    #[test]
    fn a_wrong_password_does_not_verify() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(!verify_password("Correct horse battery staple", &hash));
        assert!(!verify_password("", &hash));
    }

    #[test]
    fn the_same_password_hashes_differently_each_time() {
        let a = hash_password("same").unwrap();
        let b = hash_password("same").unwrap();
        assert_ne!(a, b, "a per-password salt must make the hashes differ");
        assert!(verify_password("same", &a) && verify_password("same", &b));
    }

    #[test]
    fn garbage_in_the_stored_field_fails_closed() {
        assert!(!verify_password("anything", "not-a-phc-string"));
        assert!(!verify_password("anything", ""));
    }

    #[test]
    fn the_decoy_hash_is_a_real_verifiable_hash_that_nothing_matches() {
        let decoy = decoy_hash();
        assert!(
            PasswordHash::new(decoy).is_ok(),
            "must be well-formed or it would fail fast"
        );
        assert!(!verify_password("", decoy));
        assert!(!verify_password("password", decoy));
    }

    #[test]
    fn hashing_uses_argon2id() {
        let hash = hash_password("x").unwrap();
        assert!(hash.starts_with("$argon2id$"), "got {hash}");
    }

    #[test]
    fn random_tokens_do_not_repeat() {
        let a = random_token();
        let b = random_token();
        assert_ne!(a, b);
        assert_eq!(
            a.len(),
            TOKEN_BYTES * 2,
            "hex encoding doubles the byte count"
        );
    }
}
