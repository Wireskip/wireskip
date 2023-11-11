use crate::{error, tunnel, HostArgs};
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use log::{debug, error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::bytes::Bytes;

type Reply = Result<Response<Empty<Bytes>>, error::Box>;

async fn server_handle(mut req: Request<Incoming>) -> Reply {
    let tgt = req.uri().authority();
    debug!("Handler started for target {tgt:?}");

    let mut ts = TcpStream::connect(tgt.ok_or("missing :authority: (target)")?.to_string()).await?;

    tokio::task::spawn(async move {
        match hyper::upgrade::on(&mut req).await {
            Ok(u) => {
                debug!("Upgraded server side!");
                if let Err(e) = tunnel::join(&mut TokioIo::new(u), &mut ts).await {
                    error!("Connection failed with {e}!");
                }
            }
            Err(e) => error!("Upgrade error: {e}"),
        }
    });
    Ok(Response::new(http_body_util::Empty::new()))
}

pub async fn run(args: HostArgs) -> Result<(), error::Box> {
    info!("Listening for h2c on {}", args.listen);
    let l = TcpListener::bind(args.listen).await?;

    loop {
        let (stream, who) = l.accept().await?;

        tokio::task::spawn(async move {
            if let Err(err) = http2::Builder::new(TokioExecutor::new())
                // .enable_connect_protocol() -- TODO UDP!!
                .serve_connection(TokioIo::new(stream), service_fn(server_handle))
                .await
            {
                info!("Error serving connection from {who}: {err}");
            }
        });
    }
}
