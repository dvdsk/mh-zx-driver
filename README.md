# mh-zx-driver
MH-Z* family CO2 sensor driver built on top of `embedded-hal` primitives. This is a `no_std` crate suitable for use on bare-metal.

## Supported devices

The code has been tested with MH-Z19B sensor, other sensors in MH-Z* family
support the same UART protocol.

## Datasheets
* [MH-Z19B](https://web.archive.org/web/20180517074844/https://www.winsen-sensor.com/d/files/infrared-gas-sensor/mh-z19b-co2-ver1_0.pdf)
* [MH-Z19](https://web.archive.org/web/20190507154811/https://www.winsen-sensor.com/d/files/PDF/Infrared%20Gas%20Sensor/NDIR%20CO2%20SENSOR/MH-Z19%20CO2%20Ver1.0.pdf)
* [MH-Z14](https://web.archive.org/web/20200105191455/https://www.winsen-sensor.com/d/files/PDF/Infrared%20Gas%20Sensor/NDIR%20CO2%20SENSOR/MH-Z14%20CO2%20V2.4.pdf)

## More resources
* https://revspace.nl/MHZ19
* https://revspace.nl/MH-Z19B
* https://habr.com/en/post/401363/
