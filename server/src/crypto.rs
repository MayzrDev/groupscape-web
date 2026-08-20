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

/// A fresh, unguessable plugin API key - the RuneLite plugin's only credential. Mirrors
/// `new_session_token`: the raw value is returned to the client once (shown once on the account
/// page) and only its `api_key_hash` (salted with "account_api_key") is ever persisted.
pub fn new_api_key() -> String {
    uuid::Uuid::new_v4().hyphenated().to_string()
}

pub fn api_key_hash(key: &str) -> String {
    token_hash(key, "account_api_key")
}

/// A signed, self-verifying OAuth `state` value: `nonce.expiry.signature`. No server-side
/// storage needed to validate it on callback, which matters here because the account API is
/// stateless bearer-token auth (no session store to stash a pending-OAuth nonce in between the
/// redirect to Discord and the callback).
const OAUTH_STATE_TTL_SECONDS: i64 = 600;

pub fn new_oauth_state() -> String {
    let nonce = uuid::Uuid::new_v4().hyphenated().to_string();
    let expires_at = chrono::Utc::now().timestamp() + OAUTH_STATE_TTL_SECONDS;
    let payload = format!("{}.{}", nonce, expires_at);
    let sig = token_hash(&payload, "oauth_state");
    format!("{}.{}", payload, sig)
}

pub fn verify_oauth_state(state: &str) -> bool {
    let mut parts = state.splitn(3, '.');
    let (Some(nonce), Some(expires_at), Some(sig)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let payload = format!("{}.{}", nonce, expires_at);
    let expected_sig = token_hash(&payload, "oauth_state");
    if sig != expected_sig {
        return false;
    }
    match expires_at.parse::<i64>() {
        Ok(expires_at) => chrono::Utc::now().timestamp() < expires_at,
        Err(_) => false,
    }
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

    #[test]
    fn api_keys_are_unique_and_hash_deterministically() {
        let a = new_api_key();
        let b = new_api_key();
        assert_ne!(a, b);
        assert_eq!(api_key_hash(&a), api_key_hash(&a));
        assert_ne!(api_key_hash(&a), api_key_hash(&b));
    }

    #[test]
    fn oauth_state_round_trips() {
        let state = new_oauth_state();
        assert!(verify_oauth_state(&state));
    }

    #[test]
    fn oauth_states_are_unique() {
        assert_ne!(new_oauth_state(), new_oauth_state());
    }

    #[test]
    fn oauth_state_rejects_tampered_signature() {
        let mut state = new_oauth_state();
        state.push('x');
        assert!(!verify_oauth_state(&state));
    }

    #[test]
    fn oauth_state_rejects_tampered_expiry() {
        let state = new_oauth_state();
        let mut parts: Vec<&str> = state.splitn(3, '.').collect();
        let tampered_expiry = (chrono::Utc::now().timestamp() + 999_999).to_string();
        parts[1] = &tampered_expiry;
        let tampered = parts.join(".");
        assert!(!verify_oauth_state(&tampered));
    }

    #[test]
    fn oauth_state_rejects_garbage() {
        assert!(!verify_oauth_state("not-a-real-state"));
        assert!(!verify_oauth_state(""));
    }

    #[test]
    fn oauth_state_rejects_expired() {
        let nonce = uuid::Uuid::new_v4().hyphenated().to_string();
        let expired_at = chrono::Utc::now().timestamp() - 1;
        let payload = format!("{}.{}", nonce, expired_at);
        let sig = token_hash(&payload, "oauth_state");
        let state = format!("{}.{}", payload, sig);
        assert!(!verify_oauth_state(&state));
    }
}
