//! This example test the flash connected to the RP2040 chip.

//! This was taken directly from https://github.com/embassy-rs/embassy/blob/0cbdd8b9c84511eb2a9b7065fecb0f56a9a255d2/examples/rp/src/bin/flash.rs.
//! I just modified it a little to fit *my* purpose better.
//! The read and write functionality was removed, don't need it.
//! I just need it to clear the flash area I'm using in the main app.

#![no_std]
#![no_main]

use defmt::{debug, info};
use embassy_executor::Spawner;
use embassy_rp::flash::{Async, ERASE_SIZE};
use embassy_rp::peripherals::FLASH;
use {defmt_rtt as _, panic_probe as _};

// offset from the flash start, NOT absolute address.
const ADDR_OFFSET: u32 = 0x100000;
const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Hello World!");

    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    write_flash(&mut flash, (ERASE_SIZE * 2) as u32, 0).await;

    loop {}
}

async fn write_flash(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>, offset: u32, buf: u8) {
    match flash.blocking_erase(
	ADDR_OFFSET + offset + ERASE_SIZE as u32,
	ADDR_OFFSET + offset + ERASE_SIZE as u32 + ERASE_SIZE as u32)
    {
	Ok(_)  => debug!("Flash erase successful"),
	Err(e) => info!("Flash erase failed: {}", e)
    }
    match flash.blocking_write(ADDR_OFFSET + offset + ERASE_SIZE as u32, &[buf]) {
	Ok(_)  => debug!("Flash write successful"),
	Err(e) => info!("Flash write failed: {}", e)
    }
}
