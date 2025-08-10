#![no_std]
#![no_main]

extern crate alloc;

mod custom_content;

use alloc::{borrow::Cow, boxed::Box, rc::Rc};
use core::{cell::RefCell, mem::MaybeUninit};

use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embassy_rp::{
    bind_interrupts, gpio,
    peripherals::USB,
    uart::{self, Uart},
    usb,
};
use embassy_time::{Instant, Timer};
use embassy_usb::{
    UsbDevice,
    class::cdc_acm::{self, CdcAcmClass},
};
use embedded_alloc::TlsfHeap as Heap;
use embedded_io_async::Write;
use mindustry_rs::{
    logic::{
        deserialize_ast,
        vm::{Building, BuildingData, LVar, LogicVMBuilder, ProcessorBuilder},
    },
    types::{PackedPoint2, ProcessorLinkConfig},
};
use panic_persist::get_panic_message_bytes;
use widestring::{U16String, u16str};

macro_rules! include_ast {
    ($name:expr) => {
        #[cfg(feature = $name)]
        const AST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mlog/", $name, ".bin"));
        #[cfg(feature = $name)]
        const PROGRAM_NAME: &str = $name;
    };
}

include_ast!("blink");
include_ast!("print");
include_ast!("print_usb");

const HEAP_SIZE: usize = 64 * 1024;

#[global_allocator]
static HEAP: Heap = Heap::empty();

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

const MAX_USB_PACKET_SIZE: usize = 64;

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, usb::Driver<'static, USB>>) {
    usb.run().await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // init heap
    {
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) };
    }

    // init peripherals

    let p = embassy_rp::init(Default::default());

    let uart_config = uart::Config::default();
    let mut uart0 =
        Uart::new_with_rtscts_blocking(p.UART0, p.PIN_0, p.PIN_1, p.PIN_3, p.PIN_2, uart_config);

    // as soon as the UART is up, check if we panicked on the previous boot
    if let Some(msg) = get_panic_message_bytes() {
        uart0.blocking_write(msg).unwrap();
        Timer::after_secs(1).await;
        cortex_m::peripheral::SCB::sys_reset();
    }

    let mut led = gpio::Output::new(p.PIN_25, gpio::Level::Low);

    // set up USB

    let usb_driver = usb::Driver::new(p.USB, Irqs);

    // https://pid.codes/1209/0001/
    let mut usb_config = embassy_usb::Config::new(0x1209, 0x0001);
    usb_config.manufacturer = Some("object-Object");
    usb_config.product = Some("mlog-pico");
    usb_config.serial_number = Some(PROGRAM_NAME);

    let mut usb_builder = embassy_usb::Builder::new(
        usb_driver,
        usb_config,
        leak([0; 256]),
        leak([0; 256]),
        &mut [],
        leak([0; 64]),
    );

    let (mut usb_sender, _) = CdcAcmClass::new(
        &mut usb_builder,
        leak(cdc_acm::State::new()),
        MAX_USB_PACKET_SIZE as u16,
    )
    .split();

    let usb = usb_builder.build();

    spawner.must_spawn(usb_task(usb));

    // build VM

    let mut globals = LVar::create_global_constants();
    globals.extend([
        // GPIO pin constants
        (u16str!("@pinLED").into(), LVar::Constant(25.into())),
    ]);

    let gpio_data = Rc::new(RefCell::new(BuildingData::Memory(Box::new([0.; 30]))));
    let uart0_data = Rc::new(RefCell::new(BuildingData::Message(U16String::new())));
    let serial_data = Rc::new(RefCell::new(BuildingData::Message(U16String::new())));

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
                    ProcessorLinkConfig {
                        name: "serial".into(),
                        x: 3,
                        y: 0,
                    },
                ],
                instruction_hook: None,
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
        Building {
            block: &custom_content::SERIAL,
            position: PackedPoint2 { x: 3, y: 0 },
            data: serial_data.clone(),
        },
    ]);
    let vm = builder.build_with_globals(Cow::Owned(globals)).unwrap();

    // run!

    let mut usb_connected = false;

    let start = Instant::now();
    loop {
        vm.do_tick_with_delta(start.elapsed().into(), 1.0);

        // GPIO outputs
        if let BuildingData::Memory(memory) = &*gpio_data.borrow() {
            led.set_level((memory[25] != 0.).into());
        }

        // printflush uart0
        if let BuildingData::Message(message) = &mut *uart0_data.borrow_mut()
            && !message.is_empty()
        {
            let mut buf = [0; 4];
            for c in message.chars_lossy() {
                uart0
                    .blocking_write(c.encode_utf8(&mut buf).as_bytes())
                    .unwrap();
            }
            message.clear();
        }

        // printflush serial
        let message = if let BuildingData::Message(message) = &mut *serial_data.borrow_mut()
            && !message.is_empty()
        {
            let m = message.to_string_lossy();
            message.clear();
            Some(m)
        } else {
            None
        };
        if let Some(message) = message {
            if !usb_connected {
                usb_sender.wait_connection().await;
                usb_connected = true;
            }
            usb_sender.write_all(message.as_bytes()).await.unwrap();
            if message.len() % MAX_USB_PACKET_SIZE == 0 {
                usb_sender.write_packet(&[]).await.unwrap();
            }
        }

        // let other threads do things before we continue
        yield_now().await;
    }
}

fn leak<T>(value: T) -> &'static mut T {
    Box::leak(Box::new(value))
}
