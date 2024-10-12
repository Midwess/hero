use crate::config::CONFIGS;
use crate::services::base::Resolve;
use crate::services::errors::Errors;
use crate::smtp::template::EmailTemplate;
use crate::utils::pool::allocator::PoolResourceProvider;
use crate::utils::pool::request::PoolRequest;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};
use log::info;
use prost::Message;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::StatusCode;
use schema::crafter::crafter_request::Template;
use schema::crafter::{CrafterRequest, CrafterResponse, WelcomeTemplate};
use schema::misc::datetime::Datetime;

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
        let subject = match &template {
            EmailTemplate::WelcomeNewMember(_) => "Welcome new member"
        };

        let user = match &template {
            EmailTemplate::WelcomeNewMember(u) => u
        };

        let html = self.get_html_template(&template).await?;
        let message = lettre::Message::builder()
            .from(format!("Devlog studio <{}>", CONFIGS.smtp.username).parse().unwrap())
            .to(user.email.clone().parse().unwrap())
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
        let schema = match template {
            EmailTemplate::WelcomeNewMember(user) => WelcomeTemplate {
                user_name: match user.name.is_empty() {
                    true => user.email.clone(),
                    false => user.name.clone()
                },
                datetime: Some(Datetime::now())
            }
        };

        let crafter_request = CrafterRequest {
            template: Some(Template::Welcome(schema))
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
