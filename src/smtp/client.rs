use log::info;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use schema::crafter::crafter_request::Template;
use schema::crafter::{CrafterRequest, CrafterResponse, WelcomeTemplate};
use prost::Message;
use lettre::transport::smtp::authentication::Credentials;
use schema::misc::datetime::Datetime;
use surrealdb::sql::json;

use crate::services::errors::Errors;
use crate::{config::CONFIGS, services::base::Resolve};
use crate::smtp::template::EmailTemplate;

#[derive(Clone)]
pub struct SmtpClient {
    cred: Credentials
}

impl SmtpClient {
    pub fn new() -> Self {
        Self {
            cred: Credentials::new(CONFIGS.smtp.username.clone(), CONFIGS.smtp.password.clone())
        }
    }

    pub async fn send(&self, template: EmailTemplate) -> Resolve<()> {
        let client = reqwest::Client::new();
        let schema = match template {
            EmailTemplate::WelcomeNewMember(user) => {
                WelcomeTemplate {
                    user_name: match user.name.is_empty() {
                        true => user.email,
                        false => user.name
                    },
                    datetime: Some(Datetime::now())
                }
            },
        };

        let crafter_request = CrafterRequest {
            template: Some(Template::Welcome(schema))
        };

        let mut buffer = Vec::new();
        crafter_request.encode(&mut buffer).map_err(|e| {
            Errors::FailedToSendEmail(format!("Can not encode request {:?}", e))
        })?;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream")); // Adjust based on the content type
        let json = serde_json::to_string(&crafter_request).unwrap();
        let res = client
            .post(format!("{}/email/welcome", CONFIGS.crafter.url))
            .body(buffer)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                Errors::FailedToSendEmail(format!("Failed to generate email template from crafter service {:?}", e))
            })?;

        let bytes = res.bytes().await.map_err(|e| {
            Errors::FailedToSendEmail(format!("Failed to read bytes from crafter service {:?}", e))
        })?;

        let crafter_response = CrafterResponse::decode(bytes).unwrap();
        info!(target: "smtp-client", "Received template from crafter, html={}", crafter_response.html.len());
        Ok(())
    }
}
