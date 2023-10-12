use hyper::body::Sender;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::AsyncWrite;

pub struct SenderWrite(pub Sender);

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
