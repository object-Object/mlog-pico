use mindy::{multistr, types::content::Block};

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

pub static SERIAL: Block = Block {
    name: multistr!("serial"),
    id: -4,
    ..DEFAULT
};

pub static ST7789VW_DISPLAY: Block = Block {
    name: multistr!("st7789vw-display"),
    id: -5,
    ..DEFAULT
};
