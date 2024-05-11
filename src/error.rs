use core::fmt;

#[derive(Debug)]
#[cfg_attr(feature = "thiserror", derive(thiserror::Error))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(defmt::Format)]
pub enum Error<TxError, RxError>
where
    TxError: defmt::Format + fmt::Debug,
    RxError: defmt::Format + fmt::Debug,
{
    #[cfg_attr(
        feature = "thiserror",
        error("The sensor send back a packet however it is corrupt")
    )]
    InvalidChecksum,
    #[cfg_attr(
        feature = "thiserror",
        error("Response header is not correct for the made request")
    )]
    InvalidPacket,
    #[cfg_attr(feature = "thiserror", error("Writing data to sensor failed: {0}"))]
    WritingToUart(TxError),
    #[cfg_attr(feature = "thiserror", error("Flushing data to sensor failed: {0}"))]
    FlushingUart(TxError),
    #[cfg_attr(
        feature = "thiserror",
        error("Unexpected EOF while reading from sensor")
    )]
    ReadingEOF,
    #[cfg_attr(feature = "thiserror", error("Could not read from sensor: {0}"))]
    Reading(RxError),
}

impl<TxError, RxError> Clone for Error<TxError, RxError>
where
    TxError: defmt::Format + fmt::Debug + Clone,
    RxError: defmt::Format + fmt::Debug + Clone,
{
    fn clone(&self) -> Self {
        match self {
            Error::InvalidChecksum => Error::InvalidChecksum,
            Error::InvalidPacket => Error::InvalidPacket,
            Error::WritingToUart(e) => Error::WritingToUart(e.clone()),
            Error::FlushingUart(e) => Error::FlushingUart(e.clone()),
            Error::ReadingEOF => Error::ReadingEOF,
            Error::Reading(e) => Error::Reading(e.clone()),
        }
    }
}

impl<TxError, RxError> Eq for Error<TxError, RxError>
where
    TxError: defmt::Format + fmt::Debug + Eq,
    RxError: defmt::Format + fmt::Debug + Eq,
{
}

impl<TxError, RxError> PartialEq for Error<TxError, RxError>
where
    TxError: defmt::Format + fmt::Debug + PartialEq,
    RxError: defmt::Format + fmt::Debug + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Error::ReadingEOF, Error::ReadingEOF)
            | (Error::InvalidChecksum, Error::InvalidChecksum)
            | (Error::InvalidPacket, Error::InvalidPacket) => true,
            (Error::WritingToUart(e), Error::WritingToUart(e2))
            | (Error::FlushingUart(e), Error::FlushingUart(e2)) => e == e2,
            (Error::Reading(e), Error::Reading(e2)) => e == e2,
            (_, _) => false,
        }
    }
}

/// very ugly, still needed unfortunately
/// const cmp tracking issue: https://github.com/rust-lang/rust/issues/92391
/// workaround credits: https://stackoverflow.com/questions/53619695/
/// calculating-maximum-value-of-a-set-of-constant-expressions-at-compile-time
#[cfg(feature = "postcard")]
const fn max(a: usize, b: usize) -> usize {
    [a, b][(a < b) as usize]
}

#[cfg(feature = "postcard")]
impl<TxError, RxError> postcard::experimental::max_size::MaxSize for Error<TxError, RxError>
where
    TxError: postcard::experimental::max_size::MaxSize + core::fmt::Debug + defmt::Format,
    RxError: postcard::experimental::max_size::MaxSize + core::fmt::Debug + defmt::Format,
{
    const POSTCARD_MAX_SIZE: usize =
        1 + max(TxError::POSTCARD_MAX_SIZE, RxError::POSTCARD_MAX_SIZE);
}
