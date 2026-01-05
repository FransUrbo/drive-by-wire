use defmt::{info, error};

use embassy_rp::flash::{Async as FlashAsync, Flash};
use embassy_rp::peripherals::FLASH;
use embassy_sync::channel::{Channel, Receiver};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, blocking_mutex::raw::NoopRawMutex, mutex::Mutex,
};

// External "defines".
use crate::Button;
use crate::BUTTONS_BLOCKED;
use crate::BUTTON_ENABLED;

use crate::lib_config;
use crate::DbwConfig;
use crate::FLASH_SIZE;

use actuator::{Actuator, GearModes};

pub static CHANNEL_ACTUATOR: Channel<CriticalSectionRawMutex, Button, 64> = Channel::new();
type FlashMutex = Mutex<NoopRawMutex, Flash<'static, FLASH, FlashAsync, FLASH_SIZE>>;

// Control the actuator. Calculate in what direction and how much to move it to get to
// the desired drive mode position.
#[embassy_executor::task]
pub async fn actuator_control(
    receiver: Receiver<'static, CriticalSectionRawMutex, Button, 64>,
    flash: &'static FlashMutex,
    mut actuator: Actuator<'static>,
) {
    info!("Started actuator control task");

    loop {
        // Block waiting for button press.
        let button = receiver.receive().await;

        // Move the actuator to the gear mode selected.
        actuator
            .change_gear_mode(GearModes::from(Button::from_button(button)))
            .await;

        // Now that we're done moving the actuator, Enable reading buttons again.
        unsafe { BUTTONS_BLOCKED = false };

        // .. and update the button enabled.
        unsafe { BUTTON_ENABLED = button };

        // .. and write it to flash.
        {
            // Read the existing values from the flash.
            let mut flash = flash.lock().await;
            let mut config = match DbwConfig::read(&mut flash) {
                // Read the old/current values.
                Ok(config) => config,
                Err(e) => {
                    error!("Failed to read flash: {:?}", e);
                    lib_config::resonable_defaults()
                }
            };

            // Set new value.
            config.active_button = button;

            // Write the config to flash.
            lib_config::write_flash(&mut flash, config).await;
        }
    }
}
