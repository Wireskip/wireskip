use crate::asyncrw::AsyncRW;
use crate::payload::{Target, REASON, TARGET, TCP, UDP};
use crate::senderwrite::SenderWrite;
use crate::{error, reaper, state, taskset};
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use log::info;
use std::net::SocketAddr;
use tokio::io::copy_bidirectional;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;

async fn server_handle(
    mut req: Request<Body>,
    st: state::Async,
) -> Result<Response<Body>, error::Box> {
    info!("Handler started");

    let tgt: Target = req
        .headers()
        .get(TARGET)
        .ok_or("missing target header".to_string())?
        .to_str()?
        .parse()?;

    let (send_d, out_d) = Body::channel();

    st.write().await.tasks.spawn(async move {
        let addrs = tgt.socket_addrs(|| None)?;

        match tgt.scheme() {
            TCP => {
                let mut ts = TcpStream::connect(addrs[0]).await?;

                let mut r_half = tokio_util::io::StreamReader::new(req.body_mut().map(|v| {
                    v.map_err(|_e| std::io::Error::new(std::io::ErrorKind::Other, "Splice error!"))
                }));
                let mut w_half = SenderWrite(send_d);

                let mut arw = AsyncRW {
                    r: &mut r_half,
                    w: &mut w_half,
                };

                let mut trailers = HeaderMap::new();
                copy_bidirectional(&mut arw, &mut ts).await?;

                trailers.insert(
                    HeaderName::from_static(REASON),
                    HeaderValue::from_static("todo reason"),
                );

                arw.w.0.send_trailers(trailers).await.unwrap();
            }
            UDP => todo!(),
            &_ => todo!(),
        }
        Ok(())
    });

    info!("Handler finished");
    Ok(Response::builder().status(StatusCode::OK).body(out_d)?)
}

pub async fn spawn(st: state::Async) -> Result<(), error::Box> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let ts = taskset::new_async();

    reaper::spawn(ts);

    let make_service = make_service_fn(|_conn| {
        let st = st.clone();
        async {
            Ok::<_, error::Box>(service_fn(move |req| {
                let st = st.clone();
                server_handle(req, st)
            }))
        }
    });

    let server = Server::bind(&addr)
        .http2_only(true)
        .http2_enable_connect_protocol()
        .serve(make_service);

    info!("server is running");
    server.await?;
    Ok(())
}
