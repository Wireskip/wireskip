#![feature(slice_as_chunks)]

use clap::{Args, Parser, Subcommand};
use std::net::SocketAddr;

mod client;
mod error;
mod server;
mod socks;
mod tunnel;

#[derive(Args, Debug)]
struct JoinArgs {
    /// Address to accept SOCKSv5 requests on.
    #[arg(
        short = 'L',
        long,
        default_value_t = SocketAddr::from(([127, 0, 0, 1], 1080))
    )]
    listen: SocketAddr,

    /// Arbitrary number of relay hops to onion-route through.
    hops: Vec<SocketAddr>,
}

#[derive(Args, Debug)]
struct HostArgs {
    /// Address to accept Wireskip client requests on.
    listen: SocketAddr,
}

#[derive(Subcommand, Debug)]
enum Mode {
    /// Join Wireskip by routing through relays.
    Join(JoinArgs),
    /// Host a relay which can be used to `join`.
    Host(HostArgs),
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct MainArgs {
    #[command(subcommand)]
    mode: Mode,
}

#[tokio::main]
async fn main() -> Result<(), error::Box> {
    env_logger::init();
    let args = MainArgs::parse();

    match args.mode {
        Mode::Join(opts) => client::run(opts).await,
        Mode::Host(opts) => server::run(opts).await,
    }
}
