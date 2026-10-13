use crate::error::{ApiError, ApiResult};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

pub fn token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn id() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

pub fn digest(text: &str) -> Vec<u8> {
    Sha256::digest(text.as_bytes()).to_vec()
}

pub fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

fn password_budget() -> &'static Arc<Semaphore> {
    static BUDGET: OnceLock<Arc<Semaphore>> = OnceLock::new();
    BUDGET.get_or_init(|| Arc::new(Semaphore::new(2)))
}

pub async fn hash_password(password: String) -> ApiResult<String> {
    let permit = password_budget()
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::busy())?;
    let password = Zeroizing::new(password);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| ApiError::unavailable())
    })
    .await
    .map_err(|_| ApiError::unavailable())?
}

pub async fn verify_password(password: String, hash: Option<String>) -> ApiResult<bool> {
    let permit = password_budget()
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::busy())?;
    let password = Zeroizing::new(password);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        match hash {
            Some(hash) => {
                let parsed = PasswordHash::new(&hash).map_err(|_| ApiError::unavailable())?;
                Ok(Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok())
            }
            None => {
                // Matching adaptive work avoids the cheap unknown-account path.
                let salt = SaltString::generate(&mut OsRng);
                let _ = Argon2::default()
                    .hash_password(password.as_bytes(), &salt)
                    .map_err(|_| ApiError::unavailable())?;
                Ok(false)
            }
        }
    })
    .await
    .map_err(|_| ApiError::unavailable())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn passwords_use_salted_argon2_and_reject_wrong_input() {
        let a = hash_password("isolated-test-password".into())
            .await
            .unwrap();
        let b = hash_password("isolated-test-password".into())
            .await
            .unwrap();
        assert!(a.starts_with("$argon2id$"));
        assert_ne!(a, b);
        assert!(
            verify_password("isolated-test-password".into(), Some(a.clone()))
                .await
                .unwrap()
        );
        assert!(!verify_password("wrong-password".into(), Some(a))
            .await
            .unwrap());
        assert!(!verify_password("isolated-test-password".into(), None)
            .await
            .unwrap());
    }
    #[test]
    fn random_tokens_and_constant_comparison() {
        let a = token();
        let b = token();
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        assert!(constant_time_equal(&digest(&a), &digest(&a)));
        assert!(!constant_time_equal(&digest(&a), &digest(&b)));
    }
}
