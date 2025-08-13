use embassy_rp::gpio::{Flex, Pull};
use mindustry_rs::{
    types::LAccess,
    vm::{CustomBuildingData, InstructionResult, LValue, LogicVM, ProcessorState},
};

pub struct GpioData<'a> {
    pins: [Option<Flex<'a>>; 30],
}

impl<'a> GpioData<'a> {
    pub fn new<T>(values: T) -> Self
    where
        T: IntoIterator<Item = (usize, Flex<'a>)>,
    {
        let mut pins = [const { None }; 30];

        for (i, pin) in values.into_iter() {
            if pins[i].is_some() {
                panic!("duplicate pin id: {i}");
            }
            pins[i] = Some(pin);
        }

        Self { pins }
    }
}

impl CustomBuildingData for GpioData<'_> {
    fn read(&mut self, _: &mut ProcessorState, _: &LogicVM, address: LValue) -> Option<LValue> {
        if let Ok(i) = address.num_usize()
            && let Some(Some(pin)) = self.pins.get_mut(i)
        {
            pin.set_as_input();
            Some(bool::from(pin.get_level()).into())
        } else {
            Some(LValue::NULL)
        }
    }

    fn write(
        &mut self,
        _: &mut ProcessorState,
        _: &LogicVM,
        address: LValue,
        value: LValue,
    ) -> InstructionResult {
        if let Ok(i) = address.num_usize()
            && let Some(Some(pin)) = self.pins.get_mut(i)
        {
            pin.set_pull(if value == LValue::NULL {
                Pull::None
            } else if value.bool() {
                Pull::Up
            } else {
                Pull::Down
            });
            pin.set_level(value.bool().into());
            pin.set_as_output();
        }
        InstructionResult::Ok
    }

    fn sensor(&mut self, _: &mut ProcessorState, _: &LogicVM, sensor: LAccess) -> Option<LValue> {
        Some(match sensor {
            LAccess::MemoryCapacity => self.pins.len().into(),
            _ => return None,
        })
    }
}

macro_rules! gpio_data_pin {
    ($pin:expr) => {
        ($pin.pin() as usize, embassy_rp::gpio::Flex::new($pin))
    };
}
pub(crate) use gpio_data_pin;
