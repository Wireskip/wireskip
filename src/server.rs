use crate::error::Box;
use crate::proto::{decode_capsule, CONNECT_UDP, UDP_MAX};
use crate::{error, tunnel, HostArgs};
use http::Method;
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::ext::Protocol;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper::upgrade::OnUpgrade;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use log::{debug, error, info};
use std::io;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_util::bytes::Bytes;

type Reply = Result<Response<Empty<Bytes>>, error::Box>;

async fn join_tcp(addr: String, u: OnUpgrade) -> io::Result<()> {
    let mut ts = TcpStream::connect(addr.clone()).await?;

    tokio::task::spawn(async move {
        match u.await {
            Ok(u) => {
                debug!("Upgraded server side for TCP to {addr}");

                if let Err(e) = tunnel::join(&mut TokioIo::new(u), &mut ts).await {
                    error!("TCP connection to {addr} failed with: {e}");
                }
            }
            Err(e) => error!("Upgrade error to {addr}: {e}"),
        }
    });

    Ok(())
}

fn udp_addr_from_path(path: &str) -> Result<String, Box> {
    let mut frags: Vec<_> = path.trim_end_matches('/').rsplitn(3, '/').take(2).collect();

    if frags.len() != 2 {
        Err("malformed URI path fragments; length is not 2")?
    }

    let host = frags.pop().ok_or("missing host")?;
    let port = frags.pop().ok_or("missing port")?;

    if !frags.is_empty() {
        Err("malformed URI path fragments; too many left")?
    }

    Ok(format!("{}:{}", host, port))
}

async fn join_udp(addr: String, u: OnUpgrade) -> io::Result<()> {
    let us = UdpSocket::bind("0.0.0.0:0").await?;
    us.connect(addr.clone()).await?;
    let mut buf = [0u8; UDP_MAX];

    tokio::task::spawn(async move {
        match u.await {
            Ok(u) => {
                debug!("Upgraded server side for UDP!");
                let mut u = TokioIo::new(u);

                loop {
                    match u.read(&mut buf).await {
                        Ok(n) => {
                            if n == 0 {
                                continue;
                            }

                            let b = match decode_capsule(&mut &buf[..n]) {
                                Err(e) => {
                                    debug!("Error when decoding capsule: {e}");
                                    continue;
                                }
                                Ok(b) => b,
                            };

                            if let Err(e) = us.send(&b).await {
                                error!("UDP send to {addr} failed with: {e}");
                            }
                        }
                        Err(e) => error!("UDP read from client failed with: {e}"),
                    }
                }
            }
            Err(e) => error!("Upgrade error to {addr}: {e}"),
        }
    });

    Ok(())
}

async fn server_handle(mut req: Request<Incoming>) -> Reply {
    if req.method() != Method::CONNECT {
        Err("wrong method")?
    }

    if req
        .extensions()
        .get::<Protocol>()
        .is_some_and(|v| *v == CONNECT_UDP)
    {
        // UDP

        let addr = udp_addr_from_path(req.uri().path())?;
        debug!("Tunneling UDP to target {addr}");
        join_udp(addr, hyper::upgrade::on(&mut req)).await?;
    } else {
        // TCP

        let tgt = req
            .uri()
            .authority()
            .ok_or("missing :authority:")?
            .to_string();

        debug!("Tunneling TCP to target {tgt}");
        join_tcp(tgt, hyper::upgrade::on(&mut req)).await?
    };
    Ok(Response::new(http_body_util::Empty::new()))
}

pub async fn run(args: HostArgs) -> Result<(), error::Box> {
    info!("Listening for h2c on {}", args.listen);
    let l = TcpListener::bind(args.listen).await?;

    loop {
        let (stream, who) = l.accept().await?;

        tokio::task::spawn(async move {
            if let Err(err) = http2::Builder::new(TokioExecutor::new())
                .enable_connect_protocol()
                .serve_connection(TokioIo::new(stream), service_fn(server_handle))
                .await
            {
                info!("Error serving connection from {who}: {err}");
            }
        });
    }
}
