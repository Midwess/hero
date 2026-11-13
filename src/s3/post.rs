use super::{Error, ObjectPath, Result, S3Client};
use aws_sdk_s3::config::ProvideCredentials;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use std::{collections::BTreeMap, time::SystemTime};

/// Constraints supplied by the application for one exact-key browser upload.
#[derive(Clone, Debug)]
pub struct PostUploadOptions {
    /// Absolute policy expiration; temporary credentials may expire sooner.
    pub expires_at: SystemTime,
    /// Inclusive minimum object size in bytes, excluding multipart framing.
    pub min_content_length: u64,
    /// Inclusive maximum object size in bytes, excluding multipart framing.
    pub max_content_length: u64,
    /// Optional exact declared content type. This does not validate image bytes.
    pub content_type: Option<String>,
}

/// Authorization for a multipart/form-data POST directly to object storage.
///
/// Send every field unchanged and append the file part last. Treat the fields
/// as bearer authorization until expiration; do not log or persist them.
#[derive(Clone, Serialize, Deserialize)]
pub struct PresignedPost {
    /// Bucket endpoint accepting the multipart POST.
    pub url: String,
    /// Signed form fields, including the exact key, policy, and signature.
    pub fields: BTreeMap<String, String>,
}

impl S3Client {
    /// Signs a POST policy enforcing an exact key and inclusive object-size bounds.
    ///
    /// Endpoint addressing follows the SDK's presigned PUT endpoint, including
    /// custom endpoints and path-style settings. Credentials are resolved for
    /// each call, and session tokens are signed when present. Expiration must be
    /// in the future, within seven days, and no later than credential expiration.
    /// Keys containing `${` are rejected because S3 expands form variables.
    /// Access-point ARNs and endpoints with specialized signing scopes are unsupported.
    /// This authorizes an upload; ownership, byte validation, publication, and
    /// one-time-use semantics remain the application's responsibility.
    pub async fn sign_post_upload(&self, path: &ObjectPath, options: &PostUploadOptions) -> Result<PresignedPost> {
        let now = SystemTime::now();
        let duration = options
            .expires_at
            .duration_since(now)
            .map_err(|_| Error::InvalidRequest("POST expiration must be in the future".into()))?;
        if duration > std::time::Duration::from_secs(604_800)
            || duration.is_zero()
            || options.min_content_length > options.max_content_length
            || path.bucket().is_empty()
            || path.bucket().contains(':')
            || path.key().is_empty()
            || path.key().contains("${")
        {
            return Err(Error::InvalidRequest(
                "invalid POST expiration, size range, bucket, or key".into(),
            ));
        }
        let config = self.raw_client().config();
        let region = config.region().ok_or_else(|| Error::InvalidRequest("POST signing requires a region".into()))?;
        let credentials = self
            .post_credentials
            .as_ref()
            .ok_or_else(|| Error::InvalidRequest("POST signing requires S3Config or with_credentials_provider".into()))?
            .provide_credentials()
            .await
            .map_err(|error| Error::Operation(error.to_string()))?;
        if credentials.expiry().is_some_and(|expiry| options.expires_at > expiry) {
            return Err(Error::InvalidRequest("POST policy outlives its credentials".into()));
        }

        let endpoint_path = ObjectPath::new(path.bucket(), "hero-post-endpoint");
        let mut url = url::Url::parse(&self.sign_upload(&endpoint_path, duration).await?)
            .map_err(|error| Error::InvalidRequest(error.to_string()))?;
        // Resolve addressing through the SDK without sending a storage request.
        // A fixed ASCII key avoids depending on object-key URI encoding.
        let bucket_path = url
            .path()
            .strip_suffix("hero-post-endpoint")
            .ok_or_else(|| Error::InvalidRequest("cannot resolve POST bucket endpoint".into()))?
            .to_owned();
        url.set_path(&bucket_path);
        url.set_query(None);
        url.set_fragment(None);

        let date: DateTime<Utc> = now.into();
        let day = date.format("%Y%m%d").to_string();
        let scope = format!("{day}/{}/s3/aws4_request", region.as_ref());
        let mut fields: BTreeMap<String, String> = BTreeMap::from([
            ("key".into(), path.key().into()),
            ("x-amz-algorithm".into(), "AWS4-HMAC-SHA256".into()),
            ("x-amz-credential".into(), format!("{}/{scope}", credentials.access_key_id())),
            ("x-amz-date".into(), date.format("%Y%m%dT%H%M%SZ").to_string()),
        ]);
        if let Some(token) = credentials.session_token() {
            fields.insert("x-amz-security-token".into(), token.into());
        }
        if let Some(content_type) = &options.content_type {
            fields.insert("Content-Type".into(), content_type.clone());
        }
        let mut conditions = vec![
            json!({"bucket": path.bucket()}),
            json!([
                "content-length-range",
                options.min_content_length,
                options.max_content_length
            ]),
        ];
        conditions.extend(fields.iter().map(|(name, value)| json!({name: value})));
        let expiration: DateTime<Utc> = options.expires_at.into();
        let policy = STANDARD.encode(
            serde_json::to_vec(&json!({
                "expiration": expiration.to_rfc3339_opts(SecondsFormat::Millis, true),
                "conditions": conditions,
            }))
            .map_err(|error| Error::InvalidRequest(error.to_string()))?,
        );
        let date_key = hmac(format!("AWS4{}", credentials.secret_access_key()).as_bytes(), day.as_bytes());
        let region_key = hmac(&date_key, region.as_ref().as_bytes());
        let service_key = hmac(&region_key, b"s3");
        let signing_key = hmac(&service_key, b"aws4_request");
        let signature = hmac(&signing_key, policy.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
        fields.insert("policy".into(), policy);
        fields.insert("x-amz-signature".into(), signature);
        Ok(PresignedPost {
            url: url.into(),
            fields,
        })
    }
}

fn hmac(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(value);
    mac.finalize().into_bytes().to_vec()
}
