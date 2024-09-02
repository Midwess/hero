use crate::config::CONFIGS;
use crate::services::base::Resolve;
use crate::services::errors::Errors;
use crate::utils::pool::allocator::PoolResource;
use crate::utils::pool::request::PoolRequest;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use log::info;
use schema::misc::s3::s3_object::Content;
use schema::misc::s3::S3Object;
use std::time::Duration;

#[async_trait::async_trait]
impl PoolResource<()> for Client {
    async fn new(_param: &()) -> Self {
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let config = aws_config::from_env().region(region_provider).load().await;
        Client::new(&config)
    }
}

#[derive(Clone)]
pub struct S3Client {
    pub client: PoolRequest<Client, ()>
}

impl S3Client {
    pub async fn get_objects(&self, prefix: String) -> Resolve<Vec<S3Object>> {
        let client = self.client.retreive().await.unwrap();
        let list_objects_resp = client
            .list_objects_v2()
            .bucket(CONFIGS.aws_s3.bucket_name.clone())
            .prefix(prefix.clone())
            .send()
            .await?;
        drop(client);

        let mut result: Vec<S3Object> = vec![];
        for object in list_objects_resp.contents() {
            let path = object.key().unwrap().to_owned();
            if (&prefix).ne(&path) {
                result.push(S3Object {
                    content: Some(Content::Path(path))
                })
            }
        }

        Ok(result)
    }

    pub async fn get_default_avatars_object(&self) -> Resolve<Vec<S3Object>> {
        let objects = self.get_objects(CONFIGS.aws_s3.default_avatars_folder_path.clone()).await;

        info!(target: "get_default_avatars_object", "got {:?} objects",
           objects.as_ref().map(|it| it.len() as i32).unwrap_or(-1));

        objects
    }

    pub async fn use_cdn(&self, s3_object: &mut S3Object) -> Resolve<()> {
        let path = match &s3_object.content {
            Some(Content::Path(path)) => path.clone(),
            _ => return Err(Errors::BadRequest("Bad request, s3_object must have path".to_owned()))
        };

        let cdn_url = format!("{}/{}", CONFIGS.aws_s3.cdn_host_url.clone(), path);

        s3_object.content = Some(Content::DownloadUrl(cdn_url.to_owned()));

        Ok(())
    }

    pub async fn sign_object(&self, s3_object: &mut S3Object, duration: Duration) -> Resolve<()> {
        let path = match &s3_object.content {
            Some(Content::Path(path)) => path.clone(),
            _ => return Err(Errors::BadRequest("Bad request, s3_object must have path".to_owned()))
        };

        let presigning_config = PresigningConfig::expires_in(duration)?;
        let client = self.client.retreive().await.unwrap();
        let presigned_req = client
            .get_object()
            .bucket(CONFIGS.aws_s3.bucket_name.clone())
            .key(path)
            .presigned(presigning_config)
            .await?;

        drop(client);

        s3_object.content = Some(Content::DownloadUrl(presigned_req.uri().to_owned()));

        Ok(())
    }
}
