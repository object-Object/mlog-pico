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
    types::{PackedPoint2, ProcessorLinkConfig},
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
use widestring::{U16String, u16str};

const HEAP_SIZE: usize = 64 * 1024;

#[global_allocator]
static HEAP: Heap = Heap::empty();

macro_rules! include_ast {
    ($name:expr) => {
        #[cfg(feature = $name)]
        const AST_BYTES: &[u8] = include_bytes!(env!(concat!("MLOG:src/mlog/", $name, ".mlog")));
    };
}

include_ast!("blink");
include_ast!("print");

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

    let led_pin_id = pins.led.id().num as usize;
    let mut led_pin = pins.led.into_push_pull_output();

    let uart0 = UartPeripheral::new(
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
        uart0.write_full_blocking(msg);
        delay.delay_ms(1000);
        hal::reset();
    }

    // build VM

    let gpio_data = Rc::new(RefCell::new(BuildingData::Memory(Box::new([0.; 30]))));

    let uart0_data = Rc::new(RefCell::new(BuildingData::Message(U16String::new())));

    let mut globals = LVar::create_global_constants();
    globals.extend([
        // GPIO pin constants
        (u16str!("@pinLED").into(), LVar::Constant(led_pin_id.into())),
    ]);

    let mut builder = LogicVMBuilder::new();
    builder.add_buildings([
        Building::from_processor_builder(
            &custom_content::PROCESSOR,
            PackedPoint2 { x: 0, y: 0 },
            ProcessorBuilder {
                ipt: 1.,
                privileged: false,
                code: deserialize_ast(AST_BYTES).unwrap().into_boxed_slice(),
                links: &[
                    ProcessorLinkConfig {
                        name: "gpio".into(),
                        x: 1,
                        y: 0,
                    },
                    ProcessorLinkConfig {
                        name: "uart0".into(),
                        x: 2,
                        y: 0,
                    },
                ],
            },
            &builder,
        ),
        Building {
            block: &custom_content::GPIO,
            position: PackedPoint2 { x: 1, y: 0 },
            data: gpio_data.clone(),
        },
        Building {
            block: &custom_content::UART,
            position: PackedPoint2 { x: 2, y: 0 },
            data: uart0_data.clone(),
        },
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
            if memory[led_pin_id] == 0. {
                led_pin.set_low().unwrap();
            } else {
                led_pin.set_high().unwrap();
            }
        }

        if let BuildingData::Message(message) = &mut *uart0_data.borrow_mut()
            && !message.is_empty()
        {
            let mut buf = [0; 4];
            for c in message.chars_lossy() {
                uart0.write_full_blocking(c.encode_utf8(&mut buf).as_bytes());
            }
            message.clear();
        }
    }
}

mod custom_content {
    use mindustry_rs::{multistr, types::content::Block};

    static DEFAULT: Block = Block {
        name: multistr!(""),
        id: -1,
        logic_id: -1,
        size: 1,
        legacy: false,
        range: 0.,
        item_capacity: 0,
        liquid_capacity: 0.,
    };

    pub static PROCESSOR: Block = Block {
        name: multistr!("pico-processor"),
        id: -1,
        range: f64::MAX,
        ..DEFAULT
    };

    pub static GPIO: Block = Block {
        name: multistr!("gpio"),
        id: -2,
        ..DEFAULT
    };

    pub static UART: Block = Block {
        name: multistr!("uart"),
        id: -3,
        ..DEFAULT
    };
}
