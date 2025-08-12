use alloc::{rc::Rc, string::String};
use core::cell::{Cell, RefCell};

use embassy_executor::SpawnToken;
use embassy_futures::yield_now;
use embassy_rp::{peripherals::USB, usb};
use embassy_usb::class::cdc_acm::{self, CdcAcmClass};
use heapless::Deque;
use mindustry_rs::{
    logic::vm::{
        CustomBuildingData, LValue, LogicVM, ProcessorState, instructions::InstructionResult,
    },
    types::LAccess,
};

use crate::MAX_USB_PACKET_SIZE;

#[embassy_executor::task]
async fn serial_data_task(
    mut rx: cdc_acm::Receiver<'static, usb::Driver<'static, USB>>,
    rx_buf: Rc<RefCell<Deque<u8, MAX_USB_PACKET_SIZE>>>,
) {
    let mut buf = [0; MAX_USB_PACKET_SIZE];
    rx.wait_connection().await;
    loop {
        let n = rx.read_packet(&mut buf).await.unwrap();
        let data = &buf[..n];

        while !rx_buf.borrow().is_empty() {
            yield_now().await;
        }

        let mut queue = rx_buf.borrow_mut();
        for &item in data {
            queue.push_back(item).unwrap();
        }
    }
}

pub struct SerialData {
    tx_buf: Rc<RefCell<Option<String>>>,
    rx_buf: Rc<RefCell<Deque<u8, MAX_USB_PACKET_SIZE>>>,
}

impl SerialData {
    pub fn new(
        class: CdcAcmClass<'static, usb::Driver<'static, USB>>,
    ) -> (Self, SpawnToken<impl Sized>, impl AsyncFnMut()) {
        let (mut tx, rx) = class.split();

        let tx_buf = Rc::new(RefCell::new(None));
        let rx_buf = Rc::new(RefCell::new(Deque::new()));

        let is_connected = Cell::new(false);

        (
            Self {
                tx_buf: tx_buf.clone(),
                rx_buf: rx_buf.clone(),
            },
            serial_data_task(rx, rx_buf),
            async move || {
                if let Some(message) = tx_buf.replace(None) {
                    if !is_connected.get() {
                        tx.wait_connection().await;
                        is_connected.set(true);
                    }

                    let n = message.len().min(MAX_USB_PACKET_SIZE);
                    tx.write_packet(&message.as_bytes()[..n]).await.unwrap();
                    if n == MAX_USB_PACKET_SIZE {
                        tx.write_packet(&[]).await.unwrap();
                    }
                }
            },
        )
    }
}

impl CustomBuildingData for SerialData {
    fn read(&mut self, _: &mut ProcessorState, _: &LogicVM, address: LValue) -> Option<LValue> {
        let mut buf = self.rx_buf.borrow_mut();
        if let Ok(mut i) = address.num_usize()
            && i < buf.len()
        {
            // if we read from address i > 0 in the queue, discard all values before i
            while i > 0 {
                buf.pop_front();
                i -= 1;
            }

            Some(buf.pop_front().into())
        } else {
            Some(LValue::NULL)
        }
    }

    fn printflush(&mut self, state: &mut ProcessorState, _: &LogicVM) -> InstructionResult {
        self.tx_buf
            .replace(Some(state.printbuffer.to_string_lossy()));
        InstructionResult::Yield
    }

    fn sensor(&mut self, _: &mut ProcessorState, _: &LogicVM, sensor: LAccess) -> Option<LValue> {
        Some(match sensor {
            LAccess::MemoryCapacity => MAX_USB_PACKET_SIZE.into(),
            LAccess::BufferSize => self.rx_buf.borrow().len().into(),
            _ => return None,
        })
    }
}
