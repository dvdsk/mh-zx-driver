#![no_std]
#![doc = include_str!("../README.md")]

use core::fmt;
use embedded_io_async::{Read, ReadExactError, Write};

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
            (Error::InvalidChecksum, Error::InvalidChecksum) => true,
            (Error::InvalidPacket, Error::InvalidPacket) => true,
            (Error::WritingToUart(e), Error::WritingToUart(e2)) => e == e2,
            (Error::FlushingUart(e), Error::FlushingUart(e2)) => e == e2,
            (Error::ReadingEOF, Error::ReadingEOF) => true,
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

const PAYLOAD_SIZE: usize = 9;
fn checksum(bytes: &[u8; PAYLOAD_SIZE]) -> u8 {
    (!bytes
        .iter()
        .skip(1)
        .take(7)
        .fold(0u8, |s, i| s.wrapping_add(*i)))
    .wrapping_add(1)
}

fn checksum_valid(bytes: &[u8; PAYLOAD_SIZE]) -> bool {
    checksum(bytes) == bytes[8]
}

#[derive(defmt::Format, Debug)]
pub struct Measurement {
    /// CO2 concentration, PPM.
    pub co2: u16,
    /// Temperature, degrees Celsius plus 40.
    pub temp: u8,
    /// If ABC is turned on - counter in "ticks" within a calibration cycle.
    pub calib_ticks: u8,
    /// If ABC is turned on - the nuumber of performed calibration cycles.
    pub calib_cycles: u8,
}

#[derive(defmt::Format, Debug)]
pub struct RawMeasurement {
    // Smoothed temperature ADC value.
    pub adc_temp: u16,
    // CO2 level before clamping
    pub co2: u16,
    // Minimum light ADC value.
    pub adc_min_light: u16,
}

impl Measurement {
    fn parse_response<RxError, TxError>(
        p: [u8; PAYLOAD_SIZE],
    ) -> Result<Self, Error<RxError, TxError>>
    where
        RxError: defmt::Format + fmt::Debug,
        TxError: defmt::Format + fmt::Debug,
    {
        if p[0] != 0xFF || p[1] != 0x86 {
            return Err(Error::InvalidPacket);
        }

        let [_, _, ch, cl, temp, calib_ticks, calib_cycles, _, _] = p;
        Ok(Measurement {
            co2: u16::from_be_bytes([ch, cl]),
            temp,
            calib_ticks,
            calib_cycles,
        })
    }
}

impl RawMeasurement {
    fn parse_response<RxError, TxError>(
        p: [u8; PAYLOAD_SIZE],
    ) -> Result<Self, Error<RxError, TxError>>
    where
        RxError: defmt::Format + fmt::Debug,
        TxError: defmt::Format + fmt::Debug,
    {
        if p[0] != 0xFF || p[1] != 0x85 {
            return Err(Error::InvalidPacket);
        }

        let [_, _, th, tl, ch, cl, lh, ll, _] = p;
        Ok(RawMeasurement {
            adc_temp: u16::from_be_bytes([th, tl]),
            co2: u16::from_be_bytes([ch, cl]),
            adc_min_light: u16::from_be_bytes([lh, ll]),
        })
    }
}

pub mod commands {
    /// Read "final" CO2 concentration.
    pub const READ_CO2: [u8; 9] = [0xFF, 0x01, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79];
    /// Read raw CO2 concentration.
    pub const READ_RAW_CO2: [u8; 9] = [0xFF, 0x01, 0x85, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7a];
}

/// A struct representing sensor interface.
pub struct MHZ<Tx, Rx> {
    uart_tx: Tx,
    uart_rx: Rx,
}

impl<Tx, Rx> MHZ<Tx, Rx>
where
    Tx: Write,
    Tx::Error: defmt::Format,
    Rx: Read,
    Rx::Error: defmt::Format,
{
    /// Constructs the [`Sensor`](struct.Sensor.html) interface from 2 'halves' of UART.
    /// # Warning, take care to setup the UART with the correct settings:
    /// - Baudrate: 9600
    /// - Date bits: 8 bits
    /// - Stop bits: 1 bit 
    /// - Calibrate byte: no
    pub fn from_tx_rx(uart_tx: Tx, uart_rx: Rx) -> MHZ<Tx, Rx> {
        MHZ { uart_tx, uart_rx }
    }

    async fn read_into(&mut self, buf: &mut [u8]) -> Result<(), Error<Tx::Error, Rx::Error>> {
        self.uart_rx.read_exact(buf).await.map_err(|e| match e {
            ReadExactError::UnexpectedEof => Error::ReadingEOF,
            ReadExactError::Other(e) => Error::Reading(e),
        })
    }

    pub async fn read_co2(&mut self) -> Result<Measurement, Error<Tx::Error, Rx::Error>> {
        self.uart_tx
            .write_all(&commands::READ_CO2)
            .await
            .map_err(Error::WritingToUart)?;
        defmt::trace!("flushing uart");
        self.uart_tx.flush().await.map_err(Error::FlushingUart)?;

        defmt::trace!("reading uart");
        let mut buf = [0u8; PAYLOAD_SIZE];
        self.uart_rx
            .read_exact(&mut buf)
            .await
            .map_err(|e| match e {
                ReadExactError::UnexpectedEof => Error::ReadingEOF,
                ReadExactError::Other(e) => Error::Reading(e),
            })?;
        defmt::trace!("checking packet checksum");
        if !checksum_valid(&buf) {
            return Err(Error::InvalidChecksum);
        }
        Measurement::parse_response(buf)
    }

    pub async fn read_co2_raw(&mut self) -> Result<RawMeasurement, Error<Tx::Error, Rx::Error>> {
        self.uart_tx
            .write_all(&commands::READ_RAW_CO2)
            .await
            .map_err(Error::WritingToUart)?;
        self.uart_tx.flush().await.map_err(Error::FlushingUart)?;

        let mut buf = [0u8; PAYLOAD_SIZE];
        self.read_into(&mut buf).await?;
        if !checksum_valid(&buf) {
            return Err(Error::InvalidChecksum);
        }
        RawMeasurement::parse_response(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_measurement() {
        let p = [0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79];
        Measurement::parse_response::<(), ()>(p).unwrap();

        // checksum mismatch
        let p = [0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78];
        assert!(!checksum_valid(&p));
        Measurement::parse_response::<(), ()>(p).unwrap_err();

        // invalid command field
        let p = [0xFF, 0x87, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78];
        assert!(checksum_valid(&p));
        Measurement::parse_response::<(), ()>(p).unwrap_err();

        // byte0 is not 0xFF
        let p = [0xFE, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79];
        assert!(checksum_valid(&p));
        Measurement::parse_response::<(), ()>(p).unwrap_err();
    }

    #[test]
    fn packet_checksum() {
        let p = [0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79];
        assert!(checksum_valid(&p));

        for i in 1..PAYLOAD_SIZE - 1 {
            let mut corrupted = p;
            corrupted[i] = corrupted[i].wrapping_add(1);
            assert!(!checksum_valid(&corrupted));
        }

        assert!(checksum_valid(&commands::READ_CO2));
        assert!(checksum_valid(&commands::READ_RAW_CO2));
    }
}
