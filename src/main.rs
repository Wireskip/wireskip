#![feature(slice_as_chunks)]

mod args;
mod client;
mod error;
mod server;
mod socks;
mod tunnel;

#[tokio::main]
async fn main() -> Result<(), error::Box> {
    env_logger::init();
    let args = args::parse()?;

    match args.mode.as_str() {
        "join" => client::run().await,
        "host" => server::run().await,
        s => Err(format!("unknown mode: {s}"))?,
    }
}
