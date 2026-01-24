use defmt::{error, info};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};

// External "defines".
use crate::lib_buttons::{Button, BUTTONS_BLOCKED, BUTTON_ENABLED};
use crate::lib_config::{resonable_defaults, write_flash, DbwConfig, FlashMutex};

use actuator::Actuator;

pub static CHANNEL_ACTUATOR: Channel<CriticalSectionRawMutex, Button, 64> = Channel::new();

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

        // TODO: We need to check that we're not moving etc !!

        // Move the actuator to the gear mode selected.
        actuator.change_gear_mode(Button::to_gearmode(button)).await;

        // Now that we're done moving the actuator, Enable reading buttons again.
        unsafe { BUTTONS_BLOCKED = false };

        // .. and update the button enabled.
        unsafe { BUTTON_ENABLED = button };

        // .. and write it to flash.
        {
            // Read the existing values from the flash.
            // The flash lock is released when it goes out of scope.
            let mut flash = flash.lock().await;
            let mut config = match DbwConfig::read(&mut flash) {
                // Read the old/current values.
                Ok(config) => config,
                Err(e) => {
                    error!("Failed to read flash: {:?}", e);
                    resonable_defaults()
                }
            };

            // Set new value.
            config.active_button = button;

            // Write the config to flash.
            write_flash(&mut flash, config).await;
        }
    }
}
