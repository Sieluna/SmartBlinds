use std::sync::Arc;

use argon2::password_hash::{SaltString, rand_core};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash};

use crate::models::User;

#[derive(Debug, Clone)]
pub struct Argon2Hash(Argon2<'static>);

#[derive(Clone)]
pub struct AuthService {
    hasher: Arc<Argon2Hash>,
}

impl AuthService {
    pub fn new() -> Self {
        let hash = Argon2Hash(Argon2::default());

        Self {
            hasher: Arc::new(hash),
        }
    }

    pub fn hash(&self, password: &str) -> Result<String, password_hash::Error> {
        let hash_salt = SaltString::generate(&mut rand_core::OsRng);
        let hash = self.hasher.0.hash_password(password.as_ref(), &hash_salt)?;

        Ok(hash.to_string())
    }

    pub fn verify(&self, user: &User, password: &str) -> Result<bool, password_hash::Error> {
        let parsed_hash = PasswordHash::new(&user.password).unwrap();

        Ok(self
            .hasher
            .0
            .verify_password(password.as_ref(), &parsed_hash)
            .is_ok())
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::models::User;

    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let auth_service = AuthService::new();
        let password = "test";

        let hash = auth_service.hash(password).unwrap();

        assert!(hash.starts_with("$argon2"));

        let user = User {
            id: 0,
            email: "test@test.com".to_string(),
            password: hash,
            role: "user".to_string(),
        };

        let result = auth_service.verify(&user, password).unwrap();

        assert!(result);
    }
}
