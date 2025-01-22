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
use schema::crafter::{CrafterRequest, CrafterResponse, EmailTemplate};

pub struct SmtpPoolResourceProvider;

#[async_trait::async_trait]
impl PoolResourceProvider<SmtpTransport> for SmtpPoolResourceProvider {
    async fn new(&self) -> SmtpTransport {
        let creds = Credentials::new(CONFIGS.smtp.username.clone(), CONFIGS.smtp.password.clone());
        let connection_string = format!("{}", CONFIGS.smtp.host);
        SmtpTransport::relay(connection_string.as_str()).unwrap().credentials(creds).build()
    }
}

#[derive(Clone)]
pub struct SmtpClient {
    pub transport: PoolRequest<SmtpTransport>
}

impl SmtpClient {
    pub async fn send(&self, template: EmailTemplate) -> Resolve<()> {
        let subject = match &template.template {
            Some(Template::Otp(_)) => "OTP verification".to_owned(),
            Some(Template::Welcome(_)) => "Welcome".to_owned(),
            None => "Devlog studio".to_owned()
        };
        let user_email = match &template.template {
            Some(Template::Welcome(u)) => u.user_name.clone(),
            Some(Template::Otp(_)) => {
                todo!()
            }
            None => return Err(Errors::BadRequest("Template.template must be defined".to_owned()))
        };

        let html = self.get_html_template(&template).await?;
        let message = lettre::Message::builder()
            .from(format!("Devlog studio <{}>", CONFIGS.smtp.username).parse().unwrap())
            .to(user_email.clone().parse().unwrap())
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html)
            .unwrap();

        let transport = self.transport.retrieve().await.unwrap();
        transport
            .send(&message)
            .map_err(|e| Errors::FailedToSendEmail(format!("Failed to send email, error={:?}", e)))?;

        Ok(())
    }

    async fn get_html_template(&self, template: &EmailTemplate) -> Resolve<String> {
        let client = reqwest::Client::new();
        let crafter_request = CrafterRequest {
            template: template.clone()
        };

        let mut buffer = Vec::new();
        crafter_request.encode(&mut buffer).expect("Not able to serialize crafter request to a buffer");

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
        let res = client
            .post(format!("{}/email/welcome", CONFIGS.crafter.url))
            .body(buffer)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                Errors::FailedToSendEmail(format!("Failed to generate email template from crafter service {:?}", e))
            })?;

        if res.status() != StatusCode::OK {
            return Err(Errors::FailedToSendEmail(format!(
                "The crafter service response with error {:?}",
                res.error_for_status()
            )))
        }

        let bytes = res.bytes().await.expect("The response must be in binary format");

        let crafter_response = CrafterResponse::decode(bytes).expect("The response must be schema instance");
        info!(
            target: "smtp-client",
            "Received generated content for template={:?} from crafter, html={} chars",
            &template, crafter_response.html.len());
        Ok(crafter_response.html)
    }
}
