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
use mh_zx_driver::read_sensor;
use nb::block;
use prometheus::{__register_gauge, opts, register_gauge};
use prometheus_exporter::{FinishedUpdate, PrometheusExporter};

use std::path::Path;
use std::time::Duration;

fn main() {
    env_logger::init();

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
    let mut dev = Serial(tty_port);

    info!("Initializing metrics");
    let co2_metric = register_gauge!("mhz1x_co2_concentration", "CO2 concentration").unwrap();
    let t_metric = register_gauge!("mhz1x_temp", "Temperature").unwrap();
    let s_metric = register_gauge!("mhz1x_s", "S value").unwrap();
    let u_metric = register_gauge!("mhz1x_u", "U value").unwrap();

    let mut update_metrics = || {
        info!("Reading data from sensor");
        let reading = {
            let mut op = read_sensor(&mut dev);
            block!(op.wait())
        }
        .expect("Error reading data from sensor");
        info!(
            "Read CO2={}ppm, T={}C",
            reading.co2_concentration,
            reading.temp - 40,
        );
        co2_metric.set(reading.co2_concentration.into());
        t_metric.set(reading.temp.into());
        s_metric.set(reading.s.into());
        u_metric.set(reading.u.into());
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
