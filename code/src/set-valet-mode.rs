//! This example test the flash connected to the RP2040 chip.

//! This was taken directly from https://github.com/embassy-rs/embassy/blob/0cbdd8b9c84511eb2a9b7065fecb0f56a9a255d2/examples/rp/src/bin/flash.rs.
//! I just modified it a little to fit *my* purpose better.
//! The read and write functionality was removed, don't need it.
//! I just need it to clear the flash area I'm using in the main app.

#![no_std]
#![no_main]

use defmt::{error, info};

use embassy_executor::Spawner;
use embassy_rp::flash::Async;

pub mod config;
use crate::config::*;

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Setting valet mode in flash");

    // Instantiate the flash.
    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    // Read old values.
    match DbwConfig::read(&mut flash) {
	Ok(mut config)  => {
	    // Set the valet mode to 1 (true).
	    config.valet_mode = 1;

	    // Write flash.
	    config::write_flash(&mut flash, config).await;
	}
	Err(e) => error!("Failed to read flash: {:?}", e)
    }

    loop {}
}
