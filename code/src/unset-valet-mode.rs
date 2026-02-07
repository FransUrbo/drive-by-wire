#![no_std]
#![no_main]

//! This example test the flash connected to the RP2040 chip.

//! This was taken directly from https://github.com/embassy-rs/embassy/blob/0cbdd8b9c84511eb2a9b7065fecb0f56a9a255d2/examples/rp/src/bin/flash.rs.
//! I just modified it a little to fit *my* purpose better.
//! The read and write functionality was removed, don't need it.
//! I just need it to clear the flash area I'm using in the main app.

use defmt::{error, info};
use embassy_executor::Spawner;

pub mod lib_actuator;
pub mod lib_buttons;
pub mod lib_can_bus;
pub mod lib_config;
pub mod lib_resources;

use crate::lib_buttons::Button;
use crate::lib_config::{init_flash, DbwConfig};
use crate::lib_resources::{
    AssignedResources, PeriActuator, PeriBuiltin, PeriButtons, PeriCan, PeriEis, PeriFPScanner,
    PeriFlash, PeriNeopixel, PeriPowerMonitor, PeriSerial, PeriWatchdog,
};

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let r = split_resources! {p};

    info!("Unsetting valet mode in flash");

    // Instantiate the flash.
    let flash = init_flash(r.flash);

    // Read old values.
    let mut flash = flash.lock().await;
    match DbwConfig::read(&mut flash) {
        Ok(mut config) => {
            // Set the valet mode to false.
            config.valet_mode = false;

            // Write flash.
            lib_config::write_flash(&mut flash, config).await;
        }
        Err(e) => error!("Failed to read flash: {:?}", e),
    }

    #[allow(clippy::empty_loop)]
    loop {}
}
