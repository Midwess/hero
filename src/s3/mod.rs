use crate::services::base::Resolve;
use crate::services::errors::Errors;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use schema::value::static_resource::static_resource::Source;
use schema::value::static_resource::{S3Path, StaticResource};
use std::sync::Arc;
use std::time::Duration;

use aws_smithy_async::rt::sleep::{AsyncSleep, Sleep};

#[derive(Clone, Debug)]
pub struct S3SleepAsync {}

impl Default for S3SleepAsync {
    fn default() -> Self {
        Self::new()
    }
}

impl S3SleepAsync {
    pub fn new() -> Self {
        Self {}
    }
}

impl AsyncSleep for S3SleepAsync {
    fn sleep(&self, duration: Duration) -> Sleep {
        Sleep::new(tokio::time::sleep(duration))
    }
}

pub struct S3ClientResourceProvider;

#[derive(Clone)]
pub struct S3Client {
    pub client: Arc<Client>
}

impl S3Client {
    pub async fn get_objects(&self, path: S3Path) -> Resolve<Vec<StaticResource>> {
        let list_objects_resp =
            self.client.list_objects_v2().bucket(path.bucket()).prefix(path.prefix()).send().await?;

        let mut result: Vec<StaticResource> = vec![];
        for object in list_objects_resp.contents() {
            let prefix = object.key().unwrap().to_owned();
            // Ignoring it self
            if path.prefix().ne(&prefix) {
                result.push(StaticResource {
                    source: Some(Source::S3Path(S3Path::new(path.bucket().to_owned(), prefix)))
                })
            }
        }

        Ok(result)
    }

    pub async fn create_object(&self, path: S3Path, content: Vec<u8>) -> Resolve<()> {
        self.client
            .put_object()
            .bucket(path.bucket())
            .key(path.prefix())
            .content_length(content.len() as i64)
            .body(ByteStream::from(content))
            .send()
            .await?;

        Ok(())
    }

    pub async fn download(&self, path: S3Path) -> Resolve<Option<Vec<u8>>> {
        // Get the object, handle the case where it might not exist
        let download_result = self.client.get_object().bucket(path.bucket()).key(path.prefix()).send().await;

        match download_result {
            Ok(download_url) => {
                let body = match download_url.body.collect().await {
                    Ok(body) => body,
                    Err(_) => return Ok(None)
                };
                Ok(Some(body.to_vec()))
            }
            Err(_) => Ok(None)
        }
    }

    pub async fn use_cdn(&self, s3_object: &mut StaticResource, cdn_host_url: String) -> Resolve<()> {
        let path = match &s3_object.source {
            Some(Source::S3Path(path)) => path.clone(),
            _ => return Err(Errors::BadRequest("Bad request, static resource must be a s3 path".to_owned()))
        };

        let cdn_url = format!("{cdn_host_url}/{}", path.prefix());

        s3_object.source = Some(Source::Url(cdn_url.to_owned()));

        Ok(())
    }

    pub async fn sign_download(&self, s3_object: &mut StaticResource, duration: Duration) -> Resolve<String> {
        let path = match &s3_object.source {
            Some(Source::S3Path(path)) => path.clone(),
            Some(Source::Url(url)) => return Ok(url.clone()),
            _ => return Err(Errors::BadRequest("Bad request, static resource must be a s3 path".to_owned()))
        };

        let presigning_config = PresigningConfig::expires_in(duration)?;
        let presigned_req = self
            .client
            .get_object()
            .bucket(path.bucket())
            .key(path.prefix())
            .presigned(presigning_config)
            .await?;

        let url = presigned_req.uri().to_owned();
        s3_object.source = Some(Source::Url(url.clone()));

        Ok(url)
    }

    pub async fn sign_upload(&self, s3_object: &mut StaticResource, duration: Duration) -> Resolve<String> {
        let path = match &s3_object.source {
            Some(Source::S3Path(path)) => path.clone(),
            Some(Source::Url(url)) => return Ok(url.clone()),
            _ => return Err(Errors::BadRequest("Bad request, static resource must be a s3 path".to_owned()))
        };

        let presigning_config = PresigningConfig::expires_in(duration)?;
        let presigned_req = self
            .client
            .put_object()
            .bucket(path.bucket())
            .key(path.prefix())
            .presigned(presigning_config)
            .await?;

        let url = presigned_req.uri().to_owned();
        s3_object.source = Some(Source::Url(url.clone()));

        Ok(url)
    }
}
