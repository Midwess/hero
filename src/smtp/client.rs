use crate::cancellation::{CancellationToken, FutureExtension, TaskErrors};
use lettre::address::AddressError;
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid email address: {0}")]
    Address(#[from] AddressError),
    #[error("failed to construct email: {0}")]
    Message(#[from] lettre::error::Error),
    #[error("SMTP transport failed: {0}")]
    Transport(#[from] lettre::transport::smtp::Error),
    #[error("SMTP operation cancelled")]
    Cancelled(#[from] TaskErrors),
}

#[derive(Clone)]
pub struct SmtpClient {
    transport: Arc<AsyncSmtpTransport<Tokio1Executor>>,
    timeout: Duration,
}

impl SmtpClient {
    pub fn new(user_name: &str, password: &str, host: &str, port: u16) -> Result<Self> {
        let credentials = Credentials::new(user_name.to_owned(), password.to_owned());
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(host)?.port(port).credentials(credentials).build();

        Ok(Self {
            transport: Arc::new(transport),
            timeout: Duration::from_secs(20),
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

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
