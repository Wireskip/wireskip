use crate::taskset;
use semver::Version;
use std::{net::SocketAddr, sync::Arc};

type SyncContainer<T> = tokio::sync::RwLock<T>;

pub struct T {
    pub addr: SocketAddr,
    pub tasks: taskset::T,
    #[allow(dead_code)]
    pub version: Version,
}

pub type Async = Arc<SyncContainer<T>>;

pub fn new_async(t: T) -> Async {
    Arc::new(SyncContainer::new(t))
}
