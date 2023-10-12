use crate::taskset::Async;
use log::debug;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

pub fn spawn(ts: Async) {
    // dead task reaper / watcher thread
    let ts = ts.clone();

    tokio::task::spawn(async move {
        loop {
            let mut ct = ts.lock().await;

            match ct.join_next().await {
                Some(Err(e)) => debug! {"Connection terminated abnormally: {e:?}"},
                Some(Ok(_)) => debug! {"Connection terminated normally"},
                None => sleep(Duration::from_secs(1)).await,
            }

            debug!("Taskset has {} references", Arc::strong_count(&ts))
        }
    });
}
