#![no_std]
#![no_main]

use cortex_m::delay::Delay;
use embedded_hal::digital::OutputPin;
use panic_halt as _;
use rp_pico::{
    Pins, entry,
    hal::{
        self, Sio, Watchdog,
        pac::{CorePeripherals, Peripherals},
        prelude::*,
    },
};

#[entry]
fn main() -> ! {
    let mut pac = Peripherals::take().unwrap();
    let core = CorePeripherals::take().unwrap();

    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .unwrap();

    let mut delay = Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let sio = Sio::new(pac.SIO);

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut led_pin = pins.led.into_push_pull_output();

    loop {
        led_pin.set_high().unwrap();
        delay.delay_ms(500);
        led_pin.set_low().unwrap();
        delay.delay_ms(500);
    }
}
