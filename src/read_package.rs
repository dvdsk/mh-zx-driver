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
        if n == 0 {
            return Err(Error::ReadingEOF);
        }

        let package_start = buf.iter().rev().skip_while(|byte| **byte != 0xff).count();
        let offset = if package_start == 0 {
            continue;
        } else {
            package_start - 1
        };

        // this we know contains a body
        let mut body = &buf[offset..n];
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
                    needed = PAYLOAD_SIZE;
                    // limit search to new packages at the end of the body
                    body = &body[body.len().saturating_sub(PAYLOAD_SIZE)..];
                    let newest_starts = body.iter().rev().skip_while(|byte| **byte != 0xff).count();
                    // no package start in body
                    if newest_starts == 0 {
                        break;
                    } else {
                        body = &body[newest_starts - 1..];
                    }
                }
            }
        } // break out of this
    } // into this
}

#[cfg(all(target_os = "linux", test))]
mod test {
    use crate::Error;

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
        curr_read: usize,
        reads: &'static [&'static [u8]],
    }

    impl ErrorType for MockRx {
        type Error = Infallible;
    }

    impl Read for MockRx {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let Some(to_read) = self.reads.get(self.curr_read) else {
                return Ok(0); //eof
            };

            assert!(
                to_read.len() <= buf.len(),
                "the mockrx only supports making up to the read buffer of data available each read"
            );
            buf[..to_read.len()].copy_from_slice(&to_read[..]);
            self.curr_read += 1;
            Ok(to_read.len())
        }
    }

    #[cfg(test)]
    mod more_bytes_then_package {
        // even though we have a full package available we
        // should ignore it since we got more then the package
        // lengths of data. That usually means this package is old
        // and should be ignore
        use super::*;

        #[test]
        fn reject_one_with_trailing_data() {
            let mut rx = MockRx {
                curr_read: 0,
                reads: &[&[255, 2, 3, 4, 5, 6, 7, 8, 9, 10]],
            };
            let eof_err = block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap_err();
            assert_eq!(eof_err, Error::ReadingEOF)
        }

        #[test]
        fn reject_two_reads_with_trailing_data() {
            let mut rx = MockRx {
                curr_read: 0,
                reads: &[&[255, 2, 3, 4, 5, 6, 7, 8, 9, 10], &[11, 12, 13]],
            };
            let eof_err = block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap_err();
            assert_eq!(eof_err, Error::ReadingEOF)
        }

        #[test]
        fn two_packages_accept_last() {
            let mut rx = MockRx {
                curr_read: 0,
                reads: &[
                    &[255, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                    &[1, 2, 3, 255, 12, 13, 14], // new package starts
                    &[15, 16, 17, 18, 19],       // package ends without newer data available
                ],
            };
            let package = block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap();
            assert_eq!(package, [255, 12, 13, 14, 15, 16, 17, 18, 19])
        }

        #[test]
        fn three_packages_accept_last() {
            let mut rx = MockRx {
                curr_read: 0,
                reads: &[
                    &[255, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                    &[255, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                    &[255, 12, 13, 14, 15, 16, 17, 18, 19],
                ],
            };
            let package = block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap();
            assert_eq!(package, [255, 12, 13, 14, 15, 16, 17, 18, 19])
        }
    }

    mod not_enough_bytes {
        use super::*;

        mod followed_by_too_much {
            use super::*;

            #[test]
            fn read_second_package() {
                let mut rx = MockRx {
                    curr_read: 0,
                    reads: &[
                        &[255, 2, 3, 4],
                        &[5, 6, 7, 8, 9, 10], // element 10 is unexpected and too much
                        &[255, 12, 13, 14, 15, 16, 17, 18, 19], // a whole package again
                    ],
                };
                let package = block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap();
                assert_eq!(package, [255, 12, 13, 14, 15, 16, 17, 18, 19])
            }
        }

        #[test]
        fn eof() {
            let mut rx = MockRx {
                curr_read: 0,
                reads: &[&[255, 2, 3, 4], &[5, 6]],
            };
            let err = block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap_err();
            assert_eq!(err, Error::ReadingEOF)
        }
    }

    mod huge_read {
        use super::*;

        #[test]
        fn accept_last_package() {
            let mut rx = MockRx {
                curr_read: 0,
                reads: &[&[
                    255, 2, 3, 4, 5, 6, 7, 8, 9, 10, 255, 22, 23, 24, 25, 26, 27, 28, 29, 255, 2,
                    3, 4, 5, 6, 7, 8, 9, 10, 255, 12, 13, 14, 15, 16, 17, 18, 19,
                ]],
            };
            let package = block_on(read_package::<MockTx, MockRx>(&mut rx)).unwrap();
            assert_eq!(package, [255, 12, 13, 14, 15, 16, 17, 18, 19])
        }
    }
}
