use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::head_object::HeadObjectError;
use aws_sdk_s3::presigning::{PresigningConfig, PresigningConfigError};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing required environment variable {0}")]
    MissingEnvironment(&'static str),
    #[error("S3 object not found: {0}")]
    NotFound(ObjectPath),
    #[error("invalid S3 request: {0}")]
    InvalidRequest(String),
    #[error("S3 operation failed: {0}")]
    Operation(String),
}

impl<E, R> From<SdkError<E, R>> for Error
where
    E: std::fmt::Debug,
    R: std::fmt::Debug,
{
    fn from(error: SdkError<E, R>) -> Self {
        Self::Operation(format!("{error:?}"))
    }
}

impl From<PresigningConfigError> for Error {
    fn from(error: PresigningConfigError) -> Self {
        Self::InvalidRequest(error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectPath {
    bucket: String,
    key: String,
}

impl ObjectPath {
    pub fn new(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            key: key.into(),
        }
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl std::fmt::Display for ObjectPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "s3://{}/{}", self.bucket, self.key)
    }
}

#[derive(Clone, Debug)]
pub struct S3Config {
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub endpoint_url: Option<String>,
    pub force_path_style: bool,
}

impl S3Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            region: std::env::var("AWS_S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned()),
            access_key_id: required_env("AWS_ACCESS_KEY_ID")?,
            secret_access_key: required_env("AWS_SECRET_ACCESS_KEY")?,
            endpoint_url: std::env::var("AWS_ENDPOINT_URL").ok().filter(|value| !value.is_empty()),
            force_path_style: std::env::var("AWS_S3_FORCE_PATH_STYLE")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(true),
        })
    }

    pub fn build_client(&self) -> S3Client {
        let credentials =
            Credentials::new(self.access_key_id.clone(), self.secret_access_key.clone(), None, None, "hero");

        let mut builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(self.region.clone()))
            .credentials_provider(credentials)
            .force_path_style(self.force_path_style);

        if let Some(endpoint_url) = &self.endpoint_url {
            builder = builder.endpoint_url(endpoint_url);
        }

        S3Client::new(Client::from_conf(builder.build()))
    }
}

fn required_env(name: &'static str) -> Result<String> {
    std::env::var(name).map_err(|_| Error::MissingEnvironment(name))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadPart {
    pub part_number: i32,
    pub upload_url: String,
    pub content_length: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultipartUpload {
    pub upload_id: String,
    pub parts: Vec<UploadPart>,
}

#[derive(Clone)]
pub struct S3Client {
    client: Arc<Client>,
}

impl S3Client {
    pub fn new(client: Client) -> Self {
        Self {
            client: Arc::new(client),
        }
    }

    pub fn raw_client(&self) -> &Client {
        &self.client
    }

    pub async fn list_objects(&self, path: &ObjectPath) -> Result<Vec<ObjectPath>> {
        let response = self.client.list_objects_v2().bucket(path.bucket()).prefix(path.key()).send().await?;

        Ok(response
            .contents()
            .iter()
            .filter_map(|object| object.key())
            .filter(|key| *key != path.key())
            .map(|key| ObjectPath::new(path.bucket(), key))
            .collect())
    }

    pub async fn create_object(&self, path: &ObjectPath, content: Vec<u8>) -> Result<()> {
        self.client
            .put_object()
            .bucket(path.bucket())
            .key(path.key())
            .content_length(content.len() as i64)
            .body(ByteStream::from(content))
            .send()
            .await?;
        Ok(())
    }

    pub async fn delete_object(&self, path: &ObjectPath) -> Result<()> {
        self.client.delete_object().bucket(path.bucket()).key(path.key()).send().await?;
        Ok(())
    }

    pub async fn head_object(&self, path: &ObjectPath) -> Result<()> {
        match self.client.head_object().bucket(path.bucket()).key(path.key()).send().await {
            Ok(_) => Ok(()),
            Err(SdkError::ServiceError(error)) if matches!(error.err(), HeadObjectError::NotFound(_)) => {
                Err(Error::NotFound(path.clone()))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub async fn download(&self, path: &ObjectPath) -> Result<Option<Vec<u8>>> {
        let response = match self.client.get_object().bucket(path.bucket()).key(path.key()).send().await {
            Ok(response) => response,
            Err(SdkError::ServiceError(error)) if error.raw().status().as_u16() == 404 => return Ok(None),
            Err(error) => return Err(error.into()),
        };

        response
            .body
            .collect()
            .await
            .map(|body| Some(body.to_vec()))
            .map_err(|error| Error::Operation(error.to_string()))
    }

    pub fn cdn_url(path: &ObjectPath, cdn_base_url: &str) -> String {
        format!("{}/{}", cdn_base_url.trim_end_matches('/'), path.key())
    }

    pub async fn sign_download(&self, path: &ObjectPath, duration: Duration) -> Result<String> {
        let config = PresigningConfig::expires_in(duration)?;
        let request = self
            .client
            .get_object()
            .bucket(path.bucket())
            .key(path.key())
            .response_content_disposition("attachment")
            .presigned(config)
            .await?;
        Ok(request.uri().to_string())
    }

    pub async fn sign_upload(&self, path: &ObjectPath, duration: Duration) -> Result<String> {
        let config = PresigningConfig::expires_in(duration)?;
        let request = self.client.put_object().bucket(path.bucket()).key(path.key()).presigned(config).await?;
        Ok(request.uri().to_string())
    }

    pub async fn create_multipart_upload(&self, path: &ObjectPath) -> Result<String> {
        self.client
            .create_multipart_upload()
            .bucket(path.bucket())
            .key(path.key())
            .send()
            .await?
            .upload_id
            .ok_or_else(|| Error::Operation("S3 did not return a multipart upload ID".to_owned()))
    }

    pub async fn generate_part_upload_url(
        &self,
        path: &ObjectPath,
        upload_id: &str,
        part_number: i32,
        duration: Duration,
    ) -> Result<String> {
        if part_number < 1 {
            return Err(Error::InvalidRequest("part_number must be at least 1".to_owned()));
        }

        let config = PresigningConfig::expires_in(duration)?;
        let request = self
            .client
            .upload_part()
            .bucket(path.bucket())
            .key(path.key())
            .upload_id(upload_id)
            .part_number(part_number)
            .presigned(config)
            .await?;
        Ok(request.uri().to_string())
    }

    pub async fn complete_multipart_upload(
        &self,
        path: &ObjectPath,
        upload_id: impl Into<String>,
        e_tags: Vec<String>,
    ) -> Result<()> {
        let completed_parts = e_tags
            .into_iter()
            .enumerate()
            .map(|(index, e_tag)| CompletedPart::builder().e_tag(e_tag).part_number((index + 1) as i32).build())
            .collect::<Vec<_>>();

        self.client
            .complete_multipart_upload()
            .bucket(path.bucket())
            .key(path.key())
            .upload_id(upload_id)
            .multipart_upload(CompletedMultipartUpload::builder().set_parts(Some(completed_parts)).build())
            .send()
            .await?;
        Ok(())
    }

    pub async fn abort_incomplete_multipart_uploads(&self, path: &ObjectPath) -> Result<()> {
        let response = self.client.list_multipart_uploads().bucket(path.bucket()).prefix(path.key()).send().await?;

        for upload in response.uploads() {
            if let (Some(key), Some(upload_id)) = (upload.key(), upload.upload_id()) {
                self.client
                    .abort_multipart_upload()
                    .bucket(path.bucket())
                    .key(key)
                    .upload_id(upload_id)
                    .send()
                    .await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_path_and_cdn_url_are_stable() {
        let path = ObjectPath::new("assets", "images/avatar.png");
        assert_eq!(path.to_string(), "s3://assets/images/avatar.png");
        assert_eq!(
            S3Client::cdn_url(&path, "https://cdn.example.com/"),
            "https://cdn.example.com/images/avatar.png"
        );
    }
}
