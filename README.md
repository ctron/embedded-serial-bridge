# Serial bridge for STM32F723E-DISCOVERY

This bridges the `UART5` (ESP-01) with the `USART6` (Virtual COM). So you can use your host's USB UART device
to talk to the ESP-01 on the discovery board.

## Flashing

    cargo embed --release
