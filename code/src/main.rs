#![no_std]
#![no_main]

use defmt::{debug, error, info, trace};

use embassy_executor::Spawner;
use embassy_rp::gpio::{AnyPin, Level, Input, Output, Pin, Pull, SlewRate};
use embassy_time::{with_deadline, Duration, Instant, Timer};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{PIO1, UART0};
use embassy_rp::uart::InterruptHandler as UARTInterruptHandler;
use embassy_rp::pio::{InterruptHandler as PIOInterruptHandler, Pio};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_rp::watchdog::*;

use ws2812;
use debounce;
use r503;

use {defmt_rtt as _, panic_probe as _};

#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
enum Button { P, R, N, D, UNSET }

enum LedStatus { On, Off }
enum StopWatchdog { Yes }

static CHANNEL_P:        Channel<ThreadModeRawMutex, LedStatus, 64>	= Channel::new();
static CHANNEL_N:        Channel<ThreadModeRawMutex, LedStatus, 64>	= Channel::new();
static CHANNEL_R:        Channel<ThreadModeRawMutex, LedStatus, 64>	= Channel::new();
static CHANNEL_D:        Channel<ThreadModeRawMutex, LedStatus, 64>	= Channel::new();
static CHANNEL_WATCHDOG: Channel<ThreadModeRawMutex, StopWatchdog, 64>	= Channel::new();
static CHANNEL_ACTUATOR: Channel<ThreadModeRawMutex, Button,    64>	= Channel::new();

// Start with the button UNSET, then change it when we know what gear the car is in.
static mut BUTTON_ENABLED: Button = Button::UNSET;

// When the actuator is moving, we need to set this to `true` to block input.
static mut BUTTONS_BLOCKED: bool = false;

bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => PIOInterruptHandler<PIO1>;	// NeoPixel
    UART0_IRQ  => UARTInterruptHandler<UART0>;	// Fingerprint scanner
});

// ================================================================================

#[embassy_executor::task]
async fn feed_watchdog(
    control: Receiver<'static, ThreadModeRawMutex, StopWatchdog, 64>,
    mut wd: embassy_rp::watchdog::Watchdog)
{
    // Feed the watchdog every 3/4 second to avoid reset.
    loop {
	wd.feed();

        Timer::after_millis(750).await;

	trace!("Trying to receive");
	match control.try_receive() { // Only *if* there's data, receive and deal with it.
	    Ok(StopWatchdog::Yes) => {
		info!("StopWatchdog = Yes received");
		break
	    },
	    Err(_) => {
		trace!("WD control - Uncaught error received");
		continue
	    }
	}
    }
}

// Actually move the actuator.
async fn move_actuator(
    amount:		i8,
    pin_motor_plus:	&mut Output<'static>,
    pin_motor_minus:	&mut Output<'static>)
{
    if amount < 0 {
	info!("Moving actuator: direction=FORWARD; amount={}", amount);
	pin_motor_minus.set_low(); // Set the MINUS to low.
	Timer::after_millis(500).await;

	// FAKE: Simulate move by toggling the pin HIGH and LOW `amount` times.
	for _i in amount..0 {
	    pin_motor_plus.set_high();
	    Timer::after_millis(500).await;
	    pin_motor_plus.set_low();
	    Timer::after_millis(500).await;
	}
    } else {
	info!("Moving actuator: direction=BACKWARD; amount={}", amount);
	pin_motor_plus.set_low(); // Set the PLUS to low.

	// FAKE: Simulate move by toggling the pin HIGH and LOW `amount` times.
	for _i in 0..amount {
	    pin_motor_minus.set_high();
	    Timer::after_millis(500).await;
	    pin_motor_minus.set_low();
	    Timer::after_millis(500).await;
	}
    }
}

// Control the actuator. Calculate in what direction and how much to move it to get to
// the desired drive mode position.
#[embassy_executor::task]
async fn actuator_control(
    receiver:			Receiver<'static, ThreadModeRawMutex, Button, 64>,
    mut pin_motor_plus:		Output<'static>,
    mut pin_motor_minus:	Output<'static>,
    _pin_pot:			Input<'static>)
{
    pin_motor_plus.set_slew_rate(SlewRate::Fast);
    pin_motor_minus.set_slew_rate(SlewRate::Fast);

    loop {
	let button = receiver.receive().await; // Block waiting for data.

	// TODO: Figure out what gear is in from this.
	//let _actuator_position = pin_pot.get_level();

	// FAKE: Use the current button selected to calculate the direction and amount to move the actuator
	let _actuator_position = unsafe { BUTTON_ENABLED as u8 };

	// Calculate the direction to move, based on current position and where we want to go.
	// - => Higher gear - BACKWARDS
	// + => Lower gear  - FORWARD
	let amount: i8 = _actuator_position as i8 - button as i8;
	debug!("Move direction and amount => '{} - {} = {}'", _actuator_position, button as i8, amount);

	// Move the actuator.
	move_actuator(amount, &mut pin_motor_plus, &mut pin_motor_minus).await;

	// Now that we're done moving the actuator, Enable reading buttons again.
	unsafe { BUTTONS_BLOCKED = false };

	// .. and update the button enabled.
	unsafe { BUTTON_ENABLED = button };
    }
}

// Control the drive button LEDs - four buttons, four LEDs.
#[embassy_executor::task(pool_size = 4)]
async fn set_led(receiver: Receiver<'static, ThreadModeRawMutex, LedStatus, 64>, led_pin: AnyPin) {
    let mut led = Output::new(led_pin, Level::Low); // Always start with the LED off.

    loop {
	match receiver.receive().await { // Block waiting for data.
	    LedStatus::On  => led.set_high(),
	    LedStatus::Off => led.set_low()
	}
    }
}

// Listen for button presses - four buttons, one task each.
#[embassy_executor::task(pool_size = 4)]
async fn read_button(
    spawner: Spawner,
    button:  Button,
    btn_pin: AnyPin,
    led_pin: AnyPin)
{
    let mut btn = debounce::Debouncer::new(Input::new(btn_pin, Pull::Up), Duration::from_millis(20));

    // Spawn off a LED driver for this button.
    match button {
	Button::UNSET => (), // Should be impossible, but just to make the compiler happy.
	Button::P     => spawner.spawn(set_led(CHANNEL_P.receiver(), led_pin)).unwrap(),
	Button::N     => spawner.spawn(set_led(CHANNEL_N.receiver(), led_pin)).unwrap(),
	Button::R     => spawner.spawn(set_led(CHANNEL_R.receiver(), led_pin)).unwrap(),
	Button::D     => spawner.spawn(set_led(CHANNEL_D.receiver(), led_pin)).unwrap()
    }

    loop {
        btn.debounce().await; // Button pressed

	if unsafe { BUTTONS_BLOCKED == true } {
	    debug!("Buttons blocked == {}", unsafe { BUTTONS_BLOCKED as u8 });
	    while unsafe { BUTTONS_BLOCKED == true } {
		// Block here while we wait for it to be unblocked.
		debug!("Waiting for unblock (button task: {})", button as u8);
		Timer::after_secs(1).await;
	    }
	    continue;
	}

        let start = Instant::now();
        info!("Button press detected (button task: {})", button as u8);

	// Don't really care how long a button have been pressed as,
	// the `debounce()` will detect when it's been RELEASED.
	match with_deadline(start + Duration::from_secs(1), btn.debounce()).await {
            Ok(_) => {
		trace!("Button pressed for: {}ms", start.elapsed().as_millis());
		debug!("Button enabled: {}; Button pressed: {}", unsafe { BUTTON_ENABLED as u8 }, button as u8);

		// We know who WE are, so turn ON our own LED and turn off all the other LEDs.
		// Turn on our OWN LED.
		match button {
		    Button::UNSET => (),
		    Button::P  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for _i in 0..3 {
				CHANNEL_P.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_P.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons
			    unsafe { BUTTONS_BLOCKED = true };

			    CHANNEL_P.send(LedStatus::On).await;
			    CHANNEL_N.send(LedStatus::Off).await;
			    CHANNEL_R.send(LedStatus::Off).await;
			    CHANNEL_D.send(LedStatus::Off).await;

			    // Trigger the actuator to switch to (P)ark.
			    CHANNEL_ACTUATOR.send(Button::P).await;
			}

			continue;
		    }
		    Button::N  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for _i in 0..3 {
				CHANNEL_N.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_N.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons
			    unsafe { BUTTONS_BLOCKED = true };

			    CHANNEL_P.send(LedStatus::Off).await;
			    CHANNEL_N.send(LedStatus::On).await;
			    CHANNEL_R.send(LedStatus::Off).await;
			    CHANNEL_D.send(LedStatus::Off).await;

			    // Trigger the actuator to switch to (N)eutral.
			    CHANNEL_ACTUATOR.send(Button::N).await;
			}
			continue;
		    }
		    Button::R  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for _i in 0..3 {
				CHANNEL_R.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_R.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons
			    unsafe { BUTTONS_BLOCKED = true };

			    CHANNEL_P.send(LedStatus::Off).await;
			    CHANNEL_N.send(LedStatus::Off).await;
			    CHANNEL_R.send(LedStatus::On).await;
			    CHANNEL_D.send(LedStatus::Off).await;

			    // Trigger the actuator to switch to (R)everse.
			    CHANNEL_ACTUATOR.send(Button::R).await;
			}

			continue;
		    }
		    Button::D  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for _i in 0..3 {
				CHANNEL_D.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_D.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons
			    unsafe { BUTTONS_BLOCKED = true };

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
	    // TODO: This probably needs to be longer, need to wait for the actuator.
	    //       Don't know how long that takes to move, but we can't allow another
	    //       gear change until it's done + 1s (?).
	    _ => Timer::after_millis(250).await
	}

	// wait for button release before handling another press
	btn.debounce().await;
	trace!("Button pressed for: {}ms", start.elapsed().as_millis());
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let valet_mode: bool = false;

    info!("Start");

    let p = embassy_rp::init(Default::default());

    // =====
    // Initialize the NeoPixel LED. Do this first, so we can turn on the LED.
    let Pio { mut common, sm0, .. } = Pio::new(p.PIO1, Irqs);
    let mut neopixel = ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH3, p.PIN_15);
    info!("Initialized the NeoPixel LED");
    neopixel.write(&[(255,100,0).into()]).await; // ORANGE -> starting

    // =====
    // Initialize the watchdog. Needs to be second, so it'll restart if something goes wrong.
    let mut watchdog = Watchdog::new(p.WATCHDOG);
    watchdog.start(Duration::from_millis(1_050));
    spawner.spawn(feed_watchdog(CHANNEL_WATCHDOG.receiver(), watchdog)).unwrap();
    info!("Initialized the watchdog timer");

    // =====
    // TODO: Initialize the CAN bus. Needs to come third, so we can talk to the IC.
    info!("Initialized the CAN bus");

    // TODO: Send message to IC: "Starting Drive-By-Wire system".

    // =====
    // Initialize the MOSFET relays.
    let mut eis_steering_lock   = Output::new(p.PIN_18, Level::Low);	// EIS/steering lock
    let mut eis_ignition_switch = Output::new(p.PIN_19, Level::Low);	// EIS/ignition switch
    let mut eis_start           = Output::new(p.PIN_22, Level::Low);	// EIS/start
    info!("Initialized the MOSFET relays");

    // =====
    let actuator_motor_plus    = Output::new(p.PIN_27, Level::Low);	// Actuator/Motor Relay (-)
    let actuator_motor_minus   = Output::new(p.PIN_28, Level::Low);	// Actuator/Motor Relay (+)
    let actuator_potentiometer = Input::new(p.PIN_26, Pull::None);	// Actuator/Potentiometer Brush
    spawner.spawn(actuator_control(
	CHANNEL_ACTUATOR.receiver(),
	actuator_motor_plus,
	actuator_motor_minus,
	actuator_potentiometer)
    ).unwrap();
    info!("Initialized the actuator");

    // TODO: Test actuator control.

    // =====
    // Initialize the fingerprint scanner.
    let mut fp_scanner = r503::R503::new(p.UART0, Irqs, p.PIN_16, p.DMA_CH0, p.PIN_17, p.DMA_CH1, p.PIN_13.into());
    info!("Initialized the fingerprint scanner");

    // TODO: Send message to IC: "Authorizing use".

    // TODO: Check valet mode.
    // FAKE: Enable valet mode, I know the fingerprint scanner etc work, so don't
    //       need to do that while testing and developing.
    //valet_mode = true;

    // ================================================================================

    // Verify fingerprint.
    if valet_mode {
	info!("Valet mode, won't check fingerprint");
    } else {
	if fp_scanner.Wrapper_Verify_Fingerprint().await {
	    error!("Can't match fingerprint");

	    debug!("NeoPixel RED");
	    neopixel.write(&[(255,0,0).into()]).await; // RED

	    // Give it five seconds before we reset.
	    Timer::after_secs(5).await;

	    // Stop feeding the watchdog, resulting in a reset.
	    CHANNEL_WATCHDOG.send(StopWatchdog::Yes).await;
	} else {
	    info!("Fingerprint matches, use authorized");
	}
    }
    neopixel.write(&[(0,255,0).into()]).await; // GREEN
    fp_scanner.Wrapper_AuraSet_Off().await; // Turn off the aura.

    // TODO: Send message to IC: "Use authorized, welcome <user|valet>".

    // =====
    // Spawn off one button reader per button. They will then spawn off a LED controller each so that
    // each button can control their "own" LED.
    spawner.spawn(read_button(spawner, Button::P, p.PIN_2.degrade(), p.PIN_6.degrade())).unwrap(); // button/P
    spawner.spawn(read_button(spawner, Button::R, p.PIN_3.degrade(), p.PIN_7.degrade())).unwrap(); // button/R
    spawner.spawn(read_button(spawner, Button::N, p.PIN_4.degrade(), p.PIN_8.degrade())).unwrap(); // button/N
    spawner.spawn(read_button(spawner, Button::D, p.PIN_5.degrade(), p.PIN_9.degrade())).unwrap(); // button/D
    info!("Initialized the drive buttons");

    // TODO: Find out what gear the car is in.
    // NOTE: Need to do this *after* we've verified that the actuator works. It will tell us what position it
    //       is in, and from there we can extrapolate the gear.
    // FAKE: Current gear => (P)ark. Turn off all the others.
    CHANNEL_P.send(LedStatus::On).await;
    CHANNEL_N.send(LedStatus::Off).await;
    CHANNEL_R.send(LedStatus::Off).await;
    CHANNEL_D.send(LedStatus::Off).await;
    unsafe { BUTTON_ENABLED = Button::P };

    // Turn on the steering lock, allowing the key to be inserted.
    eis_steering_lock.set_high();

    // Turn on the ignition switch.
    eis_ignition_switch.set_high();

    // =====
    // Starting the car by turning on the EIS/start relay on for one sec and then turn it off.
    if !valet_mode {
	// Sleep here three seconds to allow the car to "catch up".
	// Sometime, it takes a while for the car to "wake up". Not sure why..
	debug!("Waiting 3s to wakeup the car");
	Timer::after_secs(3).await;

	info!("Sending start signal to car");
	eis_start.set_high();
	Timer::after_secs(1).await;
	eis_start.set_low();
    }

    // =====
    // TODO: Not sure how we avoid stopping the program here, the button presses are done in separate tasks!
    loop {
	debug!("Main loop waiting..");
	Timer::after_secs(10).await;
    }
}
