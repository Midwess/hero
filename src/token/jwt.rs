//! Compact HS256 tokens that wrap application-defined serializable data.

use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Standard timestamps and application payload encoded by Hero JWT helpers.
pub struct Claims<T> {
    exp: usize,
    iat: usize,
    data: T,
}

#[derive(Debug, Error)]
/// JWT configuration or coding failure.
pub enum JwtErrors {
    /// Token encoding, parsing, signature, or expiration validation failed.
    #[error("JWT coding error: {0}")]
    CodingError(#[from] jsonwebtoken::errors::Error),
    /// HS256 cannot be used with an empty shared secret.
    #[error("Invalid secret")]
    InvalidSecret,
}

/// Creates an HS256 token with `iat` set to now and `exp` set from `expiration`.
///
/// Payloads should avoid secrets unless the token is additionally encrypted;
/// JWT payloads are signed but remain readable by holders.
pub fn create_jwt_token<T>(data: T, secret: &str, expiration: Duration) -> Result<String, JwtErrors>
where
    T: Clone + Serialize,
{
    if secret.is_empty() {
        return Err(JwtErrors::InvalidSecret);
    }

    let header = Header::new(Algorithm::HS256);
    let encoding_key = EncodingKey::from_secret(secret.as_ref());

    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        exp: (Utc::now() + expiration).timestamp() as usize,
        iat: now,
        data,
    };

    let token = encode(&header, &claims, &encoding_key)?;
    Ok(token)
}

/// Verifies an HS256 signature and expiration before returning the payload.
pub fn decode_jwt_token<T>(token: &str, secret: &str) -> Result<T, JwtErrors>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    if secret.is_empty() {
        return Err(JwtErrors::InvalidSecret);
    }

    let decoding_key = DecodingKey::from_secret(secret.as_ref());
    let validation = Validation::new(Algorithm::HS256);

    let token_data = decode::<Claims<T>>(token, &decoding_key, &validation)?;
    Ok(token_data.claims.data)
}

/// Reads a token payload without checking its signature or expiration.
///
/// Use this only for non-authoritative inspection, such as displaying metadata
/// before a trusted service performs validation. Never make authorization
/// decisions from this result.
pub fn decode_jwt_token_without_validation<T>(token: &str) -> Result<T, JwtErrors>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    let mut validation = Validation::new(Algorithm::HS256);
    #[allow(deprecated)]
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;

    let decoding_key = DecodingKey::from_secret(&[]);

    let token_data = decode::<Claims<T>>(token, &decoding_key, &validation)?;
    Ok(token_data.claims.data)
}
