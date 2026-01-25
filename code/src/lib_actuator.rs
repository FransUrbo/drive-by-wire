use defmt::info;

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};

// External "defines".
use crate::lib_buttons::{Button, BUTTONS_BLOCKED, BUTTON_ENABLED};
use crate::lib_config::{FlashConfigMessages, CHANNEL_FLASH};

use actuator::Actuator;

pub static CHANNEL_ACTUATOR: Channel<CriticalSectionRawMutex, Button, 64> = Channel::new();

// Control the actuator. Calculate in what direction and how much to move it to get to
// the desired drive mode position.
#[embassy_executor::task]
pub async fn actuator_control(
    receiver: Receiver<'static, CriticalSectionRawMutex, Button, 64>,
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

        // ... and update the flash.
        CHANNEL_FLASH.send(FlashConfigMessages::from(button)).await;
    }
}
