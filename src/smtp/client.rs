use crate::config::CONFIGS;
use crate::services::base::Resolve;
use crate::services::errors::Errors;
use crate::utils::pool::allocator::PoolResourceProvider;
use crate::utils::pool::request::PoolRequest;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};
use log::info;
use prost::Message;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::StatusCode;
use schema::crafter::email_template::Template;
use schema::crafter::EmailTemplate;

pub struct SmtpPoolResourceProvider;

#[async_trait::async_trait]
impl PoolResourceProvider<SmtpTransport> for SmtpPoolResourceProvider {
    async fn new(&self) -> SmtpTransport {
        let creds = Credentials::new(CONFIGS.smtp.username.clone(), CONFIGS.smtp.password.clone());
        let connection_string = CONFIGS.smtp.host.to_string();
        SmtpTransport::relay(&connection_string).unwrap().credentials(creds).build()
    }
}

#[derive(Clone)]
pub struct SmtpClient {
    pub transport: PoolRequest<SmtpTransport>,
    pub crafter_url: String
}

impl SmtpClient {
    pub async fn send(&self, template: EmailTemplate, receiver: String) -> Resolve<()> {
        let subject = match &template.template {
            Some(Template::Otp(_)) => "OTP verification".to_owned(),
            Some(Template::Welcome(_)) => "Welcome to Westrise".to_owned(),
            Some(Template::SendFile(_)) => "Files received".to_owned(),
            None => "Westrise".to_owned()
        };

        let html = self.get_html_template(&template).await?;
        let message = lettre::Message::builder()
            .from(format!("Westrise <{}>", CONFIGS.smtp.username).parse().unwrap())
            .to(receiver.parse().unwrap())
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html)
            .unwrap();

        let transport = self.transport.retrieve().await.unwrap();
        transport
            .send(&message)
            .map_err(|e| Errors::FailedToSendEmail(format!("Failed to send email, error={e:?}")))?;

        Ok(())
    }

    async fn get_html_template(&self, template: &EmailTemplate) -> Resolve<String> {
        let client = reqwest::Client::new();
        let mut buffer = Vec::new();
        template.encode(&mut buffer).expect("Not able to serialize crafter request to a buffer");

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
        let res = client
            .post(format!("{}/email", self.crafter_url))
            .body(buffer)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                Errors::FailedToSendEmail(format!("Failed to generate email template from crafter service {e:?}"))
            })?;

        if res.status() != StatusCode::OK {
            return Err(Errors::FailedToSendEmail(format!(
                "The crafter service response with error {:?}",
                res.error_for_status()
            )))
        }

        let bytes = res.bytes().await.expect("The response must be in binary format");
        let crafter_response = String::from_utf8(bytes.to_vec())
            .map_err(|e| Errors::FailedToSendEmail(format!("Failed to parse crafter response as UTF-8: {e}")))?;

        info!(
            target: "smtp-client",
            "Received generated content for template={:?} from crafter, html={} chars",
            &template, crafter_response.len());
        Ok(crafter_response)
    }
}
