use crate::socks::AddrType;
use crate::{error, socks, tunnel, JoinArgs};
use http::{Method, Request, StatusCode};
use http_body_util::Empty;
use hyper::upgrade;
use hyper_util::rt::{TokioExecutor, TokioIo};
use log::{debug, error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_util::bytes::Bytes;

pub async fn run(args: JoinArgs) -> Result<(), error::Box> {
    // socks from client
    info!("Listening for socks5 on {}", args.listen);
    let s5l = TcpListener::bind(args.listen).await.unwrap();

    // tcp to relay
    let relay = SocketAddr::from(([127, 0, 0, 1], 8080));
    info!("Connecting to relay at {relay}");
    let relay_io = TokioIo::new(TcpStream::connect(relay).await?);
    // h2 to relay
    let (send, h2c) = hyper::client::conn::http2::handshake(TokioExecutor::new(), relay_io).await?;

    tokio::task::spawn(async move {
        if let Err(err) = h2c.await {
            error!("h2c to relay failed: {err}");
        }
    });

    let send = Arc::new(Mutex::new(send));

    while let Ok((mut l, _)) = s5l.accept().await {
        let addr = socks::handshake(&mut l).await.unwrap();
        let send = send.clone();

        tokio::task::spawn(async move {
            let addr = match addr {
                AddrType::IP(sa) => sa.to_string(),
                AddrType::DN((s, p)) => format!("{s}:{p}"),
            };

            let req = Request::builder()
                .uri(addr)
                .method(Method::CONNECT)
                .body(Empty::<Bytes>::new())?;

            let res = send.lock().await.send_request(req).await?;
            let status = res.status();

            if status != StatusCode::OK {
                error!("Server refused upgrade with {status}");
                Err("server refused upgrade")?
            }

            let mut r = TokioIo::new(upgrade::on(res).await?);
            debug!("Upgraded client side!");
            socks::write_ok(&mut l).await?;
            tunnel::join(&mut l, &mut r).await?;
            Ok::<(), error::Box>(())
        });
    }
    info!("No more listening for socks5 on :1080");
    Ok(())
}
