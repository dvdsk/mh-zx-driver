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

use embedded_hal::serial::{Read as SRead, Write as SWrite};
use nb::Result;

use core::fmt;

const PAYLOAD_SIZE: usize = 9;
const CMD_READ_SENSOR: &[u8; PAYLOAD_SIZE] =
    &[0xFF, 0x01, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79];

type Packet = [u8; PAYLOAD_SIZE];

/// A wrapper for possible errors returned by sensor operations.
pub enum Error<T>
where
    T: SRead<u8> + SWrite<u8>,
{
    WriteError(<T as SWrite<u8>>::Error),
    ReadError(<T as SRead<u8>>::Error),
    ResponseInvalid,
}

impl<T> fmt::Debug for Error<T>
where
    T: SRead<u8> + SWrite<u8>,
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

/// A structure holding state for active read operation.
pub struct ReadOp<'a, T> {
    dev: &'a mut T,
    state: State,
}

/// Sensor readings.
#[derive(PartialEq)]
pub struct Reading {
    /// CO2 concentration in PPM.
    pub co2_concentration: u16,
    /// Temperature in Celsius plus 40. Some sensors in the family may not export this field.
    pub temp: u8,
    pub s: u8,
    pub u: u16,
}

impl<'a, T> ReadOp<'a, T>
where
    T: SRead<u8> + SWrite<u8>,
{
    pub fn wait(&mut self) -> Result<Reading, Error<T>> {
        loop {
            match self.state {
                State::Writing(_, PAYLOAD_SIZE) => {
                    self.dev.flush().map_err(|e| e.map(Error::WriteError))?;
                    self.state = State::Reading(Default::default(), 0)
                }
                State::Writing(buf, ref mut len) => {
                    self.dev
                        .write(buf[*len])
                        .map_err(|e| e.map(Error::WriteError))?;
                    *len += 1;
                }
                State::Reading(ref buf, PAYLOAD_SIZE) => {
                    let res = parse_response(buf);
                    self.reset();
                    return res.map_err(|_| Error::ResponseInvalid.into());
                }
                State::Reading(ref mut buf, ref mut len) => {
                    buf[*len] = self.dev.read().map_err(|e| e.map(Error::ReadError))?;
                    *len += 1;
                }
            }
        }
    }

    fn reset(&mut self) {
        self.state = State::Writing(CMD_READ_SENSOR, 0)
    }
}

struct ResponseInvalid {}

fn parse_response(buf: &Packet) -> core::result::Result<Reading, ResponseInvalid> {
    let chksum = (!buf
        .iter()
        .skip(1)
        .take(7)
        .fold(0u8, |s, i| s.wrapping_add(*i)))
    .wrapping_add(1);
    if chksum != buf[8] {
        return Err(ResponseInvalid {});
    }

    let co2_concentration = u16::from_be_bytes([buf[2], buf[3]]);
    let temp = buf[4];
    let s = buf[5];
    let u = u16::from_be_bytes([buf[6], buf[7]]);

    Ok(Reading {
        co2_concentration,
        temp,
        s,
        u,
    })
}

/// Start reading CO2 concentration data from sensor.
pub fn read_sensor<T>(dev: &mut T) -> ReadOp<T> {
    ReadOp {
        state: State::Writing(CMD_READ_SENSOR, 0),
        dev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        assert!(parse_response(&[0xFF, 0x01, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]).is_err());
        assert!(
            parse_response(&[0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79]).ok()
                == Some(Reading {
                    co2_concentration: 0x100u16,
                    temp: 0,
                    s: 0,
                    u: 0
                })
        );
    }
}
