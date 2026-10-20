//! Small validation extensions for owned strings.

use validator::ValidateEmail;

/// Validation helpers implemented for [`String`].
pub trait StringExt {
    /// Checks the string with the `validator` crate's email-address rules.
    ///
    /// This is a syntax check, not proof that the domain or mailbox exists.
    fn is_email(&self) -> bool;
}

impl StringExt for String {
    fn is_email(&self) -> bool {
        self.validate_email()
    }
}
