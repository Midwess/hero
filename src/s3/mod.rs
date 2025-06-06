use crate::services::base::Resolve;
use crate::services::errors::Errors;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use log::info;
use schema::value::static_resource::static_resource::Source;
use schema::value::static_resource::StaticResource;
use std::time::Duration;

pub struct S3ClientResourceProvider;

#[derive(Clone)]
pub struct S3Client {
    pub client: Client
}

impl S3Client {
    pub async fn get_objects(&self, bucket_name: String, prefix: String) -> Resolve<Vec<StaticResource>> {
        let list_objects_resp = self.client
            .list_objects_v2()
            .bucket(bucket_name)
            .prefix(prefix.clone())
            .send()
            .await?;

        let mut result: Vec<StaticResource> = vec![];
        for object in list_objects_resp.contents() {
            let path = object.key().unwrap().to_owned();
            if prefix.ne(&path) {
                result.push(StaticResource {
                    source: Some(Source::Path(path))
                })
            }
        }

        Ok(result)
    }

    pub async fn create_object(&self, bucket_name: String, prefix: String, content: Vec<u8>) -> Resolve<()> {
        self.client
            .put_object()
            .bucket(bucket_name)
            .key(prefix)
            .content_length(content.len() as i64)
            .body(ByteStream::from(content))
            .send()
            .await?;

        Ok(())
    }

    pub async fn download(&self, bucket_name: String, prefix: String) -> Resolve<Option<Vec<u8>>> {
        // Get the object, handle the case where it might not exist
        let download_result = self.client.get_object().bucket(bucket_name).key(prefix).send().await;

        match download_result {
            Ok(download_url) => {
                info!(target: "s3", "Downloading file from S3 {:?}", download_url);
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
            Some(Source::Path(path)) => path.clone(),
            _ => return Err(Errors::BadRequest("Bad request, s3_object must have path".to_owned()))
        };

        let cdn_url = format!("{}/{}", cdn_host_url, path);

        s3_object.source = Some(Source::Url(cdn_url.to_owned()));

        Ok(())
    }

    pub async fn sign_object(&self, bucket_name: String, s3_object: &mut StaticResource, duration: Duration) -> Resolve<()> {
        let path = match &s3_object.source {
            Some(Source::Path(path)) => path.clone(),
            _ => return Err(Errors::BadRequest("Bad request, s3_object must have path".to_owned()))
        };

        let presigning_config = PresigningConfig::expires_in(duration)?;
        let presigned_req = self.client
            .get_object()
            .bucket(bucket_name)
            .key(path)
            .presigned(presigning_config)
            .await?;

        s3_object.source = Some(Source::Url(presigned_req.uri().to_owned()));

        Ok(())
    }
}
