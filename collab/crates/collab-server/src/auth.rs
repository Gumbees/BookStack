use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenClaims {
    pub user_id: i64,
    pub user_name: String,
    pub page_id: i64,
    pub exp: u64,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidToken(String),
    ExpiredToken,
    MissingSecret,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidToken(msg) => write!(f, "invalid token: {msg}"),
            AuthError::ExpiredToken => write!(f, "token expired"),
            AuthError::MissingSecret => write!(f, "jwt secret not configured"),
        }
    }
}

pub fn verify_token(token: &str, secret: &str) -> Result<TokenClaims, AuthError> {
    if secret.is_empty() {
        return Err(AuthError::MissingSecret);
    }

    let key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    decode::<TokenClaims>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
            _ => AuthError::InvalidToken(e.to_string()),
        })
}
