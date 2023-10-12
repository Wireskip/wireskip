use std::{
    env,
    net::{SocketAddr, ToSocketAddrs},
};

use crate::error;

pub struct Args {
    pub mode: String,
    pub addr: SocketAddr,
}

pub fn parse() -> Result<Args, error::Box> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        return Err(
            "Usage: wireskip <client|server> <target relay address|listening address>".into(),
        );
    }

    let mode = args[1].to_string();
    let addr = args[2].to_socket_addrs()?.next().ok_or("zzz")?;
    Ok(Args { mode, addr })
}
