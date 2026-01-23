use defmt::{debug, error, info, todo, trace, Format};

use embassy_executor::Spawner;
use embassy_rp::{
    gpio::{AnyPin, Input, Level, Output, Pull},
    Peri,
};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    channel::{Channel, Receiver},
    mutex::Mutex,
};
use embassy_time::{with_deadline, Duration, Instant, Timer};

pub type ScannerMutex = Mutex<NoopRawMutex, r503::R503<'static>>;

// External "defines".
use crate::lib_actuator::CHANNEL_ACTUATOR;
use crate::lib_can_bus::{CANMessage, CHANNEL_CANWRITE};
use crate::lib_config::{FlashConfigMessages, CHANNEL_FLASH};

use actuator::GearModes;
use debounce;
use r503;

#[derive(Copy, Clone, Format, PartialEq)]
#[repr(u8)]
pub enum Button {
    P,
    R,
    N,
    D,
}

// https://medium.com/@mikecode/rust-conversion-between-enum-and-integer-0e10e613573c
impl Button {
    pub fn from_integer(v: u8) -> Self {
        match v {
            0 => Self::P,
            1 => Self::R,
            2 => Self::N,
            3 => Self::D,
            _ => panic!("Unknown value: {}", v),
        }
    }

    pub fn from(v: Self) -> u8 {
        match v {
            Self::P => 0,
            Self::R => 1,
            Self::N => 2,
            Self::D => 3,
        }
    }

    pub fn to_gearmode(v: Self) -> GearModes {
        match v {
            Self::P => GearModes::P,
            Self::R => GearModes::R,
            Self::N => GearModes::N,
            Self::D => GearModes::D,
        }
    }

    pub fn iterator() -> impl Iterator<Item = Button> {
        [Self::P, Self::R, Self::N, Self::D].iter().copied()
    }
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
pub static mut BUTTON_ENABLED: Button = Button::P;

// When the actuator is moving, we need to set this to `true` to block input.
pub static mut BUTTONS_BLOCKED: bool = false;

// Control the drive button LEDs - four buttons, four LEDs.
// The `button` parameter is only here to prettify the log output :).
#[embassy_executor::task(pool_size = 4)]
async fn set_led(
    receiver: Receiver<'static, CriticalSectionRawMutex, LedStatus, 64>,
    led_pin: Peri<'static, AnyPin>,
    button: Button,
) {
    debug!("Button::{}: Started button LED control task", button);

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
    //    flash: &'static FlashMutex,
    fp_scanner: &'static ScannerMutex,
    button: Button,
    btn_pin: Peri<'static, AnyPin>,
    led_pin: Peri<'static, AnyPin>,
) {
    // Initialize the button listener.
    let mut btn =
        debounce::Debouncer::new(Input::new(btn_pin, Pull::Up), Duration::from_millis(50));

    // Spawn off a LED driver for this button.
    match button {
        Button::P => spawner.spawn(set_led(CHANNEL_P.receiver(), led_pin, button).unwrap()),
        Button::N => spawner.spawn(set_led(CHANNEL_N.receiver(), led_pin, button).unwrap()),
        Button::R => spawner.spawn(set_led(CHANNEL_R.receiver(), led_pin, button).unwrap()),
        Button::D => spawner.spawn(set_led(CHANNEL_D.receiver(), led_pin, button).unwrap()),
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

            // NOTE: Must be in 'P', then press both 'P' and 'N' at the same time for this to work.
            if unsafe { BUTTON_ENABLED == Button::P } && button == Button::N {
                debug!(
                    "Button::{}: Both 'P' and 'N' pressed - toggling Valet Mode",
                    button
                );

                // Turn on the 'P' and 'N' LEDs, to indicate that both have been pressed.
                CHANNEL_P.send(LedStatus::On).await;
                CHANNEL_N.send(LedStatus::On).await;

                {
                    // Verify with a valid fingerprint that we're authorized to change Valet Mode.
                    // The fp_scanner lock is released when it goes out of scope.
                    let mut fp_scanner = fp_scanner.lock().await;
                    if !fp_scanner.Wrapper_Verify_Fingerprint().await {
                        error!("Can't match fingerprint, will not toggle Valet Mode");

                        // Give it five seconds before we retry.
                        Timer::after_secs(5).await;

                        // Turn off the aura.
                        fp_scanner.Wrapper_AuraSet_Off().await;

                        // Turn off the 'N' LED.
                        CHANNEL_N.send(LedStatus::Off).await;

                        // Restart loop.
                        continue;
                    } else {
                        // Toggle Valet Mode.
                        match CHANNEL_FLASH.receive().await {
                            FlashConfigMessages::ValetOn => {
                                CHANNEL_CANWRITE.send(CANMessage::DisableValetMode).await;
                                CHANNEL_FLASH.send(FlashConfigMessages::ValetOff).await;
                            }
                            FlashConfigMessages::ValetOff => {
                                CHANNEL_CANWRITE.send(CANMessage::EnableValetMode).await;
                                CHANNEL_FLASH.send(FlashConfigMessages::ValetOff).await;
                            }
                            _ => todo!(),
                        }

                        // Turn off the 'N' LED.
                        CHANNEL_N.send(LedStatus::Off).await;
                    }
                }

                // Give it a second, so we don't *also* deal with the enabled button.
                // As in, let the button block "reach" the 'N' button task.
                Timer::after_secs(1).await;
                unsafe { BUTTONS_BLOCKED = false };
            }

            let mut cnt = 1;
            while unsafe { BUTTONS_BLOCKED } {
                // Block here while we wait for it to be unblocked.
                debug!("Button::{}: Waiting for unblock", button);
                Timer::after_secs(1).await;

                if cnt >= 5 {
                    unsafe { BUTTONS_BLOCKED = false };
                    break;
                }

                cnt += 1;
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
