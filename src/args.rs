use clap::{Parser, Subcommand};
use std::net::SocketAddr;

#[derive(Subcommand, Debug)]
pub enum Mode {
    Join,
    Host,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub mode: Mode,

    #[arg(short, long)]
    pub addr: SocketAddr,
}
