#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use core::{cell::RefCell, mem::MaybeUninit};

use embassy_embedded_hal::shared_bus::blocking::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embassy_rp::{
    bind_interrupts,
    gpio::{self, Pin},
    peripherals::{UART0, USB},
    spi::{self, Spi},
    uart::{self, BufferedUart},
    usb,
};
use embassy_sync::blocking_mutex::{Mutex, raw::NoopRawMutex};
use embassy_time::{Delay, Instant, Timer};
use embassy_usb::{
    UsbDevice,
    class::cdc_acm::{self, CdcAcmClass},
};
use embedded_alloc::TlsfHeap as Heap;
use embedded_graphics::draw_target::DrawTarget;
use embedded_io_async::Write;
use mindustry_rs::{
    parser::deserialize_ast,
    types::{PackedPoint2, ProcessorLinkConfig},
    vm::{Building, LVar, LogicVMBuilder, ProcessorBuilder, instructions::Instruction},
};
use mipidsi::{
    interface::SpiInterface,
    options::{ColorInversion, Orientation, Rotation},
};
use panic_persist::get_panic_message_bytes;
use widestring::u16str;

use self::{
    buildings::{DISPLAY_RESET_COLOR, DisplayData, GpioData, SerialData, UartData, gpio_data_pin},
    st7789vw::ST7789VW,
};

mod buildings;
mod custom_content;
mod st7789vw;

macro_rules! include_ast {
    ($name:expr) => {
        #[cfg(feature = $name)]
        const AST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mlog/", $name, ".bin"));
        #[cfg(feature = $name)]
        const PROGRAM_NAME: &str = $name;
    };
}

include_ast!("blink");
include_ast!("button_matrix");
include_ast!("draw");
include_ast!("mandelbrot");
include_ast!("print");
include_ast!("print_usb");

const HEAP_SIZE: usize = 64 * 1024;

#[global_allocator]
static HEAP: Heap = Heap::empty();

bind_interrupts!(struct Irqs {
    UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

const MAX_USB_PACKET_SIZE: usize = 64;
const UART_BUFFER_SIZE: usize = 400;

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
    let mut uart0 = BufferedUart::new(
        p.UART0,
        p.PIN_0,
        p.PIN_1,
        Irqs,
        &mut [0; UART_BUFFER_SIZE],
        &mut [0; UART_BUFFER_SIZE],
        uart_config,
    );

    // as soon as the UART is up, check if we panicked on the previous boot
    if let Some(msg) = get_panic_message_bytes() {
        uart0.write_all(msg).await.unwrap();
        uart0.flush().await.unwrap();
        Timer::after_secs(1).await;
        cortex_m::peripheral::SCB::sys_reset();
    }

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

    let mut serial_class = CdcAcmClass::new(
        &mut usb_builder,
        leak(cdc_acm::State::new()),
        MAX_USB_PACKET_SIZE as u16,
    );

    let usb = usb_builder.build();
    spawner.must_spawn(usb_task(usb));

    serial_class.wait_connection().await;

    // https://github.com/embassy-rs/embassy/blob/ac46e28c4b4f025279d8974adfb6120c6740e44e/examples/rp/src/bin/spi_display.rs
    let mut display_config = spi::Config::default();
    display_config.frequency = 64_000_000;
    display_config.phase = spi::Phase::CaptureOnSecondTransition;
    display_config.polarity = spi::Polarity::IdleHigh;

    // st7789v pins
    let din = p.PIN_11;
    let clk = p.PIN_10;
    let cs = p.PIN_9;
    let dc = p.PIN_8;
    let rst = p.PIN_12;
    let bl = p.PIN_13;

    let spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(RefCell::new(Spi::new_blocking_txonly(
        p.SPI1,
        clk,
        din,
        display_config,
    )));

    let display_spi = SpiDevice::new(leak(spi_bus), gpio::Output::new(cs, gpio::Level::High));

    let di = SpiInterface::new(
        display_spi,
        gpio::Output::new(dc, gpio::Level::Low),
        leak([0; 512]),
    );

    // disable backlight while initializing display so it doesn't show whatever was drawn on the previous boot
    let bl_pin = bl.pin();
    let mut bl = gpio::Flex::new(bl);
    bl.set_level(gpio::Level::Low);
    bl.set_as_output();

    let mut display = mipidsi::Builder::new(ST7789VW, di)
        .reset_pin(gpio::Output::new(rst, gpio::Level::Low))
        .orientation(Orientation::new().rotate(Rotation::Deg270))
        // inverted apparently means normal for this display (???)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut Delay)
        .unwrap();

    display.clear(DISPLAY_RESET_COLOR.into()).unwrap();
    bl.set_level(gpio::Level::High);

    // build VM

    let (uart0_data, mut uart0_tick) = UartData::new(uart0);

    let (serial_data, serial_task, mut serial_tick) = SerialData::new(serial_class);
    spawner.must_spawn(serial_task);

    let mut builder = LogicVMBuilder::new();

    builder.add_buildings([
        Building::from_processor_builder(
            &custom_content::PROCESSOR,
            PackedPoint2 { x: 0, y: 0 },
            ProcessorBuilder {
                ipt: 100.,
                privileged: true,
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
                    ProcessorLinkConfig {
                        name: "display1".into(),
                        x: 4,
                        y: 0,
                    },
                ],
                instruction_hook: Some(Box::new(|instruction, _, _| {
                    if let Instruction::Stop(_) = instruction {
                        #[cfg(feature = "pico1")]
                        embassy_rp::rom_data::reset_to_usb_boot(0, 0);

                        #[cfg(feature = "pico2")]
                        {
                            // REBOOT_TYPE_BOOTSEL
                            embassy_rp::rom_data::reboot(0x0002, 100, 0, 0);
                            loop {
                                core::hint::spin_loop();
                            }
                        }
                    }
                    None
                })),
            },
            &builder,
        ),
        Building::new(
            &custom_content::GPIO,
            PackedPoint2 { x: 1, y: 0 },
            GpioData::new([
                gpio_data_pin!(p.PIN_2),
                gpio_data_pin!(p.PIN_3),
                gpio_data_pin!(p.PIN_4),
                gpio_data_pin!(p.PIN_5),
                gpio_data_pin!(p.PIN_6),
                gpio_data_pin!(p.PIN_7),
                (bl_pin as usize, bl),
                gpio_data_pin!(p.PIN_14),
                gpio_data_pin!(p.PIN_15),
                gpio_data_pin!(p.PIN_16),
                gpio_data_pin!(p.PIN_17),
                gpio_data_pin!(p.PIN_18),
                gpio_data_pin!(p.PIN_19),
                gpio_data_pin!(p.PIN_20),
                gpio_data_pin!(p.PIN_21),
                gpio_data_pin!(p.PIN_22),
                gpio_data_pin!(p.PIN_25),
                gpio_data_pin!(p.PIN_26),
                gpio_data_pin!(p.PIN_27),
                gpio_data_pin!(p.PIN_28),
            ])
            .into(),
        ),
        Building::new(
            &custom_content::UART,
            PackedPoint2 { x: 2, y: 0 },
            uart0_data.into(),
        ),
        Building::new(
            &custom_content::SERIAL,
            PackedPoint2 { x: 3, y: 0 },
            serial_data.into(),
        ),
        Building::new(
            &custom_content::ST7789VW_DISPLAY,
            PackedPoint2 { x: 4, y: 0 },
            DisplayData::new(display).into(),
        ),
    ]);

    let mut globals = LVar::create_global_constants();
    globals.extend([
        // GPIO pin constants
        (
            u16str!("@pinBacklight").into(),
            LVar::Constant(bl_pin.into()),
        ),
        (u16str!("@pinLED").into(), LVar::Constant(25.into())),
    ]);

    let vm = builder.build_with_globals(&globals).unwrap();

    // run!

    let start = Instant::now();
    loop {
        vm.do_tick_with_delta(start.elapsed().into(), 1.0);

        uart0_tick().await;
        serial_tick().await;

        // let other threads do things before we continue
        yield_now().await;
    }
}

fn leak<T>(value: T) -> &'static mut T {
    Box::leak(Box::new(value))
}
