use crate::state::Async;
use log::debug;
use std::time::Duration;
use tokio::time::sleep;

// dead task reaper / watcher thread
pub fn spawn(st: Async) {
    tokio::task::spawn(async move {
        loop {
            match st.write().await.tasks.join_next().await {
                Some(Err(e)) => debug! {"Connection terminated abnormally: {e:?}"},
                Some(Ok(_)) => debug! {"Connection terminated normally"},
                None => sleep(Duration::from_secs(1)).await,
            }
        }
    });
}
