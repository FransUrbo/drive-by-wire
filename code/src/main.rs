#![no_std]
#![no_main]

use defmt::{debug, error, info, trace};

use embassy_executor::Spawner;
use embassy_rp::gpio::{AnyPin, Level, Input, Output, Pin, Pull, SlewRate};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{PIO1, UART0, FLASH};
use embassy_rp::uart::InterruptHandler as UARTInterruptHandler;
use embassy_rp::pio::{InterruptHandler as PIOInterruptHandler, Pio};
use embassy_rp::watchdog::*;
use embassy_rp::flash::{Async, ERASE_SIZE};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_time::{with_deadline, Duration, Instant, Timer};

use ws2812;
use debounce;
use r503;

use {defmt_rtt as _, panic_probe as _};

// ================================================================================

#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
enum Button { P, R, N, D, UNSET }
enum LedStatus { On, Off }
enum StopWatchdog { Yes }
enum CANMessage { Starting, InitFP, FPInitialized, InitActuator, ActuatorInitialized, Authorizing, Authorized }

// Setup the communication channels between the tasks.
static CHANNEL_P:        Channel<ThreadModeRawMutex, LedStatus,    64>	= Channel::new();
static CHANNEL_N:        Channel<ThreadModeRawMutex, LedStatus,    64>	= Channel::new();
static CHANNEL_R:        Channel<ThreadModeRawMutex, LedStatus,    64>	= Channel::new();
static CHANNEL_D:        Channel<ThreadModeRawMutex, LedStatus,    64>	= Channel::new();
static CHANNEL_WATCHDOG: Channel<ThreadModeRawMutex, StopWatchdog, 64>	= Channel::new();
static CHANNEL_ACTUATOR: Channel<ThreadModeRawMutex, Button,       64>	= Channel::new();
static CHANNEL_CANWRITE: Channel<ThreadModeRawMutex, CANMessage,   64>	= Channel::new();

// Start with the button UNSET, then change it when we know what gear the car is in.
static mut BUTTON_ENABLED: Button = Button::UNSET;

// When the actuator is moving, we need to set this to `true` to block input.
static mut BUTTONS_BLOCKED: bool = false;

// Set the distance between the different mode. 70mm is the total throw from begining to end.
static ACTUATOR_DISTANCE_BETWEEN_POSITIONS: i8 = 70 / 3; // 3 because the start doesn't count :).

// Setup the flash storage size. Gives us four u8 "slots" for long-term storage.
// https://github.com/embassy-rs/embassy/blob/45a2abc392df91ce6963ac0956f48f22bfa1489b/examples/rp/src/bin/flash.rs
const ADDR_OFFSET: u32 = 0x100000;
const FLASH_SIZE: usize = 2 * 1024 * 1024; // 2MB flash in the Pico.

bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => PIOInterruptHandler<PIO1>;	// NeoPixel
    UART0_IRQ  => UARTInterruptHandler<UART0>;	// Fingerprint scanner
});

// ================================================================================

// Doggy is hungry, needs to be feed every three quarter second, otherwise it gets cranky! :)
#[embassy_executor::task]
async fn feed_watchdog(
    control: Receiver<'static, ThreadModeRawMutex, StopWatchdog, 64>,
    mut wd: embassy_rp::watchdog::Watchdog)
{
    debug!("Started watchdog feeder task");

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

// Write messages to CAN-bus.
#[embassy_executor::task]
async fn write_can(receiver: Receiver<'static, ThreadModeRawMutex, CANMessage, 64>) {
    debug!("Started CAN write task");

    loop {
	let message = receiver.receive().await; // Block waiting for data.
	match message {
	    CANMessage::Starting		=> {
		debug!("Sending message to IC: 'Starting Drive-By-Wire system'");
	    }
	    CANMessage::InitFP			=> {
		debug!("Sending message to IC: 'Initializing Fingerprint Scanner'");
	    }
	    CANMessage::FPInitialized		=> {
		debug!("Sending message to IC: 'Fingerprint scanner initialized'");
	    }
	    CANMessage::InitActuator		=> {
		debug!("Sending message to IC: 'Initializing actuator'");
	    }
	    CANMessage::ActuatorInitialized	=> {
		debug!("Sending message to IC: 'Actuator initialized'");
	    }
	    CANMessage::Authorizing		=> {
		debug!("Sending message to IC: 'Authorizing use'");
	    }
	    CANMessage::Authorized		=> {
		debug!("Sending message to IC: 'Use authorized'");
	    }
	}
    }
}

// Read CAN-bus messages.
#[embassy_executor::task]
async fn read_can() {
    debug!("Started CAN read task");

    loop {
	// TODO: Read CAN-bus messages (blocking).

	// TODO: If we're moving, disable buttons.

	// TODO: If we're NOT moving, and brake pedal is NOT depressed, disable buttons.

	// TODO: If we're NOT moving, and brake pedal is depressed, enable buttons.

	Timer::after_secs(600).await; // TODO: Nothing to do yet, just sleep as long as we can, but 10 minutes should do it.
    }
}

async fn write_flash(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>, offset: u32, buf: u8) -> u8 {
    match flash.blocking_erase(
	ADDR_OFFSET + offset + ERASE_SIZE as u32,
	ADDR_OFFSET + offset + ERASE_SIZE as u32 + ERASE_SIZE as u32)
    {
	Ok(_)  => debug!("Flash erase successful"),
	Err(e) => info!("Flash erase failed: {}", e)
    }
    match flash.blocking_write(ADDR_OFFSET + offset + ERASE_SIZE as u32, &[buf]) {
	Ok(_)  => debug!("Flash write successful"),
	Err(e) => info!("Flash write failed: {}", e)
    }
    read_flash(flash, 0x00).await
}

async fn read_flash(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>, offset: u32) -> u8 {
    let mut buf: [u8; 1] = [0; 1];
    match flash.blocking_read(ADDR_OFFSET + offset + ERASE_SIZE as u32, &mut buf) {
	Ok(_) => debug!("Read successful"),
	Err(e) => info!("Read failed: {}", e)
    }
    info!("Flash content: {:?}", buf[..]);

    return buf[0];
}

// Actually move the actuator.
// TODO: How do we actually move the actuator?
//       Is it enough to send it +3V3 or +5V on the motor relay, or does it need more power? If so,
//       we need two more MOSFETs.
async fn move_actuator(
    amount:		i8, // Distance in mm in either direction.
    pin_motor_plus:	&mut Output<'static>,
    pin_motor_minus:	&mut Output<'static>)
{
    if amount < 0 {
	info!("Moving actuator: direction=FORWARD; amount={}", amount);
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

		pos = pos + 1;
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

		pos = pos + 1;
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
async fn actuator_control(
    receiver:			Receiver<'static, ThreadModeRawMutex, Button, 64>,
    mut flash:			embassy_rp::flash::Flash<'static, FLASH, Async, FLASH_SIZE>,
    mut pin_motor_plus:		Output<'static>,
    mut pin_motor_minus:	Output<'static>,
    _pin_pot:			Input<'static>)
{
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
	let amount: i8 = (_actuator_position as i8 - button as i8) * ACTUATOR_DISTANCE_BETWEEN_POSITIONS;
	debug!("Move direction and amount => '{} - {} = {}'", _actuator_position, button as i8, amount);

	// Move the actuator.
	move_actuator(amount, &mut pin_motor_plus, &mut pin_motor_minus).await;

	// Now that we're done moving the actuator, Enable reading buttons again.
	unsafe { BUTTONS_BLOCKED = false };

	// .. and update the button enabled.
	unsafe { BUTTON_ENABLED = button };

	// .. and write it to flash.
	write_flash(&mut flash, 0x00, button as u8).await;
    }
}

// Control the drive button LEDs - four buttons, four LEDs.
#[embassy_executor::task(pool_size = 4)]
async fn set_led(receiver: Receiver<'static, ThreadModeRawMutex, LedStatus, 64>, led_pin: AnyPin) {
    debug!("Started button LED control task");

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
    debug!("Started button control task");

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
	// NOTE: We need to wait for a button to be pressed, BEFORE we can check if we're
	//       blocked. If we don't, we've checked if we're blocked, we're not and we
	//       start listening to a button. But if someone else have got the press,
	//       then "this" will still wait for a press!
	//       If we ARE blocked, then sleep until we're not, then restart the loop from
	//       the beginning, listening for a button press again.
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

	// Got a valid button press. Process it..
        let start = Instant::now();
        info!("Button press detected (button task: {})", button as u8);

	// Don't really care how long a button have been pressed as,
	// the `debounce()` will detect when it's been RELEASED.
	match with_deadline(start + Duration::from_secs(1), btn.debounce()).await {
            Ok(_) => {
		trace!("Button pressed for: {}ms", start.elapsed().as_millis());
		debug!("Button enabled: {}; Button pressed: {}", unsafe { BUTTON_ENABLED as u8 }, button as u8);

		// We know who WE are, so turn ON our own LED and turn off all the other LEDs.
		match button {
		    Button::UNSET => (),
		    Button::P  => {
			if unsafe { button == BUTTON_ENABLED } {
			    // Already enabled => blink *our* LED three times.
			    for _i in 0..3 {
				CHANNEL_P.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_P.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons while we deal with this one.
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
			    // Already enabled => blink *our* LED three times.
			    for _i in 0..3 {
				CHANNEL_N.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_N.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons while we deal with this one.
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
			    // Already enabled => blink *our* LED three times.
			    for _i in 0..3 {
				CHANNEL_R.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_R.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons while we deal with this one.
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
			    // Already enabled => blink *our* LED three times.
			    for _i in 0..3 {
				CHANNEL_D.send(LedStatus::Off).await;
				Timer::after_millis(500).await;
				CHANNEL_D.send(LedStatus::On).await;
				Timer::after_millis(500).await;
			    }
			} else {
			    // Disable reading buttons while we deal with this one.
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
	    _ => Timer::after_millis(250).await
	}

	// Wait for button release before handling another press.
	btn.debounce().await;
	trace!("Button pressed for: {}ms", start.elapsed().as_millis());
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Start");

    // =====
    // Initialize the built-in LED and turn it on. Just for completness.
    let _builtin_led = Output::new(p.PIN_25, Level::High);

    // =====
    // Initialize the NeoPixel LED. Do this first, so we can turn on the status LED.
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
    spawner.spawn(read_can()).unwrap();
    spawner.spawn(write_can(CHANNEL_CANWRITE.receiver())).unwrap();
    info!("Initialized the CAN bus");

    // Send message to IC: "Starting Drive-By-Wire system".
    CHANNEL_CANWRITE.send(CANMessage::Starting).await;

    // =====
    // Initialize the MOSFET relays.
    let mut eis_steering_lock   = Output::new(p.PIN_18, Level::Low);	// EIS/steering lock
    let mut eis_ignition_switch = Output::new(p.PIN_19, Level::Low);	// EIS/ignition switch
    let mut eis_start           = Output::new(p.PIN_22, Level::Low);	// EIS/start
    info!("Initialized the MOSFET relays");

    // =====
    // Initialize the flash drive where we stor "currently selected drive mode".
    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH4);
    let stored_button = read_flash(&mut flash, 0x00).await; // Read the stored button from the flash.

    // =====
    // Check valet mode.
    // NOTE: How do we actually do that?? How do we SET it to valet mode??
    let valet_mode = read_flash(&mut flash, (ERASE_SIZE * 2) as u32).await; // Read the stored button from the flash.
    debug!("Valet mode: {}", valet_mode);

    // =====
    CHANNEL_CANWRITE.send(CANMessage::InitActuator).await;
    let mut actuator_motor_plus  = Output::new(p.PIN_27, Level::Low);	// Actuator/Motor Relay (-)
    let mut actuator_motor_minus = Output::new(p.PIN_28, Level::Low);	// Actuator/Motor Relay (+)
    let actuator_potentiometer   = Input::new(p.PIN_26, Pull::None);	// Actuator/Potentiometer Brush

    // Test actuator control. Move it backward 1mm, then forward 1mm.
    // TODO: How do we know the actuator test worked?
    info!("Testing actuator control");
    move_actuator(-1, &mut actuator_motor_plus, &mut actuator_motor_minus).await;
    Timer::after_millis(100).await;
    move_actuator(1, &mut actuator_motor_plus, &mut actuator_motor_minus).await;

    // Actuator works. Spawn off the actuator control task.
    spawner.spawn(actuator_control(
	CHANNEL_ACTUATOR.receiver(),
	flash,
	actuator_motor_plus,
	actuator_motor_minus,
	actuator_potentiometer)
    ).unwrap();
    info!("Initialized the actuator");
    CHANNEL_CANWRITE.send(CANMessage::ActuatorInitialized).await;

    // =====
    // Initialize the fingerprint scanner.
    CHANNEL_CANWRITE.send(CANMessage::InitFP).await;
    let mut fp_scanner = r503::R503::new(p.UART0, Irqs, p.PIN_16, p.DMA_CH0, p.PIN_17, p.DMA_CH1, p.PIN_13.into());
    info!("Initialized the fingerprint scanner");
    CHANNEL_CANWRITE.send(CANMessage::FPInitialized).await;

    // ================================================================================

    // Send message to IC: "Authorizing use".
    CHANNEL_CANWRITE.send(CANMessage::Authorizing).await;

    // Verify fingerprint.
    if valet_mode != 0 {
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

    // Send message to IC: "Use authorized".
    CHANNEL_CANWRITE.send(CANMessage::Authorized).await;

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
    // FAKE: Read what button (gear) was enabled when last it changed from the flash.
    match stored_button {
	0 => {
	    debug!("Setting enabled button to P");
	    CHANNEL_P.send(LedStatus::On).await;
	    CHANNEL_R.send(LedStatus::Off).await;
	    CHANNEL_N.send(LedStatus::Off).await;
	    CHANNEL_D.send(LedStatus::Off).await;

	    unsafe { BUTTON_ENABLED = Button::P };
	}
	1 => {
	    debug!("Setting enabled button to R");
	    CHANNEL_P.send(LedStatus::Off).await;
	    CHANNEL_R.send(LedStatus::On).await;
	    CHANNEL_N.send(LedStatus::Off).await;
	    CHANNEL_D.send(LedStatus::Off).await;

	    unsafe { BUTTON_ENABLED = Button::R };
	}
	2 => {
	    debug!("Setting enabled button to N");
	    CHANNEL_P.send(LedStatus::Off).await;
	    CHANNEL_R.send(LedStatus::Off).await;
	    CHANNEL_N.send(LedStatus::On).await;
	    CHANNEL_D.send(LedStatus::Off).await;

	    unsafe { BUTTON_ENABLED = Button::N };
	}
	3 => {
	    debug!("Setting enabled button to D");
	    CHANNEL_P.send(LedStatus::Off).await;
	    CHANNEL_R.send(LedStatus::Off).await;
	    CHANNEL_N.send(LedStatus::Off).await;
	    CHANNEL_D.send(LedStatus::On).await;

	    unsafe { BUTTON_ENABLED = Button::D };
	}
	_ => ()
    }

    // Turn on the steering lock, allowing the key to be inserted.
    eis_steering_lock.set_high();

    // Turn on the ignition switch.
    eis_ignition_switch.set_high();

    // =====
    // Starting the car by turning on the EIS/start relay on for one sec and then turn it off.
    if valet_mode != 0 {
	// Set the status LED to BLUE to indicate valet mode.
	neopixel.write(&[(0,0,255).into()]).await;
    } else {
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
    info!("Main function complete, control handed over to subtasks.");
    loop {
	Timer::after_secs(600).await; // Nothing to do, just sleep as long as we can, but 10 minutes should do it.
    }
}
