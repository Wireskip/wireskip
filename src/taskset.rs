use std::sync::Arc;
use tokio::task::JoinSet;

type SyncContainer<T> = tokio::sync::Mutex<T>;

pub type T = JoinSet<Result<(), crate::error::Box>>;
pub type Async = Arc<SyncContainer<T>>;

pub fn new_async() -> Async {
    Arc::new(SyncContainer::new(T::new()))
}
