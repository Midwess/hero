use crate::config::CONFIGS;
use crate::services::base::Resolve;
use crate::services::errors::Errors;
use crate::utils::pool::allocator::PoolResourceProvider;
use crate::utils::pool::request::PoolRequest;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use log::info;
use schema::value::static_resource::static_resource::Source;
use schema::value::static_resource::StaticResource;
use std::time::Duration;

pub struct S3ClientResourceProvider;

#[async_trait::async_trait]
impl PoolResourceProvider<Client> for S3ClientResourceProvider {
    async fn new(&self) -> Client {
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let config = aws_config::from_env().region(region_provider).load().await;
        Client::new(&config)
    }
}

#[derive(Clone)]
pub struct S3Client {
    pub client: PoolRequest<Client>
}

impl S3Client {
    pub async fn get_objects(&self, prefix: String) -> Resolve<Vec<StaticResource>> {
        let client = self.client.retrieve().await.unwrap();
        let list_objects_resp = client
            .list_objects_v2()
            .bucket(CONFIGS.aws_s3.bucket_name.clone())
            .prefix(prefix.clone())
            .send()
            .await?;
        drop(client);

        let mut result: Vec<StaticResource> = vec![];
        for object in list_objects_resp.contents() {
            let path = object.key().unwrap().to_owned();
            if (&prefix).ne(&path) {
                result.push(StaticResource {
                    source: Some(Source::Path(path))
                })
            }
        }

        Ok(result)
    }

    pub async fn create_object(&self, prefix: String, content: Vec<u8>) -> Resolve<()> {
        log::info!(target: "s3", "Creating object {:?} {}", prefix, content.len());
        let client = self.client.retrieve().await.unwrap();
        client
            .put_object()
            .bucket(CONFIGS.aws_s3.bucket_name.clone())
            .key(prefix)
            .content_length(content.len() as i64)
            .body(ByteStream::from(content))
            .send()
            .await?;

        Ok(())
    }

    pub async fn download(&self, prefix: String) -> Resolve<Option<Vec<u8>>> {
        let client = self.client.retrieve().await.unwrap();

        // Get the object, handle the case where it might not exist
        let download_result = client.get_object().bucket(CONFIGS.aws_s3.bucket_name.clone()).key(prefix).send().await;

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

    pub async fn get_default_avatars_object(&self) -> Resolve<Vec<StaticResource>> {
        let objects = self.get_objects(CONFIGS.aws_s3.default_avatars_folder_path.clone()).await;

        info!(target: "get_default_avatars_object", "got {:?} objects",
           objects.as_ref().map(|it| it.len() as i32).unwrap_or(-1));

        objects
    }

    pub async fn use_cdn(&self, s3_object: &mut StaticResource) -> Resolve<()> {
        let path = match &s3_object.source {
            Some(Source::Path(path)) => path.clone(),
            _ => return Err(Errors::BadRequest("Bad request, s3_object must have path".to_owned()))
        };

        let cdn_url = format!("{}/{}", CONFIGS.aws_s3.cdn_host_url.clone(), path);

        s3_object.source = Some(Source::Url(cdn_url.to_owned()));

        Ok(())
    }

    pub async fn sign_object(&self, s3_object: &mut StaticResource, duration: Duration) -> Resolve<()> {
        let path = match &s3_object.source {
            Some(Source::Path(path)) => path.clone(),
            _ => return Err(Errors::BadRequest("Bad request, s3_object must have path".to_owned()))
        };

        let presigning_config = PresigningConfig::expires_in(duration)?;
        let client = self.client.retrieve().await.unwrap();
        let presigned_req = client
            .get_object()
            .bucket(CONFIGS.aws_s3.bucket_name.clone())
            .key(path)
            .presigned(presigning_config)
            .await?;

        drop(client);

        s3_object.source = Some(Source::Url(presigned_req.uri().to_owned()));

        Ok(())
    }
}
