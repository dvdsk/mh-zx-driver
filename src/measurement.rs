use super::Error;
use super::PAYLOAD_SIZE;
use core::fmt;
use defmt;

pub(crate) fn checksum(bytes: &[u8; PAYLOAD_SIZE]) -> u8 {
    (!bytes
        .iter()
        .skip(1)
        .take(7)
        .fold(0u8, |s, i| s.wrapping_add(*i)))
    .wrapping_add(1)
}

pub(crate) fn checksum_valid(bytes: &[u8; PAYLOAD_SIZE]) -> bool {
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
    /// If ABC is turned on - the number of performed calibration cycles.
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
    pub(crate) fn parse_response<RxError, TxError>(
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
    pub(crate) fn parse_response<RxError, TxError>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;

    #[test]
    fn parse_measurement() {
        let p = [0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x79];
        Measurement::parse_response::<(), ()>(p).unwrap();

        // checksum mismatch
        let p = [0xFF, 0x86, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78];
        assert!(!checksum_valid(&p));

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
