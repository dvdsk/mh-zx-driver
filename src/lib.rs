#![cfg_attr(not(target_os = "linux"), no_std)]
#![doc = include_str!("../README.md")]

use embedded_io_async::{Read, ReadExactError, Write};

mod error;
pub use error::Error;
mod measurement;
pub use measurement::{Measurement, RawMeasurement};
mod read_package;
use read_package::read_package;

const PAYLOAD_SIZE: usize = 9;

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

    pub async fn read_co2(
        &mut self,
    ) -> Result<measurement::Measurement, Error<Tx::Error, Rx::Error>> {
        self.uart_tx
            .write_all(&commands::READ_CO2)
            .await
            .map_err(Error::WritingToUart)?;
        defmt::trace!("flushing uart");
        self.uart_tx.flush().await.map_err(Error::FlushingUart)?;

        defmt::trace!("reading uart");
        let package = read_package::<Tx, Rx>(&mut self.uart_rx).await?;

        defmt::trace!("checking packet checksum");
        if !measurement::checksum_valid(&package) {
            return Err(Error::InvalidChecksum);
        }
        measurement::Measurement::parse_response(package)
    }

    pub async fn read_co2_raw(
        &mut self,
    ) -> Result<measurement::RawMeasurement, Error<Tx::Error, Rx::Error>> {
        self.uart_tx
            .write_all(&commands::READ_RAW_CO2)
            .await
            .map_err(Error::WritingToUart)?;
        self.uart_tx.flush().await.map_err(Error::FlushingUart)?;

        let mut buf = [0u8; PAYLOAD_SIZE];
        self.read_into(&mut buf).await?;
        if !measurement::checksum_valid(&buf) {
            return Err(Error::InvalidChecksum);
        }
        measurement::RawMeasurement::parse_response(buf)
    }
}
