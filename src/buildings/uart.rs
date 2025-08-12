use alloc::{rc::Rc, string::String};
use core::cell::RefCell;

use embassy_rp::uart::{BufferedUart, BufferedUartRx};
use embedded_io::{Read, ReadReady};
use embedded_io_async::Write;
use mindustry_rs::{
    types::LAccess,
    vm::{CustomBuildingData, InstructionResult, LValue, LogicVM, ProcessorState},
};

use crate::UART_BUFFER_SIZE;

pub struct UartData {
    tx_buf: Rc<RefCell<Option<String>>>,
    rx: BufferedUartRx,
}

impl UartData {
    pub fn new(uart: BufferedUart) -> (Self, impl AsyncFnMut()) {
        let (mut tx, rx) = uart.split();
        let tx_buf = Rc::new(RefCell::new(None));
        (
            Self {
                tx_buf: tx_buf.clone(),
                rx,
            },
            async move || {
                if let Some(message) = tx_buf.replace(None) {
                    tx.write_all(message.as_bytes()).await.unwrap();
                }
            },
        )
    }
}

impl CustomBuildingData for UartData {
    fn read(&mut self, _: &mut ProcessorState, _: &LogicVM, address: LValue) -> Option<LValue> {
        let mut buf = [0; 1];
        if address.numi() == 0
            && let Ok(true) = self.rx.read_ready()
            && let Ok(n) = self.rx.read(&mut buf)
            && n > 0
        {
            Some(buf[0].into())
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
            LAccess::MemoryCapacity => UART_BUFFER_SIZE.into(),
            LAccess::BufferSize => if let Ok(true) = self.rx.read_ready() {
                1
            } else {
                0
            }
            .into(),
            _ => return None,
        })
    }
}
