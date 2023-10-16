use tokio::io::{self, AsyncRead, AsyncWrite};

pub async fn join<L, R>(l: &mut L, r: &mut R) -> io::Result<()>
where
    L: AsyncRead + AsyncWrite + Unpin + ?Sized,
    R: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let (lr, rl) = io::copy_bidirectional(l, r).await?;
    log::debug!("l->r {lr}, r->l {rl} bytes wireskipped");
    Ok(())
}
