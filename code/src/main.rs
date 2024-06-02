#![no_std]
#![no_main]

use defmt::{debug, error, info};

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Input, Output, Pin, Pull};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{PIO1, UART0, FLASH};
use embassy_rp::uart::InterruptHandler as UARTInterruptHandler;
use embassy_rp::pio::{InterruptHandler as PIOInterruptHandler, Pio};
use embassy_rp::watchdog::*;
use embassy_rp::flash::Async;
use embassy_time::{Duration, Timer};

use {defmt_rtt as _, panic_probe as _};

use ws2812;
use r503;

// External "defines".
pub mod lib_actuator;
pub mod lib_buttons;
pub mod lib_can_bus;
pub mod lib_config;
pub mod lib_watchdog;

use crate::lib_actuator::*;
use crate::lib_buttons::*;
use crate::lib_can_bus::*;
use crate::lib_config::*;
use crate::lib_watchdog::*;

use crate::CHANNEL_ACTUATOR;
use crate::CHANNEL_CANWRITE;
use crate::CHANNEL_WATCHDOG;


bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => PIOInterruptHandler<PIO1>;	// NeoPixel
    UART0_IRQ  => UARTInterruptHandler<UART0>;	// Fingerprint scanner
});

// ================================================================================

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    info!("Start");

    // =====
    // 0. Initialize the built-in LED and turn it on. Just for completness.
    let _builtin_led = Output::new(p.PIN_25, Level::High);

    // =====
    //  1. Initialize the NeoPixel LED. Do this first, so we can turn on the status LED.
    let Pio { mut common, sm0, .. } = Pio::new(p.PIO1, Irqs);
    let mut neopixel = ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH3, p.PIN_15);
    info!("Initialized the NeoPixel LED");
    neopixel.write(&[(255,100,0).into()]).await; // ORANGE -> starting

    // =====
    //  2. Initialize the watchdog. Needs to be second, so it'll restart if something goes wrong.
    let mut watchdog = Watchdog::new(p.WATCHDOG);
    watchdog.start(Duration::from_millis(1_050));
    spawner.spawn(feed_watchdog(CHANNEL_WATCHDOG.receiver(), watchdog)).unwrap();
    info!("Initialized the watchdog timer");

    // =====
    //  3. TODO: Initialize the CAN bus. Needs to come third, so we can talk to the IC.
    spawner.spawn(read_can()).unwrap();
    spawner.spawn(write_can(CHANNEL_CANWRITE.receiver())).unwrap();
    info!("Initialized the CAN bus");

    // Send message to IC: "Starting Drive-By-Wire system".
    CHANNEL_CANWRITE.send(CANMessage::Starting).await;

    // =====
    //  4. Initialize the MOSFET relays.
    let mut eis_steering_lock = Output::new(p.PIN_18, Level::Low);	// EIS/steering lock
    let mut eis_start         = Output::new(p.PIN_22, Level::Low);	// EIS/start
    info!("Initialized the MOSFET relays");

    // =====
    //  5. Initialize the flash drive where we store some config values across reboots.
    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH4);
    let config = DbwConfig::read(&mut flash).unwrap();
    let stored_button = config.active_button;
    let valet_mode    = config.valet_mode;

    // =====
    //  6. Initialize and test the actuator.
    CHANNEL_CANWRITE.send(CANMessage::InitActuator).await;
    let mut actuator_motor_plus  = Output::new(p.PIN_27, Level::Low);	// Actuator/Motor Relay (-)
    let mut actuator_motor_minus = Output::new(p.PIN_28, Level::Low);	// Actuator/Motor Relay (+)
    let actuator_potentiometer   = Input::new(p.PIN_26, Pull::None);	// Actuator/Potentiometer Brush

    // Test actuator control.
    if !test_actuator(&mut actuator_motor_plus, &mut actuator_motor_minus).await {
	error!("Actuator failed to move");

	// Stop feeding the watchdog, resulting in a reset.
	CHANNEL_WATCHDOG.send(StopWatchdog::Yes).await;
    }

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
    //  7. Initialize the fingerprint scanner.
    CHANNEL_CANWRITE.send(CANMessage::InitFP).await;
    let mut fp_scanner = r503::R503::new(p.UART0, Irqs, p.PIN_16, p.DMA_CH0, p.PIN_17, p.DMA_CH1, p.PIN_13.into());
    info!("Initialized the fingerprint scanner");
    CHANNEL_CANWRITE.send(CANMessage::FPInitialized).await;

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
    //  8. Spawn off one button reader per button. They will then spawn off a LED controller each so that
    //     each button can control their "own" LED.
    spawner.spawn(read_button(spawner, Button::P, p.PIN_2.degrade(), p.PIN_6.degrade())).unwrap(); // button/P
    spawner.spawn(read_button(spawner, Button::R, p.PIN_3.degrade(), p.PIN_7.degrade())).unwrap(); // button/R
    spawner.spawn(read_button(spawner, Button::N, p.PIN_4.degrade(), p.PIN_8.degrade())).unwrap(); // button/N
    spawner.spawn(read_button(spawner, Button::D, p.PIN_5.degrade(), p.PIN_9.degrade())).unwrap(); // button/D
    info!("Initialized the drive buttons");

    // =====
    //  9. TODO: Find out what gear the car is in.
    //     NOTE: Need to do this *after* we've verified that the actuator works. It will tell us what position it
    //           is in, and from there we can extrapolate the gear.
    //     FAKE: Read what button (gear) was enabled when last it changed from the flash.
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

    // =====
    // 10. Turn on the ignition switch.
    eis_steering_lock.set_high();

    // =====
    // 11. Starting the car by turning on the EIS/start relay on for one sec and then turn it off.
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
    // 12. TODO: Not sure how we avoid stopping the program here, the button presses are done in separate tasks!
    info!("Main function complete, control handed over to subtasks.");
    loop {
	Timer::after_secs(600).await; // Nothing to do, just sleep as long as we can, but 10 minutes should do it.
    }
}
