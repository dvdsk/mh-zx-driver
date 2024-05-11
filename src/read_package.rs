use core::cmp::Ordering;
use defmt::{debug, warn};
use embedded_io_async::{Read, Write};
use heapless::Vec;

use crate::{Error, PAYLOAD_SIZE};

/// reads a whole package, if the start of a next package is already
/// available skip the just read package and finish reading that instead
///
// todo needs unit testing
pub async fn read_package<Tx, Rx>(
    rx: &mut Rx,
) -> Result<[u8; PAYLOAD_SIZE], Error<Tx::Error, Rx::Error>>
where
    Tx: Write,
    Tx::Error: defmt::Format,
    Rx: Read,
    Rx::Error: defmt::Format,
{
    let mut buf = [0u8; 5 * PAYLOAD_SIZE];
    let mut package: Vec<u8, PAYLOAD_SIZE> = Vec::new();
    let mut needed = PAYLOAD_SIZE - package.len();

    loop {
        let n = rx.read(&mut buf).await.map_err(Error::Reading)?;
        let until_package_start = buf.iter().take_while(|byte| **byte != 0xff).count();
        let mut body = &buf[until_package_start..n - until_package_start];

        if n == 0 {
            return Err(Error::ReadingEOF);
        }

        while needed > 0 {
            defmt::info!("body len: {}, needed: {}", body.len(), needed);
            match body.len().cmp(&needed) {
                Ordering::Equal => {
                    package
                        .extend_from_slice(&body[..])
                        .expect("body.len() is the same length as left capacity");
                    return Ok(package
                        .into_array()
                        .expect("just verified package is filled"));
                }
                Ordering::Less => {
                    package
                        .extend_from_slice(&body[..])
                        .expect("body.len() is less then left capacity");
                    needed -= body.len();

                    let n = rx.read(&mut buf).await.map_err(Error::Reading)?;
                    if n == 0 {
                        return Err(Error::ReadingEOF);
                    }
                    body = &buf[..n];
                }
                Ordering::Greater => {
                    debug!("skipping outdated package");
                    package.clear();
                    let newest_starts = body.iter().rev().skip_while(|byte| **byte != 0xff).count();
                    warn!("len before skip: {}", body.len());
                    // no package start in body
                    if newest_starts == body.len() {
                        warn!("no package start found");
                        break;
                    } else {
                        body = &body[newest_starts..];
                    }
                    warn!("len after skip: {}", body.len());
                }
            }
        }
    }
}

#[cfg(all(target_os = "linux", test))]
mod test {
    use super::read_package;
    use core::convert::Infallible;
    use embedded_io_async::{ErrorType, Read, Write};
    use futures::executor::block_on;

    struct MockTx;

    impl ErrorType for MockTx {
        type Error = Infallible;
    }

    impl Write for MockTx {
        async fn write(&mut self, _buf: &[u8]) -> Result<usize, Self::Error> {
            unimplemented!()
        }
    }

    struct MockRx {
        pos: usize,
        line: Vec<u8>,
    }

    impl ErrorType for MockRx {
        type Error = Infallible;
    }

    impl Read for MockRx {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let to_read = self.pos..(self.pos + buf.len()).min(self.line.len());
            buf[to_read.clone()].copy_from_slice(&self.line[to_read.clone()]);
            Ok(to_read.len())
        }
    }

    #[test]
    fn skip_package() {
        let mut rx = MockRx {
            pos: 0,
            line: vec![0xff, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        };
        block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap();
    }
}
