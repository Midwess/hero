//! HTML email delivery using rustls-backed asynchronous SMTP.

use crate::cancellation::{CancellationToken, FutureExtension, TaskErrors};
use lettre::address::AddressError;
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Result type for email construction and delivery.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
/// Failure while building or sending an email.
pub enum Error {
    /// Sender or recipient syntax was invalid.
    #[error("invalid email address: {0}")]
    Address(#[from] AddressError),
    /// Lettre could not build the MIME message.
    #[error("failed to construct email: {0}")]
    Message(#[from] lettre::error::Error),
    /// SMTP connection or delivery failed.
    #[error("SMTP transport failed: {0}")]
    Transport(#[from] lettre::transport::smtp::Error),
    /// Delivery exceeded the configured timeout.
    #[error("SMTP operation cancelled")]
    Cancelled(#[from] TaskErrors),
}

#[derive(Clone)]
/// Reusable authenticated SMTP transport for rendered HTML messages.
pub struct SmtpClient {
    transport: Arc<AsyncSmtpTransport<Tokio1Executor>>,
    timeout: Duration,
}

impl SmtpClient {
    /// Creates a rustls-backed relay transport with a 20-second delivery timeout.
    pub fn new(user_name: &str, password: &str, host: &str, port: u16) -> Result<Self> {
        let credentials = Credentials::new(user_name.to_owned(), password.to_owned());
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(host)?.port(port).credentials(credentials).build();

        Ok(Self {
            transport: Arc::new(transport),
            timeout: Duration::from_secs(20),
        })
    }

    /// Replaces the deadline applied independently to each delivery.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sends an HTML-only multipart message.
    ///
    /// `from` and `recipient` accept Lettre mailbox syntax, including display
    /// names. Dropping the returned future drops the in-flight send; reaching
    /// the configured deadline returns [`Error::Cancelled`].
    pub async fn send_html(
        &self,
        subject: impl Into<String>,
        html: impl Into<String>,
        from: impl AsRef<str>,
        recipient: impl AsRef<str>,
    ) -> Result<()> {
        let from = parse_mailbox(from.as_ref())?;
        let recipient = parse_mailbox(recipient.as_ref())?;
        let message = lettre::Message::builder().from(from).to(recipient).subject(subject.into()).multipart(
            MultiPart::alternative().singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(html.into())),
        )?;

        self.transport.send(message).with_cancel(&CancellationToken::timeout(self.timeout)).await??;
        Ok(())
    }
}

fn parse_mailbox(value: &str) -> Result<Mailbox> {
    value.parse().map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_mailbox_is_reported() {
        let result = parse_mailbox("not an email address");
        assert!(result.is_err());
    }
}
