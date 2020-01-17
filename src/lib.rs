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

use core::fmt;

const PAYLOAD_SIZE: usize = 9;
const CMD_READ_SENSOR: &[u8; PAYLOAD_SIZE] =
    &[0xFF, 0x01, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79];

type Packet = [u8; PAYLOAD_SIZE];

/// A wrapper for possible errors returned by sensor operations.
pub enum Error<T>
where
    T: Read<u8> + Write<u8>,
{
    WriteError(<T as Write<u8>>::Error),
    ReadError(<T as Read<u8>>::Error),
    ResponseInvalid,
}

impl<T> fmt::Debug for Error<T>
where
    T: Read<u8> + Write<u8>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WriteError(_) => write!(f, "WriteError"),
            Error::ReadError(_) => write!(f, "ReadError"),
            Error::ResponseInvalid => write!(f, "ResponseInvalid"),
        }
    }
}

enum State {
    Writing(&'static Packet, usize),
    Reading(Packet, usize),
}

/// Sensor readings.
#[derive(PartialEq)]
pub struct SensorReading {
    /// CO2 concentration in PPM.
    pub co2: u16,
    /// Temperature in Celsius plus 40. Some sensors in the family may not export this field.
    pub temp: u8,
}

/// Start reading CO2 concentration data from sensor.
pub fn read<'a, T>(dev: &'a mut T) -> impl 'a + FnMut() -> Result<SensorReading, Error<T>>
where
    T: Write<u8> + Read<u8>,
{
    let mut state = State::Writing(CMD_READ_SENSOR, 0);
    move || loop {
        match state {
            State::Writing(_, PAYLOAD_SIZE) => {
                dev.flush().map_err(|e| e.map(Error::WriteError))?;
                state = State::Reading(Default::default(), 0)
            }
            State::Writing(buf, ref mut len) => {
                dev.write(buf[*len]).map_err(|e| e.map(Error::WriteError))?;
                *len += 1;
            }
            State::Reading(ref buf, PAYLOAD_SIZE) => {
                // checksum validation
                let cs = (!buf
                    .iter()
                    .skip(1)
                    .take(7)
                    .fold(0u8, |s, i| s.wrapping_add(*i)))
                .wrapping_add(1);
                if cs != buf[8] {
                    return Err(Error::ResponseInvalid.into());
                }

                return Ok(SensorReading {
                    co2: u16::from_be_bytes([buf[2], buf[3]]),
                    temp: buf[4],
                });
            }
            State::Reading(ref mut buf, ref mut len) => {
                buf[*len] = dev.read().map_err(|e| e.map(Error::ReadError))?;
                *len += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use embedded_hal_mock::serial::{Mock, Transaction};
    use nb::block;

    #[test]
    fn test_read() {
        let mut m = Mock::new(&[
            Transaction::write_many(super::CMD_READ_SENSOR),
            Transaction::flush(),
            Transaction::read_many(&[0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79]),
        ]);

        let res = {
            let mut op = super::read(&mut m);
            block!(op())
        };
        m.done();
        assert!(
            res.ok()
                == Some(super::SensorReading {
                    co2: 0x100u16,
                    temp: 0,
                })
        );
    }
}
