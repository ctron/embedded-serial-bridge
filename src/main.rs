//! Serial bridge UART5(esp01) <-> USART6(virtual com) for the STM32F723E-DISCOVERY board

#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

use panic_rtt_target as _;

use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;

use embedded_hal::digital::v2::OutputPin;

#[cfg(feature = "stm32f4xx")]
use stm32f4 as _;
#[cfg(feature = "stm32f7xx")]
use stm32f7 as _;

#[cfg(feature = "stm32f4xx")]
use stm32f4xx_hal as hal;
#[cfg(feature = "stm32f7xx")]
use stm32f7xx_hal as hal;

use hal::{
    delay::Delay,
    prelude::*,
    serial::{self, Serial},
    time::U32Ext,
};

#[cfg(feature = "stm32f4xx")]
use stm32f4xx_hal::stm32::Peripherals as DevicePeripherals;

#[cfg(feature = "stm32f7xx")]
use stm32f7xx_hal::pac::Peripherals as DevicePeripherals;

#[cfg(feature = "stm32f4xx")]
macro_rules! new_serial {
    ($device:expr, $clocks:expr, $name:ident, $lower:ident, $tx:expr, $rx:expr) => {
        Serial::$lower(
            $device.$name,
            ($tx, $rx),
            serial::config::Config {
                baudrate: 115_200.bps(),
                wordlength: serial::config::WordLength::DataBits8,
                parity: serial::config::Parity::ParityNone,
                stopbits: serial::config::StopBits::STOP1,
            },
            $clocks,
        )
        .unwrap()
    };
}
#[cfg(feature = "stm32f7xx")]
macro_rules! new_serial {
    ($device:expr, $clocks:expr, $name:ident, $lower:ident, $tx:expr, $rx:expr) => {
        Serial::new(
            $device.$name,
            ($tx, $rx),
            $clocks,
            serial::Config {
                baud_rate: 115_200.bps(),
                oversampling: serial::Oversampling::By16,
                character_match: None,
            },
        )
    };
}

/// Sends out '\r\n' when a '\r' is read. Helps when the terminal you are using is sending out '\r'
/// only.
const FIX_CRLF: bool = cfg!(feature = "fix_crlf");

#[entry]
fn main() -> ! {
    rtt_init_print!(NoBlockSkip, 4096);
    let p = DevicePeripherals::take().unwrap();

    let rcc = p.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(100.mhz()).freeze();

    let core = cortex_m::Peripherals::take().unwrap();
    let mut delay = Delay::new(core.SYST, clocks);

    let gpioa = p.GPIOA.split();
    let gpiob = p.GPIOB.split();
    let gpioc = p.GPIOC.split();
    #[cfg(feature = "stm32f7xx")]
    let gpiod = p.GPIOD.split();
    #[cfg(feature = "stm32f7xx")]
    let gpiog = p.GPIOG.split();

    // user button
    #[cfg(feature = "stm32f4xx")]
    let flash_mode = !gpioc.pc13.into_pull_down_input().is_high().unwrap();
    #[cfg(feature = "stm32f7xx")]
    let flash_mode = gpioa.pa0.into_pull_down_input().is_high().unwrap();

    // SERIAL pins for esp
    #[cfg(feature = "stm32f4xx")]
    let (rx_pin_esp, tx_pin_esp) = {
        let rx = gpioa.pa10.into_alternate_af7();
        let tx = gpioa.pa9.into_alternate_af7();
        (rx, tx)
    };
    #[cfg(feature = "stm32f7xx")]
    let (rx_pin_esp, tx_pin_esp) = {
        let rx = gpiod.pd2.into_alternate_af8();
        let tx = gpioc.pc12.into_alternate_af8();
        (rx, tx)
    };

    // SERIAL pins for vcom
    #[cfg(feature = "stm32f4xx")]
    let (rx_pin_vcom, tx_pin_vcom) = {
        let tx = gpioa.pa2.into_alternate_af7();
        let rx = gpioa.pa3.into_alternate_af7();
        (rx, tx)
    };
    #[cfg(feature = "stm32f7xx")]
    let (rx_pin_vcom, tx_pin_vcom) = {
        let rx = gpioc.pc7.into_alternate_af8();
        let tx = gpioc.pc6.into_alternate_af8();
        (rx, tx)
    };

    // esp control pins
    #[cfg(feature = "stm32f4xx")]
    let (mut en, mut reset, mut esp_gpio0, mut esp_gpio2) = {
        let en = gpioc.pc0.into_push_pull_output();
        let reset = gpioc.pc1.into_push_pull_output();
        let esp_gpio0 = gpiob.pb0.into_push_pull_output();
        let esp_gpio2 = gpioa.pa4.into_push_pull_output();
        (en, reset, esp_gpio0, esp_gpio2)
    };
    #[cfg(feature = "stm32f7xx")]
    let (mut en, mut reset, mut esp_gpio0, mut esp_gpio2) = {
        let en = gpiod.pd3.into_push_pull_output();
        let reset = gpiog.pg14.into_push_pull_output();
        let esp_gpio0 = gpiog.pg13.into_push_pull_output();
        let esp_gpio2 = gpiod.pd6.into_push_pull_output();
        (en, reset, esp_gpio0, esp_gpio2)
    };

    // user interface LEDs
    #[cfg(feature = "stm32f4xx")]
    let (led_flash, mut led_busy) = {
        let led_green = gpioa.pa5.into_push_pull_output();
        (Option::<&mut dyn OutputPin<Error = ()>>::None, led_green)
    };
    #[cfg(feature = "stm32f7xx")]
    let (led_flash, mut led_busy) = {
        let led_red = gpioa.pa7.into_push_pull_output();
        let led_green = gpiob.pb1.into_push_pull_output();
        (Some(led_red), led_green)
    };

    led_busy.set_low().unwrap();

    let mode = match flash_mode {
        false => "normal",
        true => "programming",
    };

    #[allow(unused_mut)]
    if let Some(mut led_flash) = led_flash {
        // set red LED according to flash mode
        match flash_mode {
            false => led_flash.set_low(),
            true => led_flash.set_high(),
        }
        .unwrap();
    }

    rprintln!("Boot ESP ({})", mode);

    // power down first
    en.set_low().unwrap();
    reset.set_low().unwrap();

    // wait a bit
    delay.delay_ms(100u8);

    if flash_mode {
        // setup for programming
        esp_gpio0.set_low().unwrap();
        esp_gpio2.set_high().unwrap();
    } else {
        // set both to HIGH (boot from flash, non-programming mode)
        esp_gpio0.set_high().unwrap();
        esp_gpio2.set_high().unwrap();
    }

    // power on
    en.set_high().unwrap();
    reset.set_high().unwrap();

    rprintln!("Boot ESP ... done");

    #[cfg(feature = "stm32f411")]
    let (serial_vcom, serial_esp) = {
        let vcom = new_serial!(p, clocks, USART2, usart2, tx_pin_vcom, rx_pin_vcom);
        let esp = new_serial!(p, clocks, USART1, usart1, tx_pin_esp, rx_pin_esp);
        (vcom, esp)
    };
    #[cfg(feature = "stm32f723")]
    let (serial_vcom, serial_esp) = {
        let vcom = new_serial!(p, clocks, USART6, usart6, tx_pin_vcom, rx_pin_vcom);
        let esp = new_serial!(p, clocks, UART5, uart, tx_pin_esp, rx_pin_esp);
        (vcom, esp)
    };

    let (mut tx_esp, mut rx_esp) = serial_esp.split();
    let (mut tx_vcom, mut rx_vcom) = serial_vcom.split();

    const LEN: usize = 128;
    let mut to_esp = [0u8; LEN];
    let mut to_vcom = [0u8; LEN];

    let mut head_esp = 0usize;
    let mut tail_esp = 0usize;
    let mut head_vcom = 0usize;
    let mut tail_vcom = 0usize;

    if !flash_mode && FIX_CRLF {
        rprintln!("CR/LF translation active");
    } else {
        rprintln!("CR/LF translation not active");
    }

    loop {
        match rx_vcom.read() {
            Ok(c) if !flash_mode && FIX_CRLF && c == b'\r' => {
                if tail_esp + 1 < LEN {
                    to_esp[tail_esp] = b'\r';
                    to_esp[tail_esp + 1] = b'\n';
                    tail_esp += 2;
                }
            }
            Ok(c) if !flash_mode && FIX_CRLF && c == b'\n' => {}
            Ok(c) => {
                if tail_esp < LEN {
                    to_esp[tail_esp] = c;
                    tail_esp += 1;
                }
            }
            Err(_) => {}
        }
        match rx_esp.read() {
            Ok(c) => {
                if tail_vcom < LEN {
                    to_vcom[tail_vcom] = c;
                    tail_vcom += 1;
                }
            }
            Err(_) => {}
        }

        let mut busy = false;

        if head_vcom < tail_vcom {
            busy = true;
            match tx_vcom.write(to_vcom[head_vcom]) {
                Ok(_) => {
                    head_vcom += 1;
                    if head_vcom == tail_vcom {
                        head_vcom = 0;
                        tail_vcom = 0;
                    }
                }
                Err(_) => {}
            }
        }
        if head_esp < tail_esp {
            busy = true;
            match tx_esp.write(to_esp[head_esp]) {
                Ok(_) => {
                    head_esp += 1;
                    if head_esp == tail_esp {
                        head_esp = 0;
                        tail_esp = 0;
                    }
                }
                Err(_) => {}
            }
        }

        if busy {
            led_busy.set_high().ok();
        } else {
            led_busy.set_low().ok();
        }
    }
}
