# Serial bridge for embedded devices

The main goal of this project is to provide an embedded application, which can be uploaded to a microcontroller,
bridging two serial ports. Originally for connecting a virtual COM port (connected via USB to a host machine) to
an ESP-8622 (ESP-01) connected to the microcontroller.

This allows you connect from the host machine, via some terminal application (like `minicom`) to the ESP on the
development board.

It also contains some logic, to enable the programming mode of the ESP, which allows you to flash a new firmware,
from your host machine, to the ESP on the microcontroller board.

## Programming mode

The programming mode is enabled by holding the *user button* while resetting the board.

## LEDs

The setup currently supports two LEDs (if the board supports it):

* An LED indicating that serial traffic is being processed
* An LED indicating that the programming mode is active

## Flashing

The project uses `cargo-embed` from [probe.rs](https://probe.rs/) for flashing:

    cargo embed --release --feature <config>

Where `<config>` is one of the following:

* `stm32f411` – [STM32F7411RE Nucleo 64](#stm32f411)
* `stm32f723` – [STM32F723-DISCOVERY](#stm32f723)

## Configurations

The following configurations are currently supported out of the box:

### STM32F7411RE Nucleo 64
<a id="stm32f411"></a>

| Type         | Port     |
| ------------ | -------- |
| Virtual/Host | `USART2` |
| ESP          | `USART1` |

The STM32F7411RE with the Nucleo 64 board doesn't provide a specialized ESP-01 socket. You need
to manually wire up the ESP with `USART1`:

| STM32 PIN | Nucleo          | Arduino Name | ESP PIN   | Function     |
| --------- | --------------- | ------------ | --------- | ------------ |
| PA9       | CN5.1 / CN10.21 | D8           | RX        | Serial RX/TX |
| PA10      | CN9.3 / CN10.33 | D2           | TX        | Serial TX/RX |
| PC0       | CN8.6 / CN7.38  | A5           | CH_PD /EN | Chip enable  |
| PC1       | CN8.5 / CN7.36  | A4           | RST       | Reset ESP    |
| PB0       | CN8.4 / CN7.34  | A3           | (GP)IO0   | ESP Mode Selector (Normal/Programming) |
| PA4       | CN8.3 / CN7.32  | A2           | (GP)IO2   | ESP Debug TX |
|           | CN6.4 / CN7.16  | +3V3         | 3V3       | VCC          |
|           | CN6.6 / CN7.20  | GND          | GND       | Ground       |

The virtual COM (`USART2`) is routed through the STLINK connection to the host computer, connected via USB.

Note:

  * The GPIO2 port of the ESP is actually a second UART of the ESP, used for sending debug information.
    However, this setup doesn't wire it up as an input, and only ensures that the pin is set to *high*
    during startup, as required by the ESP.

See also:

  * [Product Page](https://www.st.com/en/evaluation-tools/nucleo-f411re.html)

### STM32F723-DISCOVERY
<a id="stm32f723"></a>

| Type         | Port     |
| ------------ | -------- |
| Virtual/Host | `USART6` |
| ESP          | `UART5` |

The STM32F723 discovery board has two sockets for the ESP-01. One on the main board, and a second one
on the extension board. This setup uses the one on the main board, which is mapped to `UART5`. The
installation is rather simple, as you only need to plug in the ESP-01 into the socket.

The virtual COM (`USART6`) is routed through the STLINK connection to the host computer, connected via USB.

Note:

  * Currently, the `stm32f7xx-hal` crate lacks support for the `UART5` port. It has been added on the `master`
    branch. However, a release of that is still pending. That is why the cargo file uses a patch of that crate
    directly from GitHub.

Also see:

  * [Product Page](https://www.st.com/en/evaluation-tools/32f723ediscovery.html) 
