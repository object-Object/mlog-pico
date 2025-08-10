# mlog-pico

Mindustry logic on a Raspberry Pi Pico.

## Running

https://github.com/rp-rs/rp-hal-boards/tree/5135e3dafe3e69b112e6d2d72cfb1856a7679b82/boards/rp-pico#general-instructions

```
cargo install elf2uf2-rs
cargo run --release --features blink
cargo run --release --features print
cargo run --release --features print_usb
```

## VID/PID

The default VID/PID used by this repository is one of the the [pid.codes](https://pid.codes) Test PIDs. See https://pid.codes/1209/0001/ for more info.
