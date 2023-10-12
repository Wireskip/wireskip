use crate::payload::TARGET;
use crate::{error, state};
use http::{Method, Request};
use log::debug;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_util::bytes::Bytes;

pub async fn spawn(st: state::Async) -> Result<(), error::Box> {
    let mut st = st.write().await;
    let target = st.addr.clone();

    let req = Request::builder()
        .uri(format!("https://{}", &st.addr))
        .method(Method::PUT)
        .header(TARGET, "tcp://localhost:8081")
        .body(())?;

    st.tasks.spawn(async move {
        sleep(Duration::from_secs(1)).await;
        let stream = TcpStream::connect(target).await?;
        let (send_req, conn) = h2::client::handshake(stream).await?;

        tokio::spawn(async move {
            conn.await.unwrap();
        });

        let mut send_req = send_req.ready().await?;
        let (res, mut send) = send_req.send_request(req, false)?;

        tokio::spawn(async move {
            let mut stdin = tokio::io::stdin();
            let mut buffer = [0; 128];

            loop {
                let n = stdin.read(&mut buffer[..]).await.unwrap();
                send.send_data(Bytes::copy_from_slice(&buffer[..n]), false)
                    .unwrap();
            }
        });

        let (head, mut body) = res.await?.into_parts();
        debug!("Got HTTP/2 response {head:?}");

        let mut flow_control = body.flow_control().clone();

        while let Some(chunk) = body.data().await {
            let chunk = chunk.unwrap();
            debug!("RX: {chunk:?}");
            let _ = flow_control.release_capacity(chunk.len());
        }

        if let Ok(trs) = body.trailers().await {
            debug!("trailers were: {trs:?}");
        };
        Ok(())
    });
    Ok(())
}
