#![no_std]
#![no_main]

use defmt::{debug, error, info, unwrap};

use embassy_executor::Spawner;
use embassy_rp::{
    adc::InterruptHandler as ADCInterruptHandler,
    bind_interrupts,
    flash::{Async as FlashAsync, Flash},
    gpio::{Level, Output},
    peripherals::{PIO0, UART0, UART1},
    pio::{InterruptHandler as PIOInterruptHandler, Pio},
    uart::{Blocking, Config as UartConfig, InterruptHandler as UARTInterruptHandler, UartTx},
    watchdog::*,
};
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};

use static_cell::StaticCell;

use actuator::Actuator;
use r503::R503;
use ws2812::{Colour, Ws2812};

use {defmt_serial as _, panic_probe as _};

// External "defines".
pub mod lib_actuator;
pub mod lib_buttons;
pub mod lib_can_bus;
pub mod lib_config;
pub mod lib_watchdog;
pub mod lib_resources;

use crate::lib_actuator::{actuator_control, CHANNEL_ACTUATOR};
use crate::lib_buttons::{
    read_button, Button, LedStatus, ScannerMutex, CHANNEL_D, CHANNEL_N, CHANNEL_P,
    CHANNEL_R, BUTTON_ENABLED
};
use crate::lib_can_bus::{read_can, write_can, CANMessage, CHANNEL_CANWRITE};
use crate::lib_config::{FlashMutex, DbwConfig, FLASH_SIZE};
use crate::lib_watchdog::{feed_watchdog, StopWatchdog, CHANNEL_WATCHDOG};
use crate::lib_resources::{
    AssignedResources, PeriSerial, PeriBuiltin, PeriNeopixel, PeriWatchdog, PeriSteering,
    PeriStart, PeriFlash, PeriActuator, PeriFPScanner, PeriButtons
};

// DMA Channels used (of 12):
// * Fingerprint scanner:	UART0	DMA_CH[0-1]	PIN_13, PIN_16, PIN_17
// * NeoPixel:			PIO0	DMA_CH2		PIN_15
// * Flash:			FLASH	DMA_CH3		-
// * Serial logging:		UART1	DMA_CH4		PIN_4
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0   => PIOInterruptHandler<PIO0>;		// NeoPixel
    UART0_IRQ    => UARTInterruptHandler<UART0>;	// Fingerprint scanner
    UART1_IRQ    => UARTInterruptHandler<UART1>;	// Serial logging
    ADC_IRQ_FIFO => ADCInterruptHandler;		// Actuator potentiometer
});

// ================================================================================

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let r = split_resources! {p};

    // =====
    //  1. Initialize the serial UART for debug/log output.
    let uart = UartTx::new(r.serial.uart, r.serial.pin, r.serial.dma, UartConfig::default()); // => 115200/8N1 (UART1)
    static SERIAL: StaticCell<UartTx<'static, Blocking>> = StaticCell::new();
    defmt_serial::defmt_serial(SERIAL.init(uart));

    info!("Start");
    info!(
        "Application: {}, v{}/{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH")
    );

    // =====
    //  2. Initialize the built-in LED and turn it on. Just for completness.
    let _builtin_led = Output::new(r.builtin.pin, Level::High);

    // =====
    //  3. Initialize the NeoPixel LED. Do this first, so we can turn on the status LED.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(r.neopixel.pio, Irqs);
    let mut neopixel = Ws2812::new(&mut common, sm0, r.neopixel.dma, r.neopixel.pin);
    info!("NeoPixel LED initialized");
    neopixel.set_colour(Colour::ORANGE).await;

    // =====
    //  4. Initialize the watchdog. Needs to be second, so it'll restart if something goes wrong.
    let mut watchdog = Watchdog::new(r.watchdog.peri);
    watchdog.start(Duration::from_millis(1_050));
    spawner.spawn(unwrap!(feed_watchdog(
        CHANNEL_WATCHDOG.receiver(),
        watchdog
    )));
    info!("Watchdog timer initialized");

    // =====
    //  5. Initialize the CAN bus. Needs to come third, so we can talk to the IC.
    spawner.spawn(unwrap!(read_can()));
    spawner.spawn(unwrap!(write_can(CHANNEL_CANWRITE.receiver())));
    info!("CAN bus communicators initialized");
    CHANNEL_CANWRITE.send(CANMessage::Starting).await;

    // =====
    //  6. Initialize the MOSFET relays.
    let mut eis_steering_lock = Output::new(r.steering.pin, Level::Low); // EIS/Steering lock (GREEN)
    let mut eis_start = Output::new(r.start.pin, Level::Low); // EIS/Start (YELLOW)
    info!("EIS relays initialized");
    CHANNEL_CANWRITE.send(CANMessage::RelaysInitialized).await;

    // =====
    //  7. Initialize the flash drive where we store some config values across reboots.
    info!("Initializing the flash drive");
    let flash = Flash::<_, FlashAsync, FLASH_SIZE>::new(r.flash.peri, r.flash.dma);
    static FLASH: StaticCell<FlashMutex> = StaticCell::new();
    let flash = FLASH.init(Mutex::new(flash));

    // Read the config from flash drive.
    let config = {
        // The flash lock is released when it goes out of scope.
        let mut flash = flash.lock().await;
        unwrap!(DbwConfig::read(&mut flash))
    };
    info!("{:?}", config);

    // =====
    //  8. Initialize and test the actuator.
    info!("Initializing actuator");
    CHANNEL_CANWRITE.send(CANMessage::InitActuator).await;
    let mut actuator = Actuator::new(
        r.actuator.mplus.into(), // pin_motor_plus
        r.actuator.mminus.into(), // pin_motor_minus
        r.actuator.vsel.into(), // pin_volt_select - UART0
        r.actuator.pot, // pin_pot         - ADC2
        r.actuator.adc,
        Irqs,
    );

    // Test actuator control.
    if !actuator.test_actuator().await {
        // ERROR: Actuator have not moved.
        error!("Actuator failed to move - resetting");
        CHANNEL_CANWRITE.send(CANMessage::ActuatorTestFailed).await;

        // Stop feeding the watchdog, resulting in a reset.
        CHANNEL_WATCHDOG.send(StopWatchdog::Yes).await;
    }

    // Actuator works. Spawn off the actuator control task.
    spawner.spawn(unwrap!(actuator_control(CHANNEL_ACTUATOR.receiver(), flash, actuator)));
    info!("Actuator initialized");
    CHANNEL_CANWRITE.send(CANMessage::ActuatorInitialized).await;

    // =====
    //  9. Initialize the fingerprint scanner.
    info!("Initializing the fingerprint scanner");
    CHANNEL_CANWRITE.send(CANMessage::InitFP).await;
    let fp_scanner = R503::new(
        r.fpscan.uart,
        Irqs,
        r.fpscan.send_pin,
        r.fpscan.send_dma,
        r.fpscan.recv_pin,
        r.fpscan.recv_dma,
        r.fpscan.wakeup.into()
    );
    static FP_SCANNER: StaticCell<ScannerMutex> = StaticCell::new();
    let fp_scanner = FP_SCANNER.init(Mutex::new(fp_scanner));
    info!("Fingerprint scanner initialized");
    CHANNEL_CANWRITE.send(CANMessage::FPInitialized).await;

    // Verify fingerprint.
    info!("Authorizing use");
    CHANNEL_CANWRITE.send(CANMessage::Authorizing).await;
    if config.valet_mode {
        neopixel.set_colour(Colour::WHITE).await;

        info!("Running in VALET mode, won't authorize");
        CHANNEL_CANWRITE.send(CANMessage::ValetMode).await;
    } else {
        // Loop until we get a successful fingerprint match.
        loop {
            neopixel.set_colour(Colour::BLUE).await;

            {
                // The fp_scanner lock is released when it goes out of scope.
                let mut fp_scanner = fp_scanner.lock().await;
                if !fp_scanner.Wrapper_Verify_Fingerprint().await {
                    error!("Can't match fingerprint - retrying");

                    debug!("NeoPixel RED");
                    neopixel.set_colour(Colour::RED).await;

                    // Give it five seconds before we retry.
                    Timer::after_secs(5).await;
                } else {
                    info!("Fingerprint matches, use authorized");
                    break;
                }

                fp_scanner.Wrapper_AuraSet_Off().await; // Turn off the aura.
            }
        }

        neopixel.set_colour(Colour::GREEN).await;
        info!("Use authorized");
        CHANNEL_CANWRITE.send(CANMessage::Authorized).await;
    }

    // =====
    // 10. Spawn off one button reader per button. They will then spawn off a LED controller each
    //     so thateach button can control their "own" LED.
    info!("Initializing drive buttons");
    spawner.spawn(unwrap!(read_button(
        spawner,
        flash,
        fp_scanner,
        Button::P,
        r.buttons.p_but.into(),
        r.buttons.p_led.into()
    ))); // button/P
    spawner.spawn(unwrap!(read_button(
        spawner,
        flash,
        fp_scanner,
        Button::R,
        r.buttons.r_but.into(),
        r.buttons.r_led.into()
    ))); // button/R
    spawner.spawn(unwrap!(read_button(
        spawner,
        flash,
        fp_scanner,
        Button::N,
        r.buttons.n_but.into(),
        r.buttons.n_led.into()
    ))); // button/N
    spawner.spawn(unwrap!(read_button(
        spawner,
        flash,
        fp_scanner,
        Button::D,
        r.buttons.d_but.into(),
        r.buttons.d_led.into()
    ))); // button/D
    info!("Drive buttons initialized");
    CHANNEL_CANWRITE.send(CANMessage::ButtonsInitialized).await;

    // =====
    // 11. Read what button (gear) was enabled when last it changed from the flash.
    match config.active_button {
        Button::P => {
            info!("Setting enabled button to P");
            CHANNEL_P.send(LedStatus::On).await;
            CHANNEL_R.send(LedStatus::Off).await;
            CHANNEL_N.send(LedStatus::Off).await;
            CHANNEL_D.send(LedStatus::Off).await;

            unsafe { BUTTON_ENABLED = Button::P };
        }
        Button::R => {
            info!("Setting enabled button to R");
            CHANNEL_P.send(LedStatus::Off).await;
            CHANNEL_R.send(LedStatus::On).await;
            CHANNEL_N.send(LedStatus::Off).await;
            CHANNEL_D.send(LedStatus::Off).await;

            unsafe { BUTTON_ENABLED = Button::R };
        }
        Button::N => {
            info!("Setting enabled button to N");
            CHANNEL_P.send(LedStatus::Off).await;
            CHANNEL_R.send(LedStatus::Off).await;
            CHANNEL_N.send(LedStatus::On).await;
            CHANNEL_D.send(LedStatus::Off).await;

            unsafe { BUTTON_ENABLED = Button::N };
        }
        Button::D => {
            info!("Setting enabled button to D");
            CHANNEL_P.send(LedStatus::Off).await;
            CHANNEL_R.send(LedStatus::Off).await;
            CHANNEL_N.send(LedStatus::Off).await;
            CHANNEL_D.send(LedStatus::On).await;

            unsafe { BUTTON_ENABLED = Button::D };
        }
    }

    // 12. Move the gear into the position it was last saved as.
    info!("Changing gear to {}", config.active_button);
    CHANNEL_ACTUATOR.send(config.active_button).await;

    // =====
    // 13. Turn on the ignition switch.
    eis_steering_lock.set_high();
    info!("Turning on the EIS");

    // =====
    // 14. Starting the car by turning on the EIS/start relay on for one sec and then turn it off.
    if !config.valet_mode {
        // Sleep here three seconds to allow the car to "catch up".
        // Sometime, it takes a while for the car to "wake up". Not sure why..
        info!("Waiting 3s to wakeup the car");
        Timer::after_secs(3).await;

        CHANNEL_CANWRITE.send(CANMessage::StartCar).await;

        eis_start.set_high();
        Timer::after_secs(1).await;
        eis_start.set_low();
    }

    // =====
    // 15. TODO: Not sure how we avoid stopping the program here, the button presses are done in
    //           separate tasks!
    info!("Main function complete, control handed over to subtasks.");
    loop {
        // Nothing to do, just sleep as long as we can, but 10 minutes should do it, then just loop.
        Timer::after_secs(600).await;
    }
}
