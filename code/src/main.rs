#![no_std]
#![no_main]

use defmt::{debug, error, info};

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::Async;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::peripherals::{PIO1, UART0, UART1};
use embassy_rp::pio::{InterruptHandler as PIOInterruptHandler, Pio};
use embassy_rp::uart::{Blocking, Config, InterruptHandler as UARTInterruptHandler, UartTx};
use embassy_rp::watchdog::*;
use embassy_time::{Duration, Timer};

use static_cell::StaticCell;

use {defmt_serial as _, panic_probe as _};

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

static SERIAL: StaticCell<UartTx<'_, UART1, Blocking>> = StaticCell::new();

// DMA Channels used:
// * Fingerprint scanner:	UART0	DMA_CH[0-1]	PIN_13, PIN_16, PIN_17
// * NeoPixel:			PIO1	DMA_CH2		PIN_15
// * Flash:			FLASH	DMA_CH3		-
// * Serial logging:		UART1	DMA_CH4		PIN_4
bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => PIOInterruptHandler<PIO1>;	// NeoPixel
    UART0_IRQ  => UARTInterruptHandler<UART0>;	// Fingerprint scanner
    UART1_IRQ  => UARTInterruptHandler<UART1>;	// Serial logging
});

// ================================================================================

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // =====
    //  1. Initialize the serial UART for debug/log output.
    let uart = UartTx::new(p.UART1, p.PIN_4, p.DMA_CH4, Config::default()); // => 115200/8N1
    defmt_serial::defmt_serial(SERIAL.init(uart));

    info!("Start");

    // =====
    //  2. Initialize the built-in LED and turn it on. Just for completness.
    let _builtin_led = Output::new(p.PIN_25, Level::High);

    // =====
    //  3. Initialize the NeoPixel LED. Do this first, so we can turn on the status LED.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO1, Irqs);
    let mut neopixel = ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH2, p.PIN_15);
    info!("Initialized the NeoPixel LED");
    neopixel.write(&[(255, 100, 0).into()]).await; // ORANGE -> starting

    // =====
    //  4. Initialize the watchdog. Needs to be second, so it'll restart if something goes wrong.
    let mut watchdog = Watchdog::new(p.WATCHDOG);
    watchdog.start(Duration::from_millis(1_050));
    spawner
        .spawn(feed_watchdog(CHANNEL_WATCHDOG.receiver(), watchdog))
        .unwrap();
    info!("Initialized the watchdog timer");

    // =====
    //  5. TODO: Initialize the CAN bus. Needs to come third, so we can talk to the IC.
    spawner.spawn(read_can()).unwrap();
    spawner
        .spawn(write_can(CHANNEL_CANWRITE.receiver()))
        .unwrap();

    // Send message to IC: "Starting Drive-By-Wire system".
    CHANNEL_CANWRITE.send(CANMessage::Starting).await;

    // =====
    //  6. Initialize the MOSFET relays.
    let mut eis_steering_lock = Output::new(p.PIN_18, Level::Low); // EIS/steering lock
    let mut eis_start = Output::new(p.PIN_22, Level::Low); // EIS/start
    CHANNEL_CANWRITE.send(CANMessage::RelaysInitialized).await;

    // =====
    //  7. Initialize the flash drive where we store some config values across reboots.
    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH3);

    // Read the config from flash drive.
    let config = DbwConfig::read(&mut flash).unwrap();

    // =====
    //  8. Initialize and test the actuator.
    CHANNEL_CANWRITE.send(CANMessage::InitActuator).await;
    let mut actuator_motor_plus = Output::new(p.PIN_27, Level::Low); // Actuator/Motor Relay (-)
    let mut actuator_motor_minus = Output::new(p.PIN_28, Level::Low); // Actuator/Motor Relay (+)
    let actuator_potentiometer = Input::new(p.PIN_26, Pull::None); // Actuator/Potentiometer Brush

    // Test actuator control.
    if !test_actuator(&mut actuator_motor_plus, &mut actuator_motor_minus).await {
        error!("Actuator failed to move");

        // Stop feeding the watchdog, resulting in a reset.
        CHANNEL_WATCHDOG.send(StopWatchdog::Yes).await;
    }

    // Actuator works. Spawn off the actuator control task.
    spawner
        .spawn(actuator_control(
            CHANNEL_ACTUATOR.receiver(),
            flash,
            actuator_motor_plus,
            actuator_motor_minus,
            actuator_potentiometer,
        ))
        .unwrap();
    CHANNEL_CANWRITE.send(CANMessage::ActuatorInitialized).await;

    // =====
    //  9. Initialize the fingerprint scanner.
    CHANNEL_CANWRITE.send(CANMessage::InitFP).await;
    let mut fp_scanner = r503::R503::new(
        p.UART0,
        Irqs,
        p.PIN_16,
        p.DMA_CH0,
        p.PIN_17,
        p.DMA_CH1,
        p.PIN_13.into(),
    );
    CHANNEL_CANWRITE.send(CANMessage::FPInitialized).await;

    // Send message to IC: "Authorizing use".
    CHANNEL_CANWRITE.send(CANMessage::Authorizing).await;

    // Verify fingerprint.
    if config.valet_mode {
        info!("Valet mode, won't check fingerprint");
    } else if fp_scanner.Wrapper_Verify_Fingerprint().await {
        error!("Can't match fingerprint");

        debug!("NeoPixel RED");
        neopixel.write(&[(255, 0, 0).into()]).await; // RED

        // Give it five seconds before we reset.
        Timer::after_secs(5).await;

        // Stop feeding the watchdog, resulting in a reset.
        CHANNEL_WATCHDOG.send(StopWatchdog::Yes).await;
    } else {
        info!("Fingerprint matches, use authorized");
    }
    neopixel.write(&[(0, 255, 0).into()]).await; // GREEN
    fp_scanner.Wrapper_AuraSet_Off().await; // Turn off the aura.

    // Send message to IC: "Use authorized".
    CHANNEL_CANWRITE.send(CANMessage::Authorized).await;

    // =====
    // 10. Spawn off one button reader per button. They will then spawn off a LED controller each so that
    //     each button can control their "own" LED.
    spawner
        .spawn(read_button(
            spawner,
            Button::P,
            p.PIN_2.degrade(),
            p.PIN_6.degrade(),
        ))
        .unwrap(); // button/P
    spawner
        .spawn(read_button(
            spawner,
            Button::R,
            p.PIN_3.degrade(),
            p.PIN_7.degrade(),
        ))
        .unwrap(); // button/R
    spawner
        .spawn(read_button(
            spawner,
            Button::N,
            p.PIN_0.degrade(),
            p.PIN_8.degrade(),
        ))
        .unwrap(); // button/N
    spawner
        .spawn(read_button(
            spawner,
            Button::D,
            p.PIN_1.degrade(),
            p.PIN_9.degrade(),
        ))
        .unwrap(); // button/D
    CHANNEL_CANWRITE.send(CANMessage::ButtonsInitialized).await;

    // =====
    // 11. TODO: Find out what gear the car is in.
    //     NOTE: Need to do this *after* we've verified that the actuator works. It will tell us what position it
    //           is in, and from there we can extrapolate the gear.
    //     FAKE: Read what button (gear) was enabled when last it changed from the flash.
    match config.active_button {
        Button::P => {
            debug!("Setting enabled button to P");
            CHANNEL_P.send(LedStatus::On).await;
            CHANNEL_R.send(LedStatus::Off).await;
            CHANNEL_N.send(LedStatus::Off).await;
            CHANNEL_D.send(LedStatus::Off).await;

            unsafe { BUTTON_ENABLED = Button::P };
        }
        Button::R => {
            debug!("Setting enabled button to R");
            CHANNEL_P.send(LedStatus::Off).await;
            CHANNEL_R.send(LedStatus::On).await;
            CHANNEL_N.send(LedStatus::Off).await;
            CHANNEL_D.send(LedStatus::Off).await;

            unsafe { BUTTON_ENABLED = Button::R };
        }
        Button::N => {
            debug!("Setting enabled button to N");
            CHANNEL_P.send(LedStatus::Off).await;
            CHANNEL_R.send(LedStatus::Off).await;
            CHANNEL_N.send(LedStatus::On).await;
            CHANNEL_D.send(LedStatus::Off).await;

            unsafe { BUTTON_ENABLED = Button::N };
        }
        Button::D => {
            debug!("Setting enabled button to D");
            CHANNEL_P.send(LedStatus::Off).await;
            CHANNEL_R.send(LedStatus::Off).await;
            CHANNEL_N.send(LedStatus::Off).await;
            CHANNEL_D.send(LedStatus::On).await;

            unsafe { BUTTON_ENABLED = Button::D };
        }
        _ => (),
    }

    // =====
    // 12. Turn on the ignition switch.
    eis_steering_lock.set_high();

    // =====
    // 13. Starting the car by turning on the EIS/start relay on for one sec and then turn it off.
    if config.valet_mode {
        // Set the status LED to BLUE to indicate valet mode.
        neopixel.write(&[(0, 0, 255).into()]).await;
        CHANNEL_CANWRITE.send(CANMessage::ValetMode).await;
    } else {
        // Sleep here three seconds to allow the car to "catch up".
        // Sometime, it takes a while for the car to "wake up". Not sure why..
        debug!("Waiting 3s to wakeup the car");
        Timer::after_secs(3).await;

        CHANNEL_CANWRITE.send(CANMessage::StartCar).await;

        eis_start.set_high();
        Timer::after_secs(1).await;
        eis_start.set_low();
    }

    // =====
    // 14. TODO: Not sure how we avoid stopping the program here, the button presses are done in separate tasks!
    info!("Main function complete, control handed over to subtasks.");
    loop {
        Timer::after_secs(600).await; // Nothing to do, just sleep as long as we can, but 10 minutes should do it.
    }
}
