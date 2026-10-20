//! S3-compatible object storage with explicit bucket/key paths.
//!
//! [`S3Config`] supports AWS S3 and compatible services through a custom
//! endpoint and path-style addressing. [`S3Client`] keeps the AWS client
//! clonable, maps missing downloads to `None`, and exposes presigned upload,
//! download, and multipart operations.
//!
//! # Example
//!
//! ```no_run
//! use hero::s3::{ObjectPath, S3Config};
//!
//! # async fn example() -> hero::s3::Result<()> {
//! let client = S3Config::from_env()?.build_client();
//! let path = ObjectPath::new("assets", "avatars/42.png");
//! if let Some(bytes) = client.download(&path).await? {
//!     println!("downloaded {} bytes", bytes.len());
//! }
//! # Ok(())
//! # }
//! ```

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

/// Result type for Hero S3 operations.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
/// Configuration and operation failures from the S3 boundary.
pub enum Error {
    /// Required credentials were absent from the environment.
    #[error("missing required environment variable {0}")]
    MissingEnvironment(&'static str),
    /// A metadata lookup found no object at the requested path.
    #[error("S3 object not found: {0}")]
    NotFound(ObjectPath),
    /// Input could not form a valid S3 or presigning request.
    #[error("invalid S3 request: {0}")]
    InvalidRequest(String),
    /// The AWS SDK or response body reported an operation failure.
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
/// Bucket and key identifying one object or key prefix.
pub struct ObjectPath {
    bucket: String,
    key: String,
}

impl ObjectPath {
    /// Creates a path without normalizing slashes or validating bucket syntax.
    pub fn new(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            key: key.into(),
        }
    }

    /// Returns the bucket name.
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Returns the exact object key or listing prefix.
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
/// Credentials and endpoint settings used to construct an S3 client.
pub struct S3Config {
    /// AWS region name; compatible services commonly use `us-east-1`.
    pub region: String,
    /// Static access-key identifier.
    pub access_key_id: String,
    /// Static secret access key.
    pub secret_access_key: String,
    /// Optional custom endpoint for an S3-compatible service.
    pub endpoint_url: Option<String>,
    /// Whether requests use path-style rather than virtual-host addressing.
    pub force_path_style: bool,
}

impl S3Config {
    /// Loads static S3 settings from conventional AWS environment variables.
    ///
    /// `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` are required.
    /// `AWS_S3_REGION` defaults to `us-east-1`, `AWS_ENDPOINT_URL` is optional,
    /// and `AWS_S3_FORCE_PATH_STYLE` defaults to `true`.
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

    /// Constructs a clonable Hero wrapper around the configured AWS SDK client.
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
/// Client-side description of one presigned multipart upload part.
pub struct UploadPart {
    /// One-based S3 multipart part number.
    pub part_number: i32,
    /// Presigned URL used to upload this part directly.
    pub upload_url: String,
    /// Expected byte length when known by the caller.
    pub content_length: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Upload ID and presigned parts needed by a multipart-upload client.
pub struct MultipartUpload {
    /// S3 upload identifier returned when the upload was created.
    pub upload_id: String,
    /// Parts the client may upload.
    pub parts: Vec<UploadPart>,
}

#[derive(Clone)]
/// Clonable high-level wrapper around [`aws_sdk_s3::Client`].
pub struct S3Client {
    client: Arc<Client>,
}

impl S3Client {
    /// Wraps an already configured AWS SDK client.
    pub fn new(client: Client) -> Self {
        Self {
            client: Arc::new(client),
        }
    }

    /// Exposes the underlying SDK client for operations Hero does not wrap.
    pub fn raw_client(&self) -> &Client {
        &self.client
    }

    /// Lists objects below a key prefix, excluding an object equal to the prefix.
    ///
    /// This currently returns one S3 response page and does not follow
    /// continuation tokens.
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

    /// Uploads an in-memory object with its content length set explicitly.
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

    /// Deletes an object. S3 may report success when the key did not exist.
    pub async fn delete_object(&self, path: &ObjectPath) -> Result<()> {
        self.client.delete_object().bucket(path.bucket()).key(path.key()).send().await?;
        Ok(())
    }

    /// Checks object metadata and maps a missing key to [`Error::NotFound`].
    pub async fn head_object(&self, path: &ObjectPath) -> Result<()> {
        match self.client.head_object().bucket(path.bucket()).key(path.key()).send().await {
            Ok(_) => Ok(()),
            Err(SdkError::ServiceError(error)) if matches!(error.err(), HeadObjectError::NotFound(_)) => {
                Err(Error::NotFound(path.clone()))
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Downloads an entire object into memory.
    ///
    /// Missing keys return `Ok(None)`; other service and body-stream failures
    /// remain errors.
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

    /// Joins a CDN base URL with the object's key, ignoring its bucket.
    pub fn cdn_url(path: &ObjectPath, cdn_base_url: &str) -> String {
        format!("{}/{}", cdn_base_url.trim_end_matches('/'), path.key())
    }

    /// Creates a presigned download URL that requests attachment disposition.
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

    /// Creates a presigned URL for uploading one complete object.
    pub async fn sign_upload(&self, path: &ObjectPath, duration: Duration) -> Result<String> {
        let config = PresigningConfig::expires_in(duration)?;
        let request = self.client.put_object().bucket(path.bucket()).key(path.key()).presigned(config).await?;
        Ok(request.uri().to_string())
    }

    /// Starts a multipart upload and returns its S3 upload ID.
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

    /// Presigns one one-based multipart upload request.
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

    /// Completes a multipart upload from ETags ordered by part number.
    ///
    /// The first ETag is assigned part 1, the second part 2, and so on. Preserve
    /// upload order when collecting ETags from parallel client uploads.
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

    /// Aborts every incomplete upload whose key starts with the supplied prefix.
    ///
    /// This currently processes one listing response and may require another
    /// call when the service paginates a large result set.
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
