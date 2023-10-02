use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use hyper::body::Sender;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use log::info;
use once_cell::sync::Lazy;
use semver::Version;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;
use std::{env, sync::Arc};
use tokio::io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::task::{self, JoinSet};
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tokio_util::bytes::Bytes;

// version of this binary
static _VERSION: Lazy<Version> = Lazy::new(|| Version::parse(env!("CARGO_PKG_VERSION")).unwrap());

type BoxError = Box<dyn std::error::Error + Send + Sync>;

type State = Arc<RwLock<JoinSet<Result<(), BoxError>>>>;

#[derive(serde::Deserialize)]
struct Payload {
    command: String,
    protocol: String,
    remote: String,
    version: Version,
}

struct AsyncRW<'r, 'w, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    r: &'r mut R,
    w: &'w mut W,
}

impl<R, W> AsyncRead for AsyncRW<'_, '_, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.r).poll_read(cx, buf)
    }
}

impl<R, W> AsyncWrite for AsyncRW<'_, '_, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.w).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.w).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.w).poll_shutdown(cx)
    }
}

struct SenderWrite(pub Sender);

impl AsyncWrite for SenderWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        ready!(self
            .0
            .poll_ready(cx)
            .map_err(|e| Error::new(ErrorKind::Other, e))?);

        match self.0.try_send_data(Box::<[u8]>::from(buf).into()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(_) => Poll::Ready(Err(Error::new(ErrorKind::Other, "Body closed"))),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let res = self.0.poll_ready(cx);
        res.map_err(|e| Error::new(ErrorKind::Other, e))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.poll_flush(cx)
    }
}

async fn handle(mut req: Request<Body>, st: State) -> Result<Response<Body>, BoxError> {
    info!("Handler started");

    let p: Payload = match req.headers().get("x-wireskip-payload") {
        Some(s) => serde_json::from_str(s.to_str()?)?,
        None => return Err("missing headers".to_string().into()),
    };

    assert!(p.command == "CONNECT");
    _ = p.protocol;
    _ = p.version;

    let (send_d, out_d) = Body::channel();

    st.write().await.spawn(async move {
        let mut ts = TcpStream::connect(p.remote).await?;

        let mut r_half = tokio_util::io::StreamReader::new(req.body_mut().map(|v| {
            v.map_err(|_e| std::io::Error::new(std::io::ErrorKind::Other, "Splice error!"))
        }));
        let mut w_half = SenderWrite(send_d);

        let mut arw = AsyncRW {
            r: &mut r_half,
            w: &mut w_half,
        };

        copy_bidirectional(&mut arw, &mut ts).await?;

        let mut trailers = HeaderMap::new();
        trailers.insert(
            HeaderName::from_static("x-wireskip-status"),
            HeaderValue::from_static("{code: 200, desc: \"OK\"}"),
        );
        arw.w.0.send_trailers(trailers).await.unwrap();
        Ok(())
    });

    info!("Handler finished");
    Ok(Response::builder().status(StatusCode::OK).body(out_d)?)
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    env_logger::init();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let conntasks = Arc::new(RwLock::new(JoinSet::<Result<(), BoxError>>::new()));
    let ct_local = conntasks.clone();

    // dead task reaper thread
    task::spawn(async move {
        loop {
            let mut ct = ct_local.write().await;

            match ct.join_next().await {
                Some(Err(e)) => info! {"Connection terminated abnormally: {e:?}"},
                Some(Ok(_)) => info! {"Connection terminated normally"},
                None => sleep(Duration::from_secs(1)).await,
            }
        }
    });

    let make_service = make_service_fn(|_conn| {
        let ct = conntasks.clone();
        async {
            Ok::<_, BoxError>(service_fn(move |req| {
                let st = ct.clone();
                handle(req, st)
            }))
        }
    });

    let server = Server::bind(&addr)
        .http2_only(true)
        .http2_enable_connect_protocol()
        .serve(make_service);

    // simple client test
    let req = Request::builder()
        .uri(format!("https://{}", addr))
        .method(Method::PUT)
        .header("x-wireskip-payload", r#"{"command": "CONNECT", "protocol": "tcp", "remote": "localhost:8081", "version": "0.1.0" }"#)
        .body(())?;

    task::spawn(async move {
        sleep(Duration::from_secs(1)).await;
        let stream = TcpStream::connect(&addr).await.unwrap();
        let (send_req, conn) = h2::client::handshake(stream).await.unwrap();

        tokio::spawn(async move {
            conn.await.unwrap();
        });

        let mut send_req = send_req.ready().await.unwrap();
        let (res, mut send) = send_req.send_request(req, false).unwrap();

        tokio::spawn(async move {
            let mut stdin = tokio::io::stdin();
            let mut buffer = [0; 128];

            loop {
                let n = stdin.read(&mut buffer[..]).await.unwrap();
                send.send_data(Bytes::copy_from_slice(&buffer[..n]), false)
                    .unwrap();
            }
        });

        let (head, mut body) = res.await.unwrap().into_parts();
        info!("Got HTTP/2 response {head:?}");

        let mut flow_control = body.flow_control().clone();

        while let Some(chunk) = body.data().await {
            let chunk = chunk.unwrap();
            println!("RX: {chunk:?}");
            let _ = flow_control.release_capacity(chunk.len());
        }

        if let Ok(trs) = body.trailers().await {
            println!("trailers were: {trs:?}");
        }
    });
    // client test end

    info!("server is running");
    server.await?;
    Ok(())
}
