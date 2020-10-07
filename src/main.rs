//! Serial bridge UART5(esp01) <-> USART6(virtual com) for the STM32F723E-DISCOVERY board

#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f7 as _;

use cortex_m_rt::entry;
use stm32f7xx_hal::delay::Delay;
use stm32f7xx_hal::{
    pac,
    prelude::*,
    serial::{self, Serial},
};

/// Sends out '\r\n' when a '\r' is read. Helps when the terminal you are using is sending out '\r'
/// only.
const FIX_CRLF: bool = cfg!(feature = "fix_crlf");

#[entry]
fn main() -> ! {
    rtt_init_print!(NoBlockSkip, 4096);

    let p = pac::Peripherals::take().unwrap();

    let rcc = p.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(216.mhz()).freeze();

    let core = stm32f7::stm32f7x3::CorePeripherals::take().unwrap();
    let mut delay = Delay::new(core.SYST, clocks);

    let gpioa = p.GPIOA.split();
    let gpiob = p.GPIOB.split();
    let gpioc = p.GPIOC.split();
    let gpiod = p.GPIOD.split();
    let gpiog = p.GPIOG.split();

    // user button
    let flash_mode = gpioa.pa0.into_pull_down_input().is_high().unwrap();

    // SERIAL pins for UART5
    let tx_pin_esp = gpioc.pc12.into_alternate_af8();
    let rx_pin_esp = gpiod.pd2.into_alternate_af8();

    // SERIAL pins for UART6
    let tx = gpioc.pc6.into_alternate_af8();
    let rx = gpioc.pc7.into_alternate_af8();

    // enable pin
    let mut en = gpiod.pd3.into_push_pull_output();
    // reset pin
    let mut reset = gpiog.pg14.into_push_pull_output();

    let mut esp_gpio0 = gpiog.pg13.into_push_pull_output();
    let mut esp_gpio2 = gpiod.pd6.into_push_pull_output();

    let mut led_red = gpioa.pa7.into_push_pull_output();
    let mut led_green = gpiob.pb1.into_push_pull_output();

    led_green.set_low().unwrap();
    led_red.set_low().unwrap();

    let mode = match flash_mode {
        false => "normal",
        true => "programming",
    };

    // set red LED according to flash mode
    match flash_mode {
        false => led_red.set_low(),
        true => led_red.set_high(),
    }
    .unwrap();

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

    let serial_vcom = Serial::new(
        p.USART6,
        (tx, rx),
        clocks,
        serial::Config {
            baud_rate: 115_200.bps(),
            oversampling: serial::Oversampling::By16,
            character_match: None,
        },
    );

    let serial_esp = Serial::new(
        p.UART5,
        (tx_pin_esp, rx_pin_esp),
        clocks,
        serial::Config {
            baud_rate: 115_200.bps(),
            oversampling: serial::Oversampling::By16,
            character_match: None,
        },
    );

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
            led_green.set_high().ok();
        } else {
            led_green.set_low().ok();
        }
    }
}
