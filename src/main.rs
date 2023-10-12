use semver::Version;
use std::env;

mod args;
mod asyncrw;
mod client;
mod error;
mod payload;
mod reaper;
mod senderwrite;
mod server;
mod state;
mod taskset;

#[tokio::main]
async fn main() -> Result<(), error::Box> {
    env_logger::init();
    let args = args::parse()?;

    let st = state::new_async(state::T {
        addr: args.addr,
        tasks: taskset::T::new(),
        version: Version::parse(env!("CARGO_PKG_VERSION")).unwrap(),
    });

    reaper::spawn(st.clone());

    match args.mode.as_str() {
        "client" => client::spawn(st).await,
        "server" => server::spawn(st).await,
        s => Err(format!("unknown mode: {s}"))?,
    }
}
