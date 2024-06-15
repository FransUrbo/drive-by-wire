use defmt::{debug, error, info, trace, Format};

use embassy_executor::Spawner;
use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pull};
use embassy_rp::peripherals::FLASH;
use embassy_sync::channel::{Channel, Receiver};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, blocking_mutex::raw::NoopRawMutex, mutex::Mutex,
};
use embassy_time::{with_deadline, Duration, Instant, Timer};

type FlashMutex = Mutex<NoopRawMutex, Flash<'static, FLASH, Async, FLASH_SIZE>>;

use debounce;

// External "defines".
use crate::CANMessage;
use crate::CHANNEL_ACTUATOR;
use crate::CHANNEL_CANWRITE;

use crate::lib_config;
use crate::DbwConfig;
use crate::FLASH_SIZE;

#[derive(Copy, Clone, Format, PartialEq)]
#[repr(u8)]
pub enum Button {
    P,
    R,
    N,
    D,
    UNSET,
}
pub enum LedStatus {
    On,
    Off,
}

// Setup the communication channels between the tasks.
pub static CHANNEL_P: Channel<CriticalSectionRawMutex, LedStatus, 64> = Channel::new();
pub static CHANNEL_N: Channel<CriticalSectionRawMutex, LedStatus, 64> = Channel::new();
pub static CHANNEL_R: Channel<CriticalSectionRawMutex, LedStatus, 64> = Channel::new();
pub static CHANNEL_D: Channel<CriticalSectionRawMutex, LedStatus, 64> = Channel::new();

// Start with the button UNSET, then change it when we know what gear the car is in.
pub static mut BUTTON_ENABLED: Button = Button::UNSET;

// When the actuator is moving, we need to set this to `true` to block input.
pub static mut BUTTONS_BLOCKED: bool = false;

// Control the drive button LEDs - four buttons, four LEDs.
#[embassy_executor::task(pool_size = 4)]
async fn set_led(
    receiver: Receiver<'static, CriticalSectionRawMutex, LedStatus, 64>,
    led_pin: AnyPin,
) {
    debug!("Started button LED control task");

    let mut led = Output::new(led_pin, Level::Low); // Always start with the LED off.

    loop {
        match receiver.receive().await {
            // Block waiting for data.
            LedStatus::On => led.set_high(),
            LedStatus::Off => led.set_low(),
        }
    }
}

// Listen for button presses - four buttons, one task each.
#[embassy_executor::task(pool_size = 4)]
pub async fn read_button(
    spawner: Spawner,
    flash: &'static FlashMutex,
    button: Button,
    btn_pin: AnyPin,
    led_pin: AnyPin,
) {
    debug!("Started button control task");

    let mut btn =
        debounce::Debouncer::new(Input::new(btn_pin, Pull::Up), Duration::from_millis(50));

    // Spawn off a LED driver for this button.
    match button {
        Button::UNSET => (), // Should be impossible, but just to make the compiler happy.
        Button::P => spawner
            .spawn(set_led(CHANNEL_P.receiver(), led_pin))
            .unwrap(),
        Button::N => spawner
            .spawn(set_led(CHANNEL_N.receiver(), led_pin))
            .unwrap(),
        Button::R => spawner
            .spawn(set_led(CHANNEL_R.receiver(), led_pin))
            .unwrap(),
        Button::D => spawner
            .spawn(set_led(CHANNEL_D.receiver(), led_pin))
            .unwrap(),
    }

    loop {
        // NOTE: We need to wait for a button to be pressed, BEFORE we can check if we're
        //       blocked. If we don't, we've checked if we're blocked, we're not and we
        //       start listening to a button. But if someone else have got the press,
        //       then "this" will still wait for a press!
        //       If we ARE blocked, then sleep until we're not, then restart the loop from
        //       the beginning, listening for a button press again.
        btn.debounce().await; // Button pressed

        if unsafe { BUTTONS_BLOCKED } {
            debug!("Buttons blocked (button task: {})", button as u8);

            if unsafe { BUTTON_ENABLED == Button::N } && button == Button::D {
                debug!("Both 'N' and 'D' pressed - toggling Valet Mode");

                {
                    // Lock the flash and read old values.
                    let mut flash = flash.lock().await;
                    let mut config = match DbwConfig::read(&mut flash) {
                        // Read the old/current values.
                        Ok(config) => config,
                        Err(e) => {
                            error!("Failed to read flash: {:?}", e);
                            lib_config::resonable_defaults()
                        }
                    };

                    // Toggle Valet Mode.
                    if config.valet_mode {
                        CHANNEL_CANWRITE.send(CANMessage::DisableValetMode).await;
                        config.valet_mode = false;
                    } else {
                        CHANNEL_CANWRITE.send(CANMessage::EnableValetMode).await;
                        config.valet_mode = true;
                    }
                    lib_config::write_flash(&mut flash, config).await;
                }

                unsafe { BUTTONS_BLOCKED = false };
            }

            while unsafe { BUTTONS_BLOCKED } {
                // Block here while we wait for it to be unblocked.
                debug!("Waiting for unblock (button task: {})", button as u8);
                Timer::after_secs(1).await;
            }
            continue;
        }

        // Disable reading buttons as soon as possible, while we deal with this one.
        // If this isn't "us", then the buttons will be re-enabled in the actuator task,
        // once the actuator have finished moving..
        // If this IS "us", we re-enable the buttons again after we've blinked "our" LED.
        unsafe { BUTTONS_BLOCKED = true };

        // Got a valid button press. Process it..
        let start = Instant::now();
        info!("Button press detected (button task: {})", button as u8);

        // Don't really care how long a button have been pressed as,
        // the `debounce()` will detect when it's been RELEASED.
        match with_deadline(start + Duration::from_secs(1), btn.debounce()).await {
            Ok(_) => {
                trace!("Button pressed for: {}ms", start.elapsed().as_millis());
                debug!(
                    "Button enabled: {}; Button pressed: {}",
                    unsafe { BUTTON_ENABLED as u8 },
                    button as u8
                );

                // We know who WE are, so turn ON our own LED and turn off all the other LEDs.
                match button {
                    Button::UNSET => (),
                    Button::P => {
                        if unsafe { button == BUTTON_ENABLED } {
                            // Already enabled => blink *our* LED three times.
                            for _i in 0..3 {
                                CHANNEL_P.send(LedStatus::Off).await;
                                Timer::after_millis(500).await;
                                CHANNEL_P.send(LedStatus::On).await;
                                Timer::after_millis(500).await;
                            }

                            unsafe { BUTTONS_BLOCKED = false };
                        } else {
                            CHANNEL_P.send(LedStatus::On).await;
                            CHANNEL_N.send(LedStatus::Off).await;
                            CHANNEL_R.send(LedStatus::Off).await;
                            CHANNEL_D.send(LedStatus::Off).await;

                            // Trigger the actuator to switch to (P)ark.
                            CHANNEL_ACTUATOR.send(Button::P).await;
                        }

                        continue;
                    }
                    Button::N => {
                        if unsafe { button == BUTTON_ENABLED } {
                            // Already enabled => blink *our* LED three times.
                            for _i in 0..3 {
                                CHANNEL_N.send(LedStatus::Off).await;
                                Timer::after_millis(500).await;
                                CHANNEL_N.send(LedStatus::On).await;
                                Timer::after_millis(500).await;
                            }

                            unsafe { BUTTONS_BLOCKED = false };
                        } else {
                            CHANNEL_P.send(LedStatus::Off).await;
                            CHANNEL_N.send(LedStatus::On).await;
                            CHANNEL_R.send(LedStatus::Off).await;
                            CHANNEL_D.send(LedStatus::Off).await;

                            // Trigger the actuator to switch to (N)eutral.
                            CHANNEL_ACTUATOR.send(Button::N).await;
                        }
                        continue;
                    }
                    Button::R => {
                        if unsafe { button == BUTTON_ENABLED } {
                            // Already enabled => blink *our* LED three times.
                            for _i in 0..3 {
                                CHANNEL_R.send(LedStatus::Off).await;
                                Timer::after_millis(500).await;
                                CHANNEL_R.send(LedStatus::On).await;
                                Timer::after_millis(500).await;
                            }

                            unsafe { BUTTONS_BLOCKED = false };
                        } else {
                            CHANNEL_P.send(LedStatus::Off).await;
                            CHANNEL_N.send(LedStatus::Off).await;
                            CHANNEL_R.send(LedStatus::On).await;
                            CHANNEL_D.send(LedStatus::Off).await;

                            // Trigger the actuator to switch to (R)everse.
                            CHANNEL_ACTUATOR.send(Button::R).await;
                        }

                        continue;
                    }
                    Button::D => {
                        if unsafe { button == BUTTON_ENABLED } {
                            // Already enabled => blink *our* LED three times.
                            for _i in 0..3 {
                                CHANNEL_D.send(LedStatus::Off).await;
                                Timer::after_millis(500).await;
                                CHANNEL_D.send(LedStatus::On).await;
                                Timer::after_millis(500).await;
                            }

                            unsafe { BUTTONS_BLOCKED = false };
                        } else {
                            CHANNEL_P.send(LedStatus::Off).await;
                            CHANNEL_N.send(LedStatus::Off).await;
                            CHANNEL_R.send(LedStatus::Off).await;
                            CHANNEL_D.send(LedStatus::On).await;

                            // Trigger the actuator to switch to (D)rive.
                            CHANNEL_ACTUATOR.send(Button::D).await;
                        }

                        continue;
                    }
                }
            }

            // Don't allow another button for quarter second.
            _ => Timer::after_millis(250).await,
        }

        // Wait for button release before handling another press.
        btn.debounce().await;
        trace!("Button pressed for: {}ms", start.elapsed().as_millis());
    }
}
