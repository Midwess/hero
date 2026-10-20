//! HS256 JSON Web Token creation and decoding.
//!
//! # Example
//!
//! ```
//! use hero::token::jwt::{create_jwt_token, decode_jwt_token};
//! use std::time::Duration;
//!
//! let token = create_jwt_token("account-42", "development-secret", Duration::from_secs(60))?;
//! let account: String = decode_jwt_token(&token, "development-secret")?;
//! assert_eq!(account, "account-42");
//! # Ok::<(), hero::token::jwt::JwtErrors>(())
//! ```

/// JWT claims, encoding, and decoding helpers.
pub mod jwt;
