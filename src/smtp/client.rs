use lettre::{SmtpTransport, Transport};
use log::info;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::StatusCode;
use schema::crafter::crafter_request::Template;
use schema::crafter::{CrafterRequest, CrafterResponse, WelcomeTemplate};
use prost::Message;
use lettre::transport::smtp::authentication::Credentials;
use schema::misc::datetime::Datetime;
use lettre::message::header::ContentType;
use crate::services::errors::Errors;
use crate::{config::CONFIGS, services::base::Resolve};
use crate::smtp::template::EmailTemplate;

#[derive(Clone)]
pub struct SmtpClient {
    transport: SmtpTransport,
    connection_string: String
}

impl SmtpClient {
    pub fn new() -> Self {
        let creds = Credentials::new(CONFIGS.smtp.username.clone(), CONFIGS.smtp.password.clone());
        let connection_string = format!("{}", CONFIGS.smtp.host);
        let transport = SmtpTransport::relay(connection_string.as_str())
            .unwrap()
            .credentials(creds)
            .build();

        Self {
            transport,
            connection_string
        }
    }

    pub async fn send(&self, template: EmailTemplate) -> Resolve<()> {
        let subject = match &template {
            EmailTemplate::WelcomeNewMember(_) => "Welcome our new members"
        };

        let user = match &template {
            EmailTemplate::WelcomeNewMember(u) => u,
        };

        let html = self.get_html_template(&template).await?;
        let message = lettre::Message::builder()
            .from(format!("Devlog studio <{}>", CONFIGS.smtp.username).parse().unwrap())
            .to(user.email.clone().parse().unwrap())
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html)
            .unwrap();

        self.transport.send(&message).map_err(|e| {
            Errors::FailedToSendEmail(format!("Failed to send email to smtp server={}, error={:?}", self.connection_string, e))
        })?;

        Ok(())
    }

    async fn get_html_template(&self, template: &EmailTemplate) -> Resolve<String> {
        let client = reqwest::Client::new();
        let schema = match template {
            EmailTemplate::WelcomeNewMember(user) => {
                WelcomeTemplate {
                    user_name: match user.name.is_empty() {
                        true => user.email.clone(),
                        false => user.name.clone()
                    },
                    datetime: Some(Datetime::now())
                }
            },
        };

        let crafter_request = CrafterRequest {
            template: Some(Template::Welcome(schema))
        };

        let mut buffer = Vec::new();
        crafter_request.encode(&mut buffer).expect("Not able to serialize crafter request to a buffer");

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream")); // Adjust based on the content type
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
            return Err(Errors::FailedToSendEmail(format!("The crafter service response with error {:?}", res.error_for_status())))
        }

        let bytes = res.bytes().await.expect("The response must be in binary format");

        let crafter_response = CrafterResponse::decode(bytes).expect("The response must be schema instance");
        info!(target: "smtp-client", "Received generated content for template={:?} from crafter, html={} chars", &template, crafter_response.html.len());
        Ok(crafter_response.html)
    }
}
