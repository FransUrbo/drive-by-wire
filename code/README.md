# Setup environment

``` shell
brew tap probe-rs/probe-rs
brew install arm-none-eabi-gdb openocd expect probe-rs
rustup-init
cargo install thumbv6m-none-eabi cargo-binutils cargo-generate defmt-print flip-link \
      probe-run elf2uf2-rs
rustup target add thumbv6m-none-eabi
```

# Build code

## Profile: dev (default)

``` shell
cargo build 2>&1 | tee build.log
```

## Profile: release

``` shell
cargo build --release 2>&1 | tee build.log
```

# Run main app

## Debug mode

``` shell
cargo run --bin drive-by-wire  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```

## Debug in realtime with gdb

Run the following command in one shell:
``` shell
openocd -f interface/cmsis-dap.cfg -f target/rp2040.cfg -c 'adapter speed 5000'
```

... and this command in another:
``` shell
arm-none-eabi-gdb -q -x openocd.gdb target.elf
```

## Load built code

``` shell
alias picoprobe="openocd -f interface/cmsis-dap.cfg -f target/rp2040.cfg -c 'adapter speed 5000'"
alias picoload="picoprobe -c 'program target.elf verify reset exit'"
picoload
```

# Valet mode

## Set valet mode

``` shell
cargo run --bin set-valet-mode  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```

## Unset valet mode

``` shell
cargo run --bin unset-valet-mode  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```

# Read config

``` shell
cargo run --bin read_config  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```
