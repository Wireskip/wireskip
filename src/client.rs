use crate::error::Box;
use crate::proto::{CONNECT_UDP, UDP_MAX};
use crate::{error, socks, tunnel, JoinArgs};
use http::{Method, Request, StatusCode};
use http_body_util::Empty;
use hyper::client::conn::http2::SendRequest;
use hyper::upgrade::{self, Upgraded};
use hyper_util::rt::{TokioExecutor, TokioIo};
use log::{debug, error, info};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tokio::task;
use tokio_util::bytes::Bytes;

type Sender = SendRequest<Empty<Bytes>>;

async fn handshake<T>(c: T) -> Result<Sender, error::Box>
where
    T: hyper::rt::Read + hyper::rt::Write + Send + Unpin + 'static,
{
    let (send, h2c) = hyper::client::conn::http2::handshake(TokioExecutor::new(), c).await?;
    // poll h2 future in the background
    tokio::task::spawn(async move {
        if let Err(err) = h2c.await {
            error!("polling h2c to relay failed: {err}");
        }
    });
    Ok(send)
}

async fn nexthop(this: &mut Sender, next: String) -> Result<Upgraded, error::Box> {
    let req = Request::builder()
        .uri(next)
        .method(Method::CONNECT)
        .body(Empty::<Bytes>::new())?;

    let res = this.send_request(req).await?;
    let status = res.status();

    if status != StatusCode::OK {
        error!("Server refused upgrade with {status}");
        Err("server refused upgrade")?
    }

    Ok(upgrade::on(res).await?)
}

async fn nexthop_udp(
    this: Arc<Mutex<SendRequest<Empty<Bytes>>>>,
    this_host: String,
    next: String,
) -> Result<Upgraded, error::Box> {
    let mut this = this.lock().await;
    let next = next.replace(":", "/");
    let next_uri = format!("https://{this_host}/.well-known/masque/udp/{next}/");

    let req = Request::builder()
        .uri(next_uri)
        .method(Method::CONNECT)
        .extension(CONNECT_UDP)
        .header("capsule-protocol", "?1")
        .body(Empty::<Bytes>::new())?;

    let res = this.send_request(req).await?;
    let status = res.status();

    if status != StatusCode::OK {
        error!("Server refused upgrade with {status}");
        Err("server refused upgrade")?
    }

    Ok(upgrade::on(res).await?)
}

async fn s5_join(
    c0: &mut TcpStream,
    send: Arc<Mutex<SendRequest<Empty<Bytes>>>>,
) -> Result<(), Box> {
    let mut send = send.lock().await;
    let (_cmd, addr) = socks::handshake(c0).await?;
    let mut target = TokioIo::new(nexthop(&mut send, addr.to_string()).await?);
    socks::write_ok(c0).await?;
    tunnel::join(c0, &mut target).await?;
    Ok(())
}

pub async fn run(args: JoinArgs) -> Result<(), error::Box> {
    // establish circuit
    // first relay is special
    // clap should ensure we have >= 1 hops
    let r = args.hops.first().unwrap();
    info!("Connecting to relay 1 at {r}");
    // c_0 is the only conn which isn't an Upgraded
    let c_0 = TcpStream::connect(r).await?;
    // last is the last relay's hostname.
    let mut last = String::new();
    // send is always the latest relay's request sender (innermost onion layer)
    let mut send = handshake(TokioIo::new(c_0)).await?;
    // send_prev are the previous hops' request senders (so they are not dropped)
    let mut send_prev = Vec::new();

    if args.hops.len() > 1 {
        for (n, r) in args.hops[1..].into_iter().enumerate() {
            let n = n + 2;
            info!("Connecting to relay {n} at {r}");

            last = r.to_string();
            let c_n = nexthop(&mut send, r.to_string()).await?;
            // this *moves* current send to send_prev
            send_prev.push(send);
            // normally mut reassignment drops the previous value but it was just moved already
            send = handshake(c_n).await?;
        }
    }

    // proxy socks from client
    info!("Listening for SOCKSv5 connections on {}", args.listen);
    let s5l = TcpListener::bind(args.listen).await.unwrap();
    let send_n = Arc::new(Mutex::new(send));
    let local = send_n.clone();

    task::spawn(async move {
        while let Ok((mut c0, peer)) = s5l.accept().await {
            let send = local.clone();

            task::spawn(async move {
                match s5_join(&mut c0, send).await {
                    Ok(()) => debug!("SOCKSv5 session from {peer} closed OK"),
                    Err(e) => debug!("SOCKSv5 session from {peer} failed: {e}"),
                }
            });
        }
    });

    // TODO unhardcode?
    let mut udp_addr = args.listen.clone();
    udp_addr.set_port(udp_addr.port() + 1);

    let udp = UdpSocket::bind(&udp_addr).await?;

    info!("Listening for SOCKSv5 UDP on {udp_addr}");

    let mut buf = [0u8; UDP_MAX];

    loop {
        let (sz, peer) = udp.recv_from(&mut buf).await?;
        debug!("Received SOCKSv5 UDP packet of size {sz} from {peer}");

        if sz > UDP_MAX {
            debug!("Oversize UDP packet (size {sz} > {UDP_MAX}) from {peer}, discarding");
            continue;
        }

        let (addr, packet) = socks::parse_udp(&mut &buf[..sz])?;
        let u = nexthop_udp(send_n.clone(), last.clone(), addr.to_string()).await?;
        let mut u = TokioIo::new(u);
        u.write_all(&packet).await?
    }
}
