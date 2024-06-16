use defmt::{debug, error, info, trace, Format};

use embassy_executor::Spawner;
use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{AnyPin, Input, Level, Output, Pull};
use embassy_rp::peripherals::{FLASH, UART0};
use embassy_sync::channel::{Channel, Receiver};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, blocking_mutex::raw::NoopRawMutex, mutex::Mutex,
};
use embassy_time::{with_deadline, Duration, Instant, Timer};

type FlashMutex = Mutex<NoopRawMutex, Flash<'static, FLASH, Async, FLASH_SIZE>>;
type ScannerMutex = Mutex<NoopRawMutex, r503::R503<'static, UART0>>;

use debounce;

// External "defines".
use crate::CANMessage;
use crate::CHANNEL_ACTUATOR;
use crate::CHANNEL_CANWRITE;

use crate::lib_config;
use crate::DbwConfig;
use crate::FLASH_SIZE;

use r503;

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
    fp_scanner: &'static ScannerMutex,
    button: Button,
    btn_pin: AnyPin,
    led_pin: AnyPin,
) {
    // Initialize the button listener.
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
    };
    debug!("Button::{}: Started button control task", button);

    loop {
        // NOTE: We need to wait for a button to be pressed, BEFORE we can check if we're
        //       blocked. If we don't, we've checked if we're blocked, we're not and we
        //       start listening to a button. But if someone else have got the press,
        //       then "this" will still wait for a press!
        //       If we ARE blocked, then sleep until we're not, then restart the loop from
        //       the beginning, listening for a button press again.
        btn.debounce().await; // Button pressed

        if unsafe { BUTTONS_BLOCKED } {
            debug!("Button::{}: Buttons blocked", button);

            if unsafe { BUTTON_ENABLED == Button::N } && button == Button::D {
                debug!(
                    "Button::{}: Both 'N' and 'D' pressed - toggling Valet Mode",
                    button
                );

                {
                    // Verify with a valid fingerprint that we're authorized to change Valet Mode.
		    let mut fp_scanner = fp_scanner.lock().await;
                    if fp_scanner.Wrapper_Verify_Fingerprint().await {
                        error!("Can't match fingerprint");

			// Give it five seconds before we retry.
			Timer::after_secs(5).await;

			// Turn off the aura.
			fp_scanner.Wrapper_AuraSet_Off().await;

			// Restart loop.
                        continue;
                    } else {
                        // Lock the flash and read old values.
                        let mut flash = flash.lock().await;
                        let mut config = match DbwConfig::read(&mut flash) {
                            Ok(config) => config,
                            Err(e) => {
                                error!("Button::{}: Failed to read flash: {:?}", button, e);
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
                }

                // Blink the 'D' LED three time, to indicate that both have been pressed.
                // The 'N' LED will be flashed three times further below..
                for _i in 0..3 {
                    CHANNEL_D.send(LedStatus::On).await;
                    Timer::after_millis(500).await;
                    CHANNEL_D.send(LedStatus::Off).await;
                    Timer::after_millis(500).await;
                }

                // Give it a second, so we don't *also* deal with the enabled button.
                // As in, let the button block "reach" the 'N' button task.
                Timer::after_secs(1).await;
                unsafe { BUTTONS_BLOCKED = false };
            }

            while unsafe { BUTTONS_BLOCKED } {
                // Block here while we wait for it to be unblocked.
                debug!("Button::{}: Waiting for unblock", button);
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
        info!("Button::{}: Button press detected", button);

        // Don't really care how long a button have been pressed as,
        // the `debounce()` will detect when it's been RELEASED.
        match with_deadline(start + Duration::from_secs(1), btn.debounce()).await {
            Ok(_) => {
                trace!(
                    "{}: Button pressed for: {}ms",
                    button,
                    start.elapsed().as_millis()
                );
                debug!(
                    "Button::{}: Button enabled: {}; Button pressed: {}",
                    button,
                    unsafe { BUTTON_ENABLED },
                    button
                );

                // We know who WE are, so turn ON our own LED and turn off all the other LEDs.
                match button {
                    Button::UNSET => (),
                    Button::P => {
                        if unsafe { button == BUTTON_ENABLED } {
                            // Already enabled => blink *our* LED three times.
                            debug!(
                                "Button::{}: Already enabled, blinking LED three times",
                                button
                            );

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
                            debug!(
                                "Button::{}: Already enabled, blinking LED three times",
                                button
                            );

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
                            debug!(
                                "Button::{}: Already enabled, blinking LED three times",
                                button
                            );

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
                            debug!(
                                "Button::{}: Already enabled, blinking LED three times",
                                button
                            );

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
        trace!(
            "{}: Button pressed for: {}ms",
            button,
            start.elapsed().as_millis()
        );
    }
}
