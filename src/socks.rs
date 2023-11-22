use crate::error::{self, Box};
use std::{fmt::Display, io, net::SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::bytes::{Buf, Bytes};

#[derive(Debug)]
pub enum Addr {
    IP(SocketAddr),
    DN((String, u16)),
}

impl Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Addr::IP(sa) => sa.to_string(),
                Addr::DN((s, p)) => format!("{s}:{p}"),
            }
        )
    }
}

pub fn parse_udp<'a>(b: &mut impl Buf) -> Result<(Addr, Bytes), Box> {
    let _rsv = b.get_u16();
    let frag = b.get_u8();

    if frag != 0 {
        Err("fragmented UDP messages are not supported")?
    }

    let addr_t = b.get_u8();
    let addr_len = match addr_t {
        0x01 | 0x04 => addr_t * 4, // ipv4 or ipv6
        0x03 => b.get_u8(),        // fqdn
        _ => Err("unknown addr type")?,
    };

    let mut addr = vec![0u8; addr_len as usize];
    b.copy_to_slice(&mut addr[..]);
    let port = b.get_u16();

    use Addr::*;
    match addr_t {
        0x01 => {
            let v4: [u8; 4] = addr.as_chunks::<4>().0[0];
            Ok((
                IP(SocketAddr::from((v4, port))),
                b.copy_to_bytes(b.remaining()),
            ))
        }
        0x04 => {
            let v6: [u8; 16] = addr.as_chunks::<16>().0[0];
            Ok((
                IP(SocketAddr::from((v6, port))),
                b.copy_to_bytes(b.remaining()),
            ))
        }
        0x03 => Ok((
            DN((String::from_utf8(addr)?, port)),
            b.copy_to_bytes(b.remaining()),
        )),
        _ => unreachable!(),
    }
}

pub enum Command {
    Connect,
    UdpAssoc,
}

pub async fn handshake<T>(t: &mut T) -> Result<(Command, Addr), error::Box>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let ver = t.read_u8().await?;
    if ver != 0x05 {
        Err("unsupported version")?
    };

    let mut authm = vec![0 as u8; t.read_u8().await? as usize];
    t.read_exact(&mut authm).await?;
    t.write_all(&[0x05, 0x00]).await?;

    // read request
    let ver = t.read_u8().await?;
    if ver != 0x05 {
        Err("TODO")?
    };
    let cmd = match t.read_u8().await? {
        0x01 => Command::Connect,
        0x03 => Command::UdpAssoc,
        _ => Err("unknown cmd")?,
    };
    let _rsv = t.read_u8().await?;
    let addr_t = t.read_u8().await?;
    let addr_len = match addr_t {
        0x01 | 0x04 => addr_t * 4,  // ipv4 or ipv6
        0x03 => t.read_u8().await?, // fqdn
        _ => Err("unknown addr type")?,
    };
    let mut addr = vec![0 as u8; addr_len as usize];
    t.read_exact(&mut addr).await?;
    let port = t.read_u16().await?;

    use Addr::*;
    match addr_t {
        0x01 => {
            let v4: [u8; 4] = addr.as_chunks::<4>().0[0];
            Ok((cmd, IP(SocketAddr::from((v4, port)))))
        }
        0x04 => {
            let v6: [u8; 16] = addr.as_chunks::<16>().0[0];
            Ok((cmd, IP(SocketAddr::from((v6, port)))))
        }
        0x03 => Ok((cmd, DN((String::from_utf8(addr)?, port)))),
        _ => unreachable!(),
    }
}

pub async fn write_ok<T>(t: &mut T) -> io::Result<()>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    t.write_all(&[
        0x05, // SOCKS v5
        0x00, // OK
        0x00, // RSV
        0x01, // IPV4
        127, 0, 0, 1, // IPV4 ADDR
    ])
    .await?;
    t.write_u16(1080).await?;
    Ok(())
}
