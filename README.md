# mlog-pico

Mindustry logic on a Raspberry Pi Pico.

## Running

https://github.com/rp-rs/rp-hal-boards/tree/5135e3dafe3e69b112e6d2d72cfb1856a7679b82/boards/rp-pico#general-instructions

### Program selection

Each file in `src/mlog` has a corresponding Cargo feature to select it. For example, to build mlog-pico with `src/mlog/print_usb.mlog`, run `cargo build --features print_usb`.

### Pico 1

```sh
rustup target add thumbv6m-none-eabi
cargo install --git https://github.com/object-Object/elf2uf2-rs --rev bbcf7458aa
cargo run --release --features print_usb  # or: cargo rr -F print_usb
```

### Pico 2 (UNTESTED)

```sh
rustup target add thumbv8m.main-none-eabihf
cargo install --git https://github.com/object-Object/elf2uf2-rs --rev bbcf7458aa
cargo run-pico2 --release --features print_usb  # or: cargo rr2 -F print_usb
```

See: https://github.com/JoNil/elf2uf2-rs/pull/39

I don't have a Pico 2 to test this on, so I don't know if this works or not. Feel free to give it a try :)

## VID/PID

The default VID/PID used by this repository is one of the the [pid.codes](https://pid.codes) Test PIDs. See https://pid.codes/1209/0001/ for more info.
