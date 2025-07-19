use std::ops::{Deref, DerefMut};

pub struct NeverSend<T>(pub T);

impl<T> Clone for NeverSend<T> where T: Clone {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

unsafe impl<T> Send for NeverSend<T> {}

unsafe impl<T> Sync for NeverSend<T> {}

impl<T> Deref for NeverSend<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for NeverSend<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
