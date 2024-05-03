# mh-zx-driver
MH-Z* family CO2 sensor driver built on top of `embedded-io-async` primitives. This
is a `no_std` crate suitable for use on bare-metal.

A fork of `https://github.com/xaep/mh-zx-driver` adapting the project to work
with embedded-io-async.

## Example
```rust
// assume you have some embedded hal implementation giving you
// a tx and rx object.
let (tx, rx) = uart.split();
let mut sensor = Sensor::from_tx_rx(tx, rx);
let measurement = sensor.read().unwrap();
println!("co2 concentration: {}ppm", measurement.co2);
```

## Supported devices

The code has been tested with MH-Z14 sensor, other sensors in MH-Z* family
support the same UART protocol thus should work.

## Datasheets
* [MH-Z19B](https://web.archive.org/web/20180517074844/https://www.winsen-sensor.com/d/files/infrared-gas-sensor/mh-z19b-co2-ver1_0.pdf)
* [MH-Z19](https://web.archive.org/web/20190507154811/https://www.winsen-sensor.com/d/files/PDF/Infrared%20Gas%20Sensor/NDIR%20CO2%20SENSOR/MH-Z19%20CO2%20Ver1.0.pdf)
* [MH-Z14](https://web.archive.org/web/20200105191455/https://www.winsen-sensor.com/d/files/PDF/Infrared%20Gas%20Sensor/NDIR%20CO2%20SENSOR/MH-Z14%20CO2%20V2.4.pdf)

## More resources
* <https://revspace.nl/MHZ19>
* <https://revspace.nl/MH-Z19B>
* <https://habr.com/en/post/401363/>
