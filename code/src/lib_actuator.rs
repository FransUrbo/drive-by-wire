use defmt::{debug, error, info, trace};

use embassy_rp::flash::Async;
use embassy_rp::gpio::{Input, Output, SlewRate};
use embassy_rp::peripherals::FLASH;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_time::Timer;

// External "defines".
use crate::Button;
use crate::BUTTONS_BLOCKED;
use crate::BUTTON_ENABLED;

use crate::lib_config;
use crate::DbwConfig;
use crate::FLASH_SIZE;

pub static CHANNEL_ACTUATOR: Channel<ThreadModeRawMutex, Button, 64> = Channel::new();

// Set the distance between the different mode. 70mm is the total throw from begining to end.
static ACTUATOR_DISTANCE_BETWEEN_POSITIONS: i8 = 70 / 3; // 3 because the start doesn't count :).

// Actually move the actuator.
// TODO: How do we actually move the actuator?
//       Is it enough to send it +3V3 or +5V on the motor relay, or does it need more power? If so,
//       we need two more MOSFETs.
pub async fn move_actuator(
    amount: i8, // Distance in mm in either direction.
    pin_motor_plus: &mut Output<'static>,
    pin_motor_minus: &mut Output<'static>,
) {
    if amount < 0 {
        info!("Moving actuator:  direction=FORWARD; amount={}", amount);
        pin_motor_minus.set_low(); // Set the other pin to low. There can be only one!

        // FAKE: Simulate move by toggling the pin HIGH and LOW `amount` (mm) times.
        let mut pos: i8 = 0; // Make sure to blink BOTH at completion of every position move.
        for i in amount..=0 {
            // FAKE: For every position, turn BOTH led on for a bit, to indicate position.
            trace!("pos={}; i={}", pos, i);
            if i % ACTUATOR_DISTANCE_BETWEEN_POSITIONS == 0 {
                if pos != 0 {
                    trace!("i % {}", ACTUATOR_DISTANCE_BETWEEN_POSITIONS);
                    pin_motor_minus.set_high();
                    pin_motor_plus.set_high();
                    Timer::after_millis(100).await;
                    pin_motor_minus.set_low();
                    pin_motor_plus.set_low();
                }

                pos += 1;
            }

            pin_motor_plus.set_high();
            Timer::after_millis(50).await;
            pin_motor_plus.set_low();
            Timer::after_millis(50).await;
        }
    } else {
        info!("Moving actuator: direction=BACKWARD; amount={}", amount);
        pin_motor_plus.set_low(); // Set the other pin to low. There can be only one!

        // FAKE: Simulate move by toggling the pin HIGH and LOW `amount` (mm) times.
        let mut pos: i8 = 0; // Make sure to blink BOTH at completion of every position move.
        for i in 0..=amount {
            // FAKE: For every position, turn BOTH led on for a bit, to indicate position.
            trace!("pos={}; i={}", pos, i);
            if i % ACTUATOR_DISTANCE_BETWEEN_POSITIONS == 0 {
                if pos != 0 {
                    trace!("i % {}", ACTUATOR_DISTANCE_BETWEEN_POSITIONS);
                    pin_motor_minus.set_high();
                    pin_motor_plus.set_high();
                    Timer::after_millis(100).await;
                    pin_motor_minus.set_low();
                    pin_motor_plus.set_low();
                }

                pos += 1;
            }

            pin_motor_minus.set_high();
            Timer::after_millis(50).await;
            pin_motor_minus.set_low();
            Timer::after_millis(50).await;
        }
    }

    // TODO: Verify with the potentiometer on the actuator that we've actually moved it to the right position.
    //       Documentation say "Actual resistance value may vary within the 0-10kΩ range based on stroke length".
}

// Control the actuator. Calculate in what direction and how much to move it to get to
// the desired drive mode position.
#[embassy_executor::task]
pub async fn actuator_control(
    receiver: Receiver<'static, ThreadModeRawMutex, Button, 64>,
    flash: &mut embassy_rp::flash::Flash<'static, FLASH, Async, FLASH_SIZE>,
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
        move_actuator(amount, &mut pin_motor_plus, &mut pin_motor_minus).await;

        // Now that we're done moving the actuator, Enable reading buttons again.
        unsafe { BUTTONS_BLOCKED = false };

        // .. and update the button enabled.
        unsafe { BUTTON_ENABLED = button };

        // .. and write it to flash.
        let mut config = match DbwConfig::read(flash) {
            // Read the old/current values.
            Ok(config) => config,
            Err(e) => {
                error!("Failed to read flash: {:?}", e);
                lib_config::resonable_defaults()
            }
        };
        config.active_button = button; // Set new value.
        lib_config::write_flash(flash, config).await;
    }
}

// ========================================

// Test actuator control. Move it backward 1mm, then forward 1mm.
// This should be safe to do EVEN IF (!!) we're moving (for whatever reason).
pub async fn test_actuator(
    pin_motor_plus: &mut Output<'static>,
    pin_motor_minus: &mut Output<'static>,
) -> bool {
    info!("Testing actuator control");
    move_actuator(-1, pin_motor_plus, pin_motor_minus).await;
    Timer::after_millis(100).await;
    move_actuator(1, pin_motor_plus, pin_motor_minus).await;

    // TODO: How do we know the actuator test worked?

    true
}
