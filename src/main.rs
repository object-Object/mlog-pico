#![no_std]
#![no_main]

extern crate alloc;

use core::{cell::RefCell, mem::MaybeUninit, time::Duration};

use alloc::{borrow::Cow, boxed::Box, rc::Rc};
use cortex_m::delay::Delay;
use embedded_alloc::TlsfHeap as Heap;
use embedded_hal::digital::OutputPin;
use mindustry_rs::{
    logic::{
        deserialize_ast,
        vm::{Building, BuildingData, LVar, LogicVMBuilder, ProcessorBuilder},
    },
    types::{PackedPoint2, content},
};
use panic_persist::get_panic_message_bytes;
use rp_pico::{
    Pins, entry,
    hal::{
        self, Clock, Sio, Timer, Watchdog,
        fugit::RateExtU32,
        pac::{CorePeripherals, Peripherals},
        uart::{DataBits, StopBits, UartConfig, UartPeripheral},
    },
};
use widestring::u16str;

const HEAP_SIZE: usize = 64 * 1024;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[entry]
fn main() -> ! {
    // init heap
    {
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
    }

    // init peripherals

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

    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let mut delay = Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let sio = Sio::new(pac.SIO);

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut led_pin = pins.led.into_push_pull_output();

    let uart = UartPeripheral::new(
        pac.UART0,
        (pins.gpio0.into_function(), pins.gpio1.into_function()),
        &mut pac.RESETS,
    )
    .enable(
        UartConfig::new(115_200.Hz(), DataBits::Eight, None, StopBits::One),
        clocks.peripheral_clock.freq(),
    )
    .unwrap();

    // check if we panicked on the previous boot

    if let Some(msg) = get_panic_message_bytes() {
        uart.write_full_blocking(msg);
        delay.delay_ms(1000);
        hal::reset();
    }

    // build VM

    let gpio_data = Rc::new(RefCell::new(BuildingData::Memory(Box::new([0.; 30]))));
    let gpio_build = Building {
        block: &content::blocks::AIR,
        position: PackedPoint2 { x: 1, y: 0 },
        data: gpio_data.clone(),
    };

    let mut globals = LVar::create_global_constants();
    globals.extend([(
        u16str!("gpio").into(),
        LVar::Constant(gpio_build.clone().into()),
    )]);

    let mut builder = LogicVMBuilder::new();
    builder.add_buildings([
        Building::from_processor_builder(
            &content::blocks::AIR, // TODO
            PackedPoint2 { x: 0, y: 0 },
            ProcessorBuilder {
                ipt: 1.,
                privileged: false,
                code: deserialize_ast(include_bytes!(env!("MLOG:src/blink.mlog")))
                    .unwrap()
                    .into_boxed_slice(),
                links: &[],
            },
            &builder,
        ),
        gpio_build,
    ]);

    let vm = builder.build_with_globals(Cow::Owned(globals)).unwrap();

    // run!

    let start = timer.get_counter();
    loop {
        vm.do_tick_with_delta(
            Duration::from_micros((timer.get_counter() - start).to_micros()),
            1.0,
        );

        if let BuildingData::Memory(memory) = &*gpio_data.borrow() {
            if memory[25] == 0. {
                led_pin.set_low().unwrap();
            } else {
                led_pin.set_high().unwrap();
            }
        }
    }
}
