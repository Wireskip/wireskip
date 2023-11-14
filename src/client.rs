use crate::socks::Addr;
use crate::{error, socks, tunnel, JoinArgs};
use http::{Method, Request, StatusCode};
use http_body_util::Empty;
use hyper::client::conn::http2::SendRequest;
use hyper::upgrade::{self, Upgraded};
use hyper_util::rt::{TokioExecutor, TokioIo};
use log::{error, info};
use tokio::net::{TcpListener, TcpStream};
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

pub async fn run(args: JoinArgs) -> Result<(), error::Box> {
    // establish circuit
    // first relay is special
    let r = args.hops.first().unwrap(); // clap should ensure we have >= 1 hops
    info!("Connecting to relay 1 at {r}");
    let c_0 = TcpStream::connect(r).await?; // the only conn which isn't an Upgraded
    let mut send = handshake(TokioIo::new(c_0)).await?; // always the latest relay's request sender

    if args.hops.len() > 1 {
        for (n, r) in args.hops[1..].into_iter().enumerate() {
            let n = n + 2;
            info!("Connecting to relay {n} at {r}");

            let c_n = nexthop(&mut send, r.to_string()).await?;
            // TODO:
            // find out a better way to avoid dropping the value
            // but allow to use / drop it later
            std::mem::forget(send);
            // normally mut reassignment drops the previous value
            // but because we just forgot it this does not happen here
            send = handshake(c_n).await?;
        }
    }

    // proxy socks from client
    info!("Listening for socks5 on {}", args.listen);
    let s5l = TcpListener::bind(args.listen).await.unwrap();

    // TODO: asynchronize
    while let Ok((mut l, _)) = s5l.accept().await {
        let addr = match socks::handshake(&mut l).await? {
            Addr::IP(sa) => sa.to_string(),
            Addr::DN((s, p)) => format!("{s}:{p}"),
        };

        let mut target = TokioIo::new(nexthop(&mut send, addr).await?);
        socks::write_ok(&mut l).await?;

        tunnel::join(&mut l, &mut target).await?;
    }

    info!("No more listening for socks5 on {}", args.listen);
    Ok(())
}
