#![no_std]
#![no_main]

//! This example test the flash connected to the RP2040 chip.

//! This was taken directly from https://github.com/embassy-rs/embassy/blob/0cbdd8b9c84511eb2a9b7065fecb0f56a9a255d2/examples/rp/src/bin/flash.rs.
//! I just modified it a little to fit *my* purpose better.
//! The read and write functionality was removed, don't need it.
//! I just need it to clear the flash area I'm using in the main app.

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{
    flash::{Async, Flash, ERASE_SIZE, FLASH_BASE},
    peripherals::FLASH,
};

use {defmt_rtt as _, panic_probe as _};

pub mod lib_actuator;
pub mod lib_buttons;
pub mod lib_can_bus;
pub mod lib_resources;
pub mod lib_config;

use crate::lib_buttons::Button;
use crate::lib_resources::{
    AssignedResources, PeriActuator, PeriBuiltin, PeriButtons, PeriFPScanner, PeriFlash,
    PeriNeopixel, PeriSerial, PeriStart, PeriSteering, PeriWatchdog, ADDR_OFFSET, FLASH_SIZE,
};
use crate::lib_config::init_flash;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let r = split_resources! {p};

    info!("Hello World!");

    // Instantiate the flash.
    let flash = init_flash(r.flash);

    {
        let mut flash = flash.lock().await;
        erase_write_sector(&mut flash);
    }

    #[allow(clippy::empty_loop)]
    loop {}
}

fn erase_write_sector(flash: &mut Flash<'_, FLASH, Async, FLASH_SIZE>) {
    info!(">>>> [erase_write_sector]");
    let mut buf = [0u8; ERASE_SIZE];

    // READ initial state
    defmt::unwrap!(flash.blocking_read(ADDR_OFFSET + ERASE_SIZE as u32, &mut buf));

    info!(
        "Addr of flash block is {:x}",
        ADDR_OFFSET + ERASE_SIZE as u32 + FLASH_BASE as u32
    );
    info!("Contents start with {=[u8]}", buf[0..4]);

    // ERASE flash area.
    defmt::unwrap!(flash.blocking_erase(
        ADDR_OFFSET + ERASE_SIZE as u32,
        ADDR_OFFSET + ERASE_SIZE as u32 + ERASE_SIZE as u32
    ));

    // READ after erase.
    defmt::unwrap!(flash.blocking_read(ADDR_OFFSET + ERASE_SIZE as u32, &mut buf));
    info!("Contents after erase starts with {=[u8]}", buf[0..4]);
    if buf.iter().any(|x| *x != 0xFF) {
        defmt::panic!("unexpected (1)");
    }

    // For the drive-by-wire, we need this to be '0' => initial mode (P)ark. Might not
    // be exactly what we want in the end, but works for now during development simulations.
    for b in buf.iter_mut() {
        *b = 0x00;
    }

    defmt::unwrap!(flash.blocking_write(ADDR_OFFSET + ERASE_SIZE as u32, &buf));

    defmt::unwrap!(flash.blocking_read(ADDR_OFFSET + ERASE_SIZE as u32, &mut buf));
    info!("Contents after write starts with {=[u8]}", buf[0..4]);
    if buf.iter().any(|x| *x != 0x00) {
        defmt::panic!("unexpected (2)");
    }
}
