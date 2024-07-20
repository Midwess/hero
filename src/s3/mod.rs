use aws_sdk_s3::Client;
use aws_config::meta::region::RegionProviderChain;

use crate::{config::CONFIGS, services::base::Resolve};

#[derive(Debug)]
pub struct S3Client {
    client: Client
}

#[derive(Debug)]
pub struct S3Object {
    s3_path: String
}

impl S3Client {
    pub async fn new() -> Self {
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&config);

        Self {
            client
        }
    }

    pub async fn get_objects(&self, path: String) -> Resolve<Vec<S3Object>> {
        let list_objects_resp = self.client.list_objects_v2()
            .bucket(CONFIGS.aws_s3.bucket_name.clone())
            .prefix(path)
            .send()
            .await?;

        let mut result: Vec<S3Object> = vec![];
        for object in list_objects_resp.contents() {
            result.push(S3Object {
                s3_path: object.key().unwrap().to_owned()
            })
        }

        Ok(result)
   }

   pub async fn get_default_avatars_object(&self) -> Resolve<Vec<S3Object>> {
       self.get_objects(CONFIGS.aws_s3.default_avatars_folder_path.clone()).await
   }
}
