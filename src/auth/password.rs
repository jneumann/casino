use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;

use crate::error::ApiError;

/// A valid Argon2 hash of a fixed secret, used to spend the same CPU time on
/// sign-in attempts for accounts that do not exist as for ones that do.
pub fn decoy_hash() -> &'static str {
    static DECOY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DECOY.get_or_init(|| hash_password("decoy-password-never-matches").unwrap_or_default())
}

/// Hashes a password with Argon2id. This is intentionally slow, so callers in
/// request handlers must run it on the blocking pool.
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("failed to hash password: {err}")))
}

/// Verifies a password against a stored PHC hash. A malformed stored hash is
/// treated as a failed match rather than an error.
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(err) => {
            tracing::error!(error = %err, "stored password hash is malformed");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_correct_password() {
        let hash = hash_password("all-in-on-black").expect("hash");

        assert!(verify_password("all-in-on-black", &hash));
        assert!(!verify_password("all-in-on-red", &hash));
    }

    #[test]
    fn salts_each_hash() {
        let first = hash_password("same-password").expect("hash");
        let second = hash_password("same-password").expect("hash");

        assert_ne!(first, second);
    }

    #[test]
    fn treats_a_malformed_hash_as_a_mismatch() {
        assert!(!verify_password("anything", "not-a-phc-string"));
    }
}
