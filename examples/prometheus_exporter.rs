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

use linux_embedded_hal::{
    serial_core::{BaudRate, CharSize, FlowControl, Parity, PortSettings, SerialPort, StopBits},
    serial_unix::TTYPort,
    Serial,
};

use env_logger;
use log::info;
use mh_zx_driver as mhz;
use nb::block;
use prometheus::{__register_gauge, opts, register_gauge};
use prometheus_exporter::{FinishedUpdate, PrometheusExporter};

use std::convert::TryInto;
use std::path::Path;
use std::time::Duration;

fn main() {
    env_logger::init().unwrap();

    info!("Initializing UART interface");
    let mut tty_port =
        TTYPort::open(Path::new("/dev/ttyAMA0")).expect("Failed to open serial port device");
    tty_port.set_timeout(Duration::from_millis(100)).unwrap();
    tty_port
        .configure(&PortSettings {
            baud_rate: BaudRate::Baud9600,
            char_size: CharSize::Bits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::Stop1,
            flow_control: FlowControl::FlowNone,
        })
        .expect("Failed to configure serial port");
    let dev = Serial(tty_port);

    let mut sensor = mhz::Sensor::new(dev);

    let mut read_co2 = || -> mhz::Measurement {
        {
            let mut write = sensor.write_packet_op(mhz::commands::READ_CO2);
            block!(write()).expect("Error sending command to sensor");
        }

        let mut p = Default::default();
        {
            let mut read = sensor.read_packet_op(&mut p);
            block!(read()).expect("Error reading response from sensor");
        }

        p.try_into().expect("Error parsing sensor response")
    };

    info!("Initializing metrics");
    let co2_metric = register_gauge!("mh_zx_co2_concentration", "CO2 concentration").unwrap();
    let temp_metric = register_gauge!("mh_zx_temp", "Temperature").unwrap();
    let calib_ticks_metric = register_gauge!("mh_zx_calibration_ticks", "Temperature").unwrap();
    let calib_cycles_metric = register_gauge!("mh_zx_calibration_cycles", "Temperature").unwrap();

    let mut update_metrics = || {
        info!("Reading data from sensor");
        let reading = read_co2();
        info!("Read CO2={}ppm, T={}C", reading.co2, reading.temp - 40,);
        co2_metric.set(reading.co2.into());
        temp_metric.set(reading.temp.into());
        calib_ticks_metric.set(reading.calibration_ticks.into());
        calib_cycles_metric.set(reading.calibration_cycles.into());
    };

    update_metrics();

    info!("Initializing Prometheus exporter");
    let (r, s) = PrometheusExporter::run_and_repeat(
        "0.0.0.0:8888"
            .parse()
            .expect("Failed to parse bind address"),
        Duration::from_secs(60),
    );

    loop {
        r.recv().unwrap();
        update_metrics();
        s.send(FinishedUpdate {}).unwrap();
    }
}
