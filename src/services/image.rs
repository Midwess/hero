use rand::Rng;

use super::base::{Resolve, Service};


#[derive(Debug)]
pub struct ImageService {}

#[derive(Debug, Clone)]
pub struct RandomImageParams {}

#[derive(Debug, Clone)]
pub struct RandomImageResult {
    url: string
}

impl Service<RandomImageParams, RandomImageResult> for ImageService {
    async fn execute(self, params: RandomImageParams) -> Resolve<RandomImageResult> {
        let random_index = rand::thread_rng().gen_range(1..7);

    }
}
