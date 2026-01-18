//! Connect to the actuator and move it back and forth in different ways.

#![no_std]
#![no_main]

use defmt::{debug, info};
use {defmt_serial as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_rp::adc::InterruptHandler as ADCInterruptHandler;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::UART1;
use embassy_rp::uart::{
    Blocking, Config as UartConfig, InterruptHandler as UARTInterruptHandler, UartTx,
};
use embassy_time::Timer;

use static_cell::StaticCell;

use actuator::{Actuator, GearModes, RESISTANCE_THROW_1MM, RESISTANCE_THROW_MIN, RESISTANCE_THROW_MAX};

// External "defines". All because we need the `Button` define!!
pub mod lib_actuator;
pub mod lib_buttons;
pub mod lib_can_bus;
pub mod lib_config;
use crate::lib_actuator::CHANNEL_ACTUATOR;
use crate::lib_buttons::{Button, BUTTONS_BLOCKED, BUTTON_ENABLED};
use crate::lib_can_bus::{CANMessage, CHANNEL_CANWRITE};
use crate::lib_config::{DbwConfig, FLASH_SIZE};

bind_interrupts!(struct Irqs {
    UART1_IRQ    => UARTInterruptHandler<UART1>;	// Serial logging
    ADC_IRQ_FIFO => ADCInterruptHandler;		// Actuator potentiometer
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialize the serial UART for debug/log output.
    let uart = UartTx::new(p.UART1, p.PIN_4, p.DMA_CH4, UartConfig::default()); // => 115200/8N1
    static SERIAL: StaticCell<UartTx<'_, Blocking>> = StaticCell::new();
    defmt_serial::defmt_serial(SERIAL.init(uart));

    info!("Start");
    info!(
        "Application: {}, v{}/{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH")
    );

    // Initialize the actuator.
    info!("Initializing actuator");
    let mut actuator = Actuator::new(
        p.PIN_10.into(), // pin_motor_plus
        p.PIN_11.into(), // pin_motor_minus
        p.PIN_12.into(), // pin_volt_select
        p.PIN_28,        // pin_pot
        p.ADC,
        Irqs,
    );
    info!("Actuator initialized");

    // -----

    // 0. Before we do anything, we move the actuator to fully retracted.
    // LOOP:
    // 1. Move the actuator to fully extended.
    // 2. Move the actuator 10mm backward 10 times.
    // 3. Move the actuator 10mm forward 10 times.
    // 4. Move the actuator to fully retracted.
    // 5. Move the actuator to each of the gear modes (from 'P' to 'D').
    // 6. Move the actuator back to 'P'.

    // TODO: What do we do if the actuator haven't moved?

    let mut position: u16;

    // Move to the fully retracted position.
    // NOTE: Just so we start from somewhere :).
    position = RESISTANCE_THROW_MIN + (RESISTANCE_THROW_1MM * 3);
    info!("0.Move actuator to the MIN end position: {:04}Ω", position);
    actuator.move_actuator(position).await;
    Timer::after_secs(3).await;

    loop {
        // Move to the fully extended position.
        position = RESISTANCE_THROW_MAX - (RESISTANCE_THROW_1MM * 3);
        info!("1.Move actuator to the MAX end position: {:04}Ω", position);
        actuator.move_actuator(position).await;
        Timer::after_secs(3).await;

        // Move backward 10mm at a time, 10 times
        info!("2.Move actuator backward 10mm at a time, 10 times");
        for i in 1..=10 {
            let position_now: u16 = actuator.read_pot().await;
            position = position_now - (RESISTANCE_THROW_1MM * 10);

            info!("    Move backward 10mm/{:02}: {}Ω", i, position);
            actuator.move_actuator(position).await;

            Timer::after_secs(1).await;
        }
        Timer::after_secs(2).await;

        // Move forward 10mm at a time, 10 times.
        info!("3.Move actuator forward 10mm at a time, 10 times");
        for i in 1..=10 {
            let position_now: u16 = actuator.read_pot().await;
            position = position_now + (RESISTANCE_THROW_1MM * 10);

            info!("    Move forward 10mm/{:02}: {}Ω", i, position);
            actuator.move_actuator(position).await;

            Timer::after_secs(1).await;
        }
        Timer::after_secs(2).await;

        // Move to the fully retracted position.
        position = RESISTANCE_THROW_MIN + (RESISTANCE_THROW_1MM * 3);
        info!("4.Move actuator to the MIN end position: {:04}Ω", position);
        actuator.move_actuator(position).await;
        Timer::after_secs(3).await;

        // Move the actuator one gear mode at a time, starting with `P`.
        info!("5. Move actuator to specific gear modes");
        for mode in GearModes::iterator() {
            info!("  Mode={}", mode);
            actuator.change_gear_mode(mode).await;

            info!("  Mode={} - DONE", mode);
            Timer::after_secs(1).await;
            debug!("After sleep (1)..");
        }

        Timer::after_secs(2).await;
        debug!("After sleep (2)..");

        info!("6. Move actuator from 'D' back to 'P'");
        actuator.change_gear_mode(GearModes::P).await;

        Timer::after_secs(3).await;
        debug!("After sleep (3)..");

        info!("--");
    }
}
