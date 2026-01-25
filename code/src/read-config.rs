#![no_std]
#![no_main]

//! This example test the flash connected to the RP2040 chip.

//! This was taken directly from https://github.com/embassy-rs/embassy/blob/0cbdd8b9c84511eb2a9b7065fecb0f56a9a255d2/examples/rp/src/bin/flash.rs.
//! I just modified it a little to fit *my* purpose better.
//! The read and write functionality was removed, don't need it.
//! I just need it to clear the flash area I'm using in the main app.

use defmt::info;

use embassy_executor::Spawner;

pub mod lib_actuator;
pub mod lib_buttons;
pub mod lib_can_bus;
pub mod lib_config;
pub mod lib_resources;

use crate::lib_buttons::Button;
use crate::lib_config::{FlashConfigMessages, CHANNEL_FLASH};

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Reading the content of the flash");

    CHANNEL_FLASH.send(FlashConfigMessages::ReadValet).await;
    let valet_mode = CHANNEL_FLASH.receive().await;
    info!("Valet mode: {}", FlashConfigMessages::to_valet(valet_mode));

    CHANNEL_FLASH.send(FlashConfigMessages::ReadButton).await;
    let active_button = CHANNEL_FLASH.receive().await;
    info!(
        "Active button: {}",
        FlashConfigMessages::to_button(&active_button)
    );

    #[allow(clippy::empty_loop)]
    loop {}
}
