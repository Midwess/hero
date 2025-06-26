pub trait DbId: Sized {
    fn soft_deleted(&self) -> bool;
    fn soft_delete(&mut self);
    fn soft_restore(&mut self);
}
