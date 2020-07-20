use futures::{prelude::*, task::Poll};
use nb::Error;

/// Turn a non-blocking function (returning
/// [`nb::Result`](../nb/type.Result.html)) into a
/// [`Future`](../futures/future/trait.Future.html).
pub fn nb_fn<T, E, F>(mut f: F) -> impl Future<Output=core::result::Result<T,E>>
where
    F: FnMut() -> nb::Result<T,E>
{
    future::poll_fn(move |ctx| {
        match f() {
            Ok(v) => Poll::Ready(Ok(v)),
            Err(Error::Other(e)) => Poll::Ready(Err(e)),
            Err(Error::WouldBlock) => {
                ctx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::nb_fn;
    use embedded_hal::serial::Read;
    use embedded_hal_mock::serial::{Mock, Transaction};
    use futures::executor::block_on;

    #[test]
    fn future() {
        let mut tx = Mock::new(&[
            Transaction::read_error(nb::Error::WouldBlock),
            Transaction::read_error(nb::Error::WouldBlock),
            Transaction::read_error(nb::Error::WouldBlock),
            Transaction::read_error(nb::Error::WouldBlock),
            Transaction::read_error(nb::Error::WouldBlock),
            Transaction::read_error(nb::Error::WouldBlock),
            Transaction::read_error(nb::Error::WouldBlock),
            Transaction::read_many(&[0x01]),
        ]);

        let f = async {
            let b = nb_fn(|| tx.read()).await.unwrap();
            assert!(b == 0x01);
        };

        block_on(f);
        tx.done();
    }
}
