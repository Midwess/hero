pub mod finally;
pub mod pool;
pub mod pool_allocator;

use rand::Rng;

pub fn random_number_in_range(min: u32, max: u32) -> u32 {
    let mut rng = rand::rng();
    rng.random_range(min..=max)
}
