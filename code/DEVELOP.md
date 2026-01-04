# Setup build environment

1. Install `rustup`.
   MacOS: `brew install rustup`
   Debian/Ubuntu: `apt install rustup` (?)
2. Install toolchain: `rustup install`.
3. Install linker: `cargo install flip-link`.
4. Install 'serial [port] pretty printer': `cargo install defmt-print`.
5. Set default toolchains: `rustup default stable`
6. Install `cargo-outdated` to check for updated dependencies.

# Build apps

1. `cargo build --verbose --profile dev`
   Available profiles: dev, release, release-dev

# Write image to Pico

1. Link the binary `ln -sf target/thumbv6m-none-eabi/<profile>/<binary> target.elf`
   Binaries: prepare-flash, read_config, set-valet-mode,
             unset-valet-mode, set-password, set-fingerprint,
             read-actuator-pot, move-actuator_forward,
             move-actuator_backward, test-actuator,
             drive-by-wire
2. Write the binary to the RaspberryPi Pico.
   ```
   openocd -f interface/cmsis-dap.cfg \
       -f target/rp2040.cfg -c "adapter speed 5000" \
       -s tcl -c "program target.elf verify reset exit"`
   ```
   I have this function defined in my `${HOME}/.bashrc`:
   ```
   picoload() {
       target="${1:-target.elf}"
       [ "${target}" != "target.elf" ] && ln -sf "${target}" "target.elf"
       openocd -f interface/cmsis-dap.cfg -f target/rp2040.cfg -c 'adapter speed 5000' \
            -c "program \"${target}\" verify reset exit"
   }
   ```
   That way, I can run `picoload` or `picoload target/thumbv6m-none-eabi/debug/drive-by-wire`
   to write the program to the pico.

3. Listen on the serial port for debug messages and communications.
   `bin/listen-serial.sh`
   NOTE: `socat` is broken under MacOS, and can't be used for this.
