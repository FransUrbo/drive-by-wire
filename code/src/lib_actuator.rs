use defmt::{debug, error};

//use embassy_rp::adc::AdcPin;
//use embassy_rp::gpio::AnyPin;

use embassy_rp::flash::{Async as FlashAsync, Flash};
use embassy_rp::peripherals::{FLASH, PIN_26};
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

use actuator::Actuator;

pub static CHANNEL_ACTUATOR: Channel<CriticalSectionRawMutex, Button, 64> = Channel::new();
type FlashMutex = Mutex<NoopRawMutex, Flash<'static, FLASH, FlashAsync, FLASH_SIZE>>;

// Control the actuator. Calculate in what direction and how much to move it to get to
// the desired drive mode position.
#[embassy_executor::task]
pub async fn actuator_control(
    receiver: Receiver<'static, CriticalSectionRawMutex, Button, 64>,
    flash: &'static FlashMutex,
    mut actuator: Actuator<'static, PIN_26>, //AnyPin>,
) {
    debug!("Started actuator control task");

    // TODO: Get the real value (in Ω).
    // If we create a command to calibrate the actuator, we can find this value and store it in the config.
    // We already know how many Ω it takes to move the actuator 1mm..
    static ACTUATOR_START_POSITION: i16 = 1200;

    loop {
        let button = receiver.receive().await; // Block waiting for data.

        // Get the current gear from the actuator.
        let current_gear = actuator.find_gear(ACTUATOR_START_POSITION).await;

        // Calculate the amount of gears and direction to move.
        // - => Higher gear - BACKWARDS
        // + => Lower gear  - FORWARD
        let amount: i8 = current_gear as i8 - button as i8;

        // Move the actuator.
        actuator.move_actuator(amount).await;

        // Now that we're done moving the actuator, Enable reading buttons again.
        unsafe { BUTTONS_BLOCKED = false };

        // .. and update the button enabled.
        unsafe { BUTTON_ENABLED = button };

        // .. and write it to flash.
        {
            let mut flash = flash.lock().await;
            let mut config = match DbwConfig::read(&mut flash) {
                // Read the old/current values.
                Ok(config) => config,
                Err(e) => {
                    error!("Failed to read flash: {:?}", e);
                    lib_config::resonable_defaults()
                }
            };
            config.active_button = button; // Set new value.
            lib_config::write_flash(&mut flash, config).await;
        }
    }
}
