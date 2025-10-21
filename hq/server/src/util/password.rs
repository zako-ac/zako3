use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

use crate::util::error::AppResult;

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);

    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string();

    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parsed_hash = PasswordHash::new(hash)?;

    let argon2 = Argon2::default();
    let r = argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok();

    Ok(r)
}

#[cfg(test)]
mod tests {
    use crate::util::password::{hash_password, verify_password};

    #[test]
    fn password_hash_success() {
        let password0 = "muffin-is-babo";
        let password1 = "muffin-is-babo";

        let hash = hash_password(password0).unwrap();
        assert!(verify_password(password1, &hash).unwrap());
    }

    #[test]
    fn password_hash_fail() {
        let password0 = "muffin-is-babo";
        let password1 = "muffin-is-zako";

        let hash = hash_password(password0).unwrap();
        assert!(!verify_password(password1, &hash).unwrap());
    }
}
