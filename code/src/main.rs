#![no_std]
#![no_main]
#![allow(unused)]

// !! Fingerprint scanner is on PIO0, and the NeoPixel is on PIO1 !!

use defmt::{debug, error, info, trace};

use embassy_executor::Spawner;
use embassy_rp::gpio::{AnyPin, Level, Input, Output, Pin, Pull};
use embassy_time::{with_deadline, Duration, Instant, Timer};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::PIO1;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_rp::watchdog::*;

use ws2812;
use debounce;
use r503;

use {defmt_rtt as _, panic_probe as _};

#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
enum Button { P, N, R, D, UNSET }
enum LedStatus { On, Off }
enum StopState { Yes }

static CHANNEL_P:        Channel<ThreadModeRawMutex, LedStatus, 64> = Channel::new();
static CHANNEL_N:        Channel<ThreadModeRawMutex, LedStatus, 64> = Channel::new();
static CHANNEL_R:        Channel<ThreadModeRawMutex, LedStatus, 64> = Channel::new();
static CHANNEL_D:        Channel<ThreadModeRawMutex, LedStatus, 64> = Channel::new();
static CHANNEL_WATCHDOG: Channel<ThreadModeRawMutex, StopState, 64> = Channel::new();
static CHANNEL_ACTUATOR: Channel<ThreadModeRawMutex, Button,    64> = Channel::new();

static mut BUTTON_ENABLED: Button = Button::UNSET;

bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => InterruptHandler<PIO1>; // NeoPixel
});

// ================================================================================

#[embassy_executor::task]
async fn feed_watchdog(
    control: Receiver<'static, ThreadModeRawMutex, StopState, 64>,
    mut wd: embassy_rp::watchdog::Watchdog)
{
    // Feed the watchdog every 3/4 second to avoid reset.
    loop {
	wd.feed();

        Timer::after_millis(750).await;

	trace!("Trying to receive");
	match control.try_receive() { // Only *if* there's data, receive and deal with it.
	    Ok(StopState::Yes) => {
		info!("StopState = Yes received");
		break
	    },
	    Err(_) => {
		trace!("WD control - Uncaught error received");
		continue
	    }
	}
    }
}

// Talk to the actuator, move it to desired gear position.
// FAKE: Just output what we *would* do if we actually HAD an actuator! :)
#[embassy_executor::task]
async fn actuator_control(receiver: Receiver<'static, ThreadModeRawMutex, Button, 64>) {
    loop {
	match receiver.receive().await { // Block waiting for data.
	    Button::P  => {
		info!("Moving actuator to (P)ark");
	    }
	    Button::N  => {
		info!("Moving actuator to (N)eutral");
	    }
	    Button::R  => {
		info!("Moving actuator to (R)everse");
	    }
	    Button::D  => {
		info!("Moving actuator to (D)rive");
	    }
	    _ => ()
	}
    }
}

// Control the drive button LEDs - four buttons, four LEDs.
#[embassy_executor::task(pool_size = 4)]
async fn set_led(receiver: Receiver<'static, ThreadModeRawMutex, LedStatus, 64>, led_pin: AnyPin) {
    let mut led = Output::new(led_pin, Level::Low);

    loop {
	match receiver.receive().await { // Block waiting for data.
	    LedStatus::On  => led.set_high(),
	    LedStatus::Off => led.set_low(),
	}
    }
}

// Listen for button button presses - four buttons.
#[embassy_executor::task(pool_size = 4)]
async fn read_button(
    spawner: Spawner,
    button:  Button,
    btn_pin: AnyPin,
    led_pin: AnyPin)
{
    let mut btn = debounce::Debouncer::new(Input::new(btn_pin, Pull::Up), Duration::from_millis(20));

    // Spawn off a LED driver for this button.
    let receiver: Receiver<'static, ThreadModeRawMutex, LedStatus, 64>;
    match button {
	Button::UNSET => (),
	Button::P     => spawner.spawn(set_led(CHANNEL_P.receiver(), led_pin)).unwrap(),
	Button::N     => spawner.spawn(set_led(CHANNEL_N.receiver(), led_pin)).unwrap(),
	Button::R     => spawner.spawn(set_led(CHANNEL_R.receiver(), led_pin)).unwrap(),
	Button::D     => spawner.spawn(set_led(CHANNEL_D.receiver(), led_pin)).unwrap()
    }

    loop {
        // button pressed
        btn.debounce().await;
        let start = Instant::now();
        info!("Button press detected");

	// Don't really care how long a button have been pressed as,
	// the `debounce()` will detect when it's been RELEASED.
	match with_deadline(start + Duration::from_secs(1), btn.debounce()).await {
            Ok(_) => {
		trace!("Button pressed for: {}ms", start.elapsed().as_millis());
		debug!("Button pressed: {}; Button enabled: {}", button as u8, unsafe { BUTTON_ENABLED as u8 });

		// We know who WE are, so turn ON our own LED and turn off all the other LEDs.
		// Turn on our OWN LED.
		match button {
		    Button::UNSET => (),
		    Button::P  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for i in 0..3 {
				CHANNEL_P.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_P.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    CHANNEL_P.send(LedStatus::On).await;
			    CHANNEL_N.send(LedStatus::Off).await;
			    CHANNEL_R.send(LedStatus::Off).await;
			    CHANNEL_D.send(LedStatus::Off).await;

			    // Trigger the actuator to switch to (P)ark.
			    CHANNEL_ACTUATOR.send(Button::P).await;

			    // Update the button enabled.
			    unsafe { BUTTON_ENABLED = Button::P };
			}

			continue;
		    }
		    Button::N  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for i in 0..3 {
				CHANNEL_N.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_N.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    CHANNEL_P.send(LedStatus::Off).await;
			    CHANNEL_N.send(LedStatus::On).await;
			    CHANNEL_R.send(LedStatus::Off).await;
			    CHANNEL_D.send(LedStatus::Off).await;

			    // Trigger the actuator to switch to (N)eutral.
			    CHANNEL_ACTUATOR.send(Button::N).await;

			    // Update the button enabled.
			    unsafe { BUTTON_ENABLED = Button::N };
			}
			continue;
		    }
		    Button::R  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for i in 0..3 {
				CHANNEL_R.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_R.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    CHANNEL_P.send(LedStatus::Off).await;
			    CHANNEL_N.send(LedStatus::Off).await;
			    CHANNEL_R.send(LedStatus::On).await;
			    CHANNEL_D.send(LedStatus::Off).await;

			    // Trigger the actuator to switch to (R)everse.
			    CHANNEL_ACTUATOR.send(Button::R).await;

			    // Update the button enabled.
			    unsafe { BUTTON_ENABLED = Button::R };
			}

			continue;
		    }
		    Button::D  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink the LED three times.
			    for i in 0..3 {
				CHANNEL_D.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_D.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    CHANNEL_P.send(LedStatus::Off).await;
			    CHANNEL_N.send(LedStatus::Off).await;
			    CHANNEL_R.send(LedStatus::Off).await;
			    CHANNEL_D.send(LedStatus::On).await;

			    // Trigger the actuator to switch to (D)rive.
			    CHANNEL_ACTUATOR.send(Button::D).await;

			    // Update the button enabled.
			    unsafe { BUTTON_ENABLED = Button::D };
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
    let mut valet: bool = false;

    info!("Start");

    let p = embassy_rp::init(Default::default());

    // =====
    // Initialize the NeoPixel LED. Do this first, so we can turn on the LED.
    let Pio { mut common, sm0, .. } = Pio::new(p.PIO1, Irqs);
    let mut ws2812 = ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH3, p.PIN_15);
    info!("Initialized the NeoPixel LED");
    ws2812.write(&[(130,255,0).into()]).await; // ORANGE -> starting

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
    let mut gpio1 = Output::new(p.PIN_18, Level::Low); // EIS/steering lock
    let mut gpio2 = Output::new(p.PIN_19, Level::Low); // EIS/ignition switch
    let mut gpio3 = Output::new(p.PIN_22, Level::Low); // EIS/start
    info!("Initialized the MOSFET relays");

    // =====
    // TODO: Initialize the actuator.
    spawner.spawn(actuator_control(CHANNEL_ACTUATOR.receiver())).unwrap();
    info!("Initialized the actuator");

    // TODO: Test actuator control.

    // =====
    // Initialize the fingerprint scanner.
    let mut r503 = r503::R503::new(p.UART0, p.PIN_16, p.DMA_CH0, p.PIN_17, p.DMA_CH1, p.PIN_13.into());
    info!("Initialized the fingerprint scanner");

    // TODO: Send message to IC: "Authorizing use".

    // TODO: Check valet mode.
    // FAKE: Enable valet mode, I know the fingerprint scanner etc work, so don't
    //       need to do that while testing and developing.
    //valet = true;

    // ================================================================================

    // Verify fingerprint.
    if ! valet {
	if r503.Wrapper_Verify_Fingerprint().await {
	    error!("Can't match fingerprint");

	    debug!("NeoPixel RED");
	    ws2812.write(&[(0,255,0).into()]).await; // RED

	    // Give it five seconds before we reset.
	    Timer::after_secs(5).await;

	    // Stop feeding the watchdog, resulting in a reset.
	    CHANNEL_WATCHDOG.send(StopState::Yes).await;
	} else {
	    info!("Fingerprint matches, use authorized");
	}
    } else {
	info!("Valet mode, won't check fingerprint");
    }
    ws2812.write(&[(255,0,0).into()]).await; // GREEN
    r503.Wrapper_AuraSet_Off().await; // Turn off the aura.

    // TODO: Send message to IC: "Use authorized, welcome <user|valet>".

    // =====
    // Spawn off one button reader per button. They will then spawn off a LED controller each so that
    // each button can control their "own" LED.
    spawner.spawn(read_button(spawner, Button::P, p.PIN_2.degrade(), p.PIN_6.degrade())).unwrap(); // button/P
    spawner.spawn(read_button(spawner, Button::N, p.PIN_3.degrade(), p.PIN_7.degrade())).unwrap(); // button/N
    spawner.spawn(read_button(spawner, Button::R, p.PIN_4.degrade(), p.PIN_8.degrade())).unwrap(); // button/R
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
    gpio1.set_high();

    // Turn on the ignition switch.
    gpio2.set_high();

    // =====
    // Starting the car by turning on the EIS/start relay on for one sec and then turn it off.
    if !valet {
	// Sleep here three seconds to allow the car to "catch up".
	// Sometime, it takes a while for the car to "wake up". Not sure why..
	debug!("Waiting 3s to wakeup the car");
	Timer::after_secs(3).await;

	gpio3.set_high();
	Timer::after_secs(1).await;
	gpio3.set_low();
    }

    // =====
    // TODO: Not sure how we avoid stopping the program here, the button presses are done in separate tasks!
    loop {
	debug!("Main loop waiting..");
	Timer::after_secs(10).await;
    }
}
