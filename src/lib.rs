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

use embedded_hal::serial::{Read, Write};
use nb::Result;

use core::convert::TryFrom;
use core::ops::{Deref, DerefMut};

const PAYLOAD_SIZE: usize = 9;

#[derive(Debug, Default)]
pub struct Packet([u8; PAYLOAD_SIZE]);

impl Packet {
    fn checksum_valid(&self) -> bool {
        let cs = (!self
            .0
            .iter()
            .skip(1)
            .take(7)
            .fold(0u8, |s, i| s.wrapping_add(*i)))
        .wrapping_add(1);
        cs == self.0[8]
    }
}

impl Deref for Packet {
    type Target = [u8; PAYLOAD_SIZE];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Packet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct Measurement {
    pub co2: u16,
    pub temp: u8,
    pub calibration_ticks: u8,
    pub calibration_cycles: u8,
}

#[derive(Debug)]
pub struct InvalidResponse(Packet);

impl TryFrom<Packet> for Measurement {
    type Error = InvalidResponse;

    fn try_from(p: Packet) -> core::result::Result<Self, Self::Error> {
        if p[0] != 0xFF || p[1] != 0x86 || !p.checksum_valid() {
            return Err(InvalidResponse(p));
        }

        Ok(Measurement {
            co2: u16::from_be_bytes([p[2], p[3]]),
            temp: p[4],
            calibration_ticks: p[6],
            calibration_cycles: p[7],
        })
    }
}

pub mod commands {
    use super::Packet;

    pub const READ_CO2: &Packet = &Packet([0xFF, 0x01, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79]);
}

pub struct Sensor<U> {
    uart: U,
}

impl<U> Sensor<U>
where
    U: Read<u8> + Write<u8>,
{
    pub fn new(uart: U) -> Sensor<U> {
        Sensor { uart }
    }

    pub fn write_packet<'a>(
        &'a mut self,
        packet: &'a Packet,
    ) -> impl FnMut() -> Result<(), <U as Write<u8>>::Error> + 'a {
        let mut i = 0usize;
        move || {
            while i < PAYLOAD_SIZE {
                self.uart.write(packet[i])?;
                i += 1;
            }
            self.uart.flush()
        }
    }

    pub fn read_packet<'a>(
        &'a mut self,
        packet: &'a mut Packet,
    ) -> impl FnMut() -> Result<(), <U as Read<u8>>::Error> + 'a {
        let mut i = 0usize;
        move || {
            while i < PAYLOAD_SIZE {
                packet[i] = self.uart.read()?;
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
    fn sensor_write() {
        let mut m = Mock::new(&[
            Transaction::write_many(commands::READ_CO2.deref()),
            Transaction::flush(),
        ]);

        let mut s = super::Sensor::new(m.clone());
        let mut op = s.write_packet(commands::READ_CO2);
        block!(op()).unwrap();

        m.done();
    }

    #[test]
    fn sensor_read() {
        let mut m = Mock::new(&[Transaction::read_many(&[
            0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79,
        ])]);

        let mut s = Sensor::new(m.clone());
        let mut p = Default::default();

        let mut op = s.read_packet(&mut p);
        block!(op()).unwrap();

        m.done();
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
            let b = p[i];
            p[i] = b.wrapping_add(1);
            assert!(!p.checksum_valid());
            p[i] = b;
        }
    }
}
