use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use blake2::{Blake2s256, Digest};
use data_encoding::HEXLOWER;
use rand_core::OsRng;
use std::fs;
use std::sync::LazyLock;

static SECRET: LazyLock<String> = LazyLock::new(|| {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/secret");
    fs::read_to_string(path).unwrap_or_else(|_| panic!("Could not find secret file at {}", path))
});

pub fn hash(value: &str, salt: &str, iterations: u32) -> std::vec::Vec<u8> {
    let mut hasher = Blake2s256::new();
    let v = value.as_bytes();
    for _ in 0..iterations {
        hasher.update(v);
    }
    hasher.update(salt);
    hasher.update(SECRET.as_str());
    hasher.finalize().to_vec()
}

pub fn token_hash(token: &str, salt: &str) -> String {
    let hashed_token = hash(token, salt, 2);
    HEXLOWER.encode(&hashed_token)
}

/// Passwords carry far less entropy than the random tokens `hash`/`token_hash` are built for,
/// so they need a memory-hard KDF (Argon2) rather than a handful of Blake2s rounds.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// A fresh, unguessable session token. The raw value is returned to the client once and only
/// its `token_hash` (salted with "account_session") is ever persisted, mirroring how group
/// tokens are stored.
pub fn new_session_token() -> String {
    uuid::Uuid::new_v4().hyphenated().to_string()
}

pub fn session_token_hash(token: &str) -> String {
    token_hash(token, "account_session")
}

#[cfg(test)]
mod password_tests {
    use super::*;

    #[test]
    fn verifies_correct_password() {
        let hash = hash_password("correct-horse-battery-staple").unwrap();
        assert!(verify_password("correct-horse-battery-staple", &hash));
    }

    #[test]
    fn rejects_incorrect_password() {
        let hash = hash_password("correct-horse-battery-staple").unwrap();
        assert!(!verify_password("wrong-password", &hash));
    }

    #[test]
    fn rejects_garbage_hash() {
        assert!(!verify_password("anything", "not-a-real-hash"));
    }

    #[test]
    fn two_hashes_of_the_same_password_differ() {
        let a = hash_password("same-password").unwrap();
        let b = hash_password("same-password").unwrap();
        assert_ne!(a, b, "salts should make each hash unique");
    }

    #[test]
    fn session_tokens_are_unique_and_hash_deterministically() {
        let a = new_session_token();
        let b = new_session_token();
        assert_ne!(a, b);
        assert_eq!(session_token_hash(&a), session_token_hash(&a));
        assert_ne!(session_token_hash(&a), session_token_hash(&b));
    }
}
