use defmt::{debug, error};

use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{Input, Output, SlewRate};
use embassy_rp::peripherals::FLASH;
use embassy_sync::channel::{Channel, Receiver};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, blocking_mutex::raw::NoopRawMutex, mutex::Mutex,
};

use actuator::*;

// External "defines".
use crate::Button;
use crate::BUTTONS_BLOCKED;
use crate::BUTTON_ENABLED;

use crate::lib_config;
use crate::DbwConfig;
use crate::FLASH_SIZE;

pub static CHANNEL_ACTUATOR: Channel<CriticalSectionRawMutex, Button, 64> = Channel::new();
type FlashMutex = Mutex<NoopRawMutex, Flash<'static, FLASH, Async, FLASH_SIZE>>;


// Control the actuator. Calculate in what direction and how much to move it to get to
// the desired drive mode position.
#[embassy_executor::task]
pub async fn actuator_control(
    receiver: Receiver<'static, CriticalSectionRawMutex, Button, 64>,
    flash: &'static FlashMutex,
    mut pin_motor_plus: Output<'static>,
    mut pin_motor_minus: Output<'static>,
    _pin_pot: Input<'static>,
) {
    debug!("Started actuator control task");

    pin_motor_plus.set_slew_rate(SlewRate::Fast);
    pin_motor_minus.set_slew_rate(SlewRate::Fast);

    loop {
        let button = receiver.receive().await; // Block waiting for data.

        // TODO: Figure out what gear is in by reading the actuator potentiometer.
        // NOTE: This might not be possible, the `get_level()` only gets the level (HIGH/LOW) of the pin,
        //       not the actual value from the potentiometer.
        //let _actuator_position = pin_pot.get_level();

        // FAKE: Use the current button selected to calculate the direction and amount to move the actuator
        let _actuator_position = unsafe { BUTTON_ENABLED as u8 };

        // Calculate the direction to move, based on current position and where we want to go.
        // - => Higher gear - BACKWARDS
        // + => Lower gear  - FORWARD
        let amount: i8 =
            (_actuator_position as i8 - button as i8) * ACTUATOR_DISTANCE_BETWEEN_POSITIONS;
        debug!(
            "Move direction and amount => '{} - {} = {}'",
            _actuator_position, button as i8, amount
        );

        // Move the actuator.
        actuator::move_actuator(amount, &mut pin_motor_plus, &mut pin_motor_minus).await;

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
