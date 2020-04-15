/*
 Copyright 2020 Constantine Verutin

 Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

#![no_std]

//! # MH-Z* CO2 sensor driver
//!
//! MH-Z* family CO2 sensor driver built on top of `embedded-hal` primitives.
//! This is a `no_std` crate suitable for use on bare-metal.
//!
//! ## Usage
//! The [`Sensor`](struct.Sensor.html) struct exposes methods to
//! send commands ([`write_packet_op`](struct.Sensor.html#method.write_packet_op))
//! to the sensor and to read the response([`read_packet_op`](struct.Sensor.html#method.read_packet_op)).
//!
//! ## Example
//! ```
//! use mh_zx_driver::{commands, Sensor, Measurement};
//! use nb::block;
//! use core::convert::TryInto;
//! # use embedded_hal_mock::serial::{Mock, Transaction};
//!
//! # let mut uart = Mock::new(&[
//! #   Transaction::write_many(commands::READ_CO2.as_slice()),
//! #   Transaction::flush(),
//! #   Transaction::read_many(&[
//! #     0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79,
//! #   ]),
//! # ]);
//! let mut sensor = Sensor::new(uart);
//! // Send command to the sensor.
//! {
//!   let mut op = sensor.write_packet_op(commands::READ_CO2);
//!   block!(op()).unwrap();
//! };
//! // Read the response.
//! let mut packet = Default::default();
//! {
//!   let mut op = sensor.read_packet_op(&mut packet);
//!   block!(op()).unwrap();
//! }
//! let meas: Measurement = packet.try_into().unwrap();
//! println!("CO2 concentration: {}", meas.co2);
//! ```
use embedded_hal::serial::{Read, Write};
use nb::Result;

use core::convert::TryFrom;

const PAYLOAD_SIZE: usize = 9;

/// A wrapper for payload (9 bytes) sent/received by sensor hardware with some utility methods.
#[derive(Debug, Default)]
pub struct Packet([u8; PAYLOAD_SIZE]);

impl Packet {
    /// Returns packet checksum.
    fn checksum(&self) -> u8 {
        (!self
            .0
            .iter()
            .skip(1)
            .take(7)
            .fold(0u8, |s, i| s.wrapping_add(*i)))
        .wrapping_add(1)
    }

    /// Verifies packet checksum.
    fn checksum_valid(&self) -> bool {
        self.checksum() == self.0[8]
    }

    /// Returns underlying byte payload as slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

/// A struct representing measurement data returned by sensor as a response to
/// the [`READ_CO2`](commands/constant.READ_CO2.html) command.
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

/// A struct representing raw CO2 data returned by sensor as a response to
/// the [`READ_RAW_CO2`](commands/constant.READ_RAW_CO2.html) command.
pub struct RawMeasurement {
    // Smoothed temperature ADC value.
    pub adc_temp: u16,
    // CO2 level before clamping
    pub co2: u16,
    // Minimum light ADC value.
    pub adc_min_light: u16,
}

#[derive(Debug)]
pub struct InvalidResponse(Packet);

impl TryFrom<Packet> for Measurement {
    type Error = InvalidResponse;

    fn try_from(p: Packet) -> core::result::Result<Self, Self::Error> {
        if p.0[0] != 0xFF || p.0[1] != 0x86 || !p.checksum_valid() {
            return Err(InvalidResponse(p));
        }

        let Packet([_, _, ch, cl, temp, calib_ticks, calib_cycles, _, _]) = p;
        Ok(Measurement {
            co2: u16::from_be_bytes([ch, cl]),
            temp,
            calib_ticks,
            calib_cycles,
        })
    }
}

impl TryFrom<Packet> for RawMeasurement {
    type Error = InvalidResponse;

    fn try_from(p: Packet) -> core::result::Result<Self, Self::Error> {
        if p.0[0] != 0xFF || p.0[1] != 0x85 || !p.checksum_valid() {
            return Err(InvalidResponse(p));
        }

        let Packet([_, _, th, tl, ch, cl, lh, ll, _]) = p;
        Ok(RawMeasurement {
            adc_temp: u16::from_be_bytes([th, tl]),
            co2: u16::from_be_bytes([ch, cl]),
            adc_min_light: u16::from_be_bytes([lh, ll]),
        })
    }
}

pub mod commands {
    use super::Packet;

    /// Read "final" CO2 concentration.
    pub const READ_CO2: &Packet = &Packet([0xFF, 0x01, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79]);
    /// Read raw CO2 concentration.
    pub const READ_RAW_CO2: &Packet =
        &Packet([0xFF, 0x01, 0x85, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7a]);
}

pub struct UartWrapper<R, W>(R, W);

impl<R, W> Read<u8> for UartWrapper<R, W>
where
    R: Read<u8>,
{
    type Error = R::Error;
    fn read(&mut self) -> Result<u8, R::Error> {
        self.0.read()
    }
}

impl<R, W> Write<u8> for UartWrapper<R, W>
where
    W: Write<u8>,
{
    type Error = W::Error;
    fn write(&mut self, word: u8) -> Result<(), W::Error> {
        self.1.write(word)
    }
    fn flush(&mut self) -> Result<(), W::Error> {
        self.1.flush()
    }
}

/// A struct representing sensor interface.
pub struct Sensor<U> {
    uart: U,
}

impl<R, W> Sensor<UartWrapper<R, W>>
where
    R: Read<u8>,
    W: Write<u8>,
{
    //! Constructs the [`Sensor`](struct.Sensor.html) interface from 2 'halves' of UART.
    pub fn from_rx_tx(read: R, write: W) -> Sensor<UartWrapper<R, W>> {
        Sensor {
            uart: UartWrapper(read, write),
        }
    }
}

impl<U> Sensor<U>
where
    U: Read<u8> + Write<u8>,
{
    pub fn new(uart: U) -> Sensor<U> {
        Sensor { uart }
    }

    /// Write a packet to the device.
    ///
    /// Returns a closure that sends a packet to the sensor.
    /// The result of this function can be used with the
    /// [`block!()`](../nb/macro.block.html) macro from
    /// [`nb`](../nb/index.html) crate, e.g.:
    /// ```
    /// # use embedded_hal_mock::serial::{Mock, Transaction};
    /// # use mh_zx_driver::{commands, Sensor};
    /// # use nb::block;
    ///
    /// # let mut uart = Mock::new(&[
    /// #   Transaction::write_many(commands::READ_CO2.as_slice()),
    /// #   Transaction::flush(),
    /// # ]);
    /// # let mut sensor = Sensor::new(uart);
    /// let mut op = sensor.write_packet_op(commands::READ_CO2);
    /// block!(op()).unwrap();
    /// ```
    pub fn write_packet_op<'a>(
        &'a mut self,
        packet: &'a Packet,
    ) -> impl FnMut() -> Result<(), <U as Write<u8>>::Error> + 'a {
        let mut i = 0usize;
        move || {
            while i < PAYLOAD_SIZE {
                self.uart.write(packet.0[i])?;
                i += 1;
            }
            self.uart.flush()
        }
    }

    /// Read a packet from the device.
    ///
    /// Returns a closure that reads a response packet from the sensor.
    /// The result of this function can be used with the
    /// [`block!()`](../nb/macro.block.html) macro from
    /// [`nb`](../nb/index.html) crate, e.g.:
    /// ```
    /// # use embedded_hal_mock::serial::{Mock, Transaction};
    /// # use mh_zx_driver::{commands, Sensor, Packet};
    /// # use nb::block;
    ///
    /// # let mut uart = Mock::new(&[
    /// #   Transaction::read_many(&[
    /// #     0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79,
    /// #   ]),
    /// # ]);
    /// # let mut sensor = Sensor::new(uart);
    /// let mut packet = Default::default();
    /// let mut op = sensor.read_packet_op(&mut packet);
    /// block!(op()).unwrap();
    /// ```
    pub fn read_packet_op<'a>(
        &'a mut self,
        packet: &'a mut Packet,
    ) -> impl FnMut() -> Result<(), <U as Read<u8>>::Error> + 'a {
        let mut i = 0usize;
        move || {
            while i < PAYLOAD_SIZE {
                packet.0[i] = self.uart.read()?;
                i += 1;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use embedded_hal_mock::serial::{Mock, Transaction};
    use nb::block;

    use super::*;
    use core::convert::TryInto;

    #[test]
    fn sensor_rx_tx() {
        let mut rx = Mock::new(&[Transaction::read_many(&[
            0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79,
        ])]);
        let mut tx = Mock::new(&[
            Transaction::write_many(commands::READ_CO2.0),
            Transaction::flush(),
        ]);

        let mut s = Sensor::from_rx_tx(rx.clone(), tx.clone());

        {
            let mut op = s.write_packet_op(commands::READ_CO2);
            block!(op()).unwrap()
        }
        let mut p = Default::default();
        {
            let mut op = s.read_packet_op(&mut p);
            block!(op()).unwrap()
        }

        let _m: Measurement = p.try_into().unwrap();
        rx.done();
        tx.done();
    }

    #[test]
    fn parse_measurement() {
        let p: Packet = Packet([0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79]);

        let _m: Measurement = p.try_into().unwrap();

        // checksum mismatch
        let p: Packet = Packet([0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78]);
        assert!(!p.checksum_valid());
        let res: core::result::Result<Measurement, _> = p.try_into();
        assert!(res.is_err());

        // invalid command field
        let p: Packet = Packet([0xFF, 0x87, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78]);
        assert!(p.checksum_valid());
        let res: core::result::Result<Measurement, _> = p.try_into();
        assert!(res.is_err());

        // byte0 is not 0xFF
        let p: Packet = Packet([0xFE, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79]);
        assert!(p.checksum_valid());
        let res: core::result::Result<Measurement, _> = p.try_into();
        assert!(res.is_err());
    }

    #[test]
    fn packet_checksum() {
        let mut p: Packet = Packet([0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79]);

        assert!(p.checksum_valid());

        for i in 1..PAYLOAD_SIZE - 1 {
            let b = p.0[i];
            p.0[i] = b.wrapping_add(1);
            assert!(!p.checksum_valid());
            p.0[i] = b;
        }

        assert!(commands::READ_CO2.checksum_valid());
        assert!(commands::READ_RAW_CO2.checksum_valid());
    }
}
