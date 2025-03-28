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

use actuator::{Actuator, Direction, GearModes, TIME_THROW_1MM, TIME_THROW_TOTAL};
use static_cell::StaticCell;

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
    static SERIAL: StaticCell<UartTx<'_, UART1, Blocking>> = StaticCell::new();
    defmt_serial::defmt_serial(SERIAL.init(uart));

    // Initialize the actuator.
    let mut actuator = Actuator::new(
        p.PIN_10.into(),
        p.PIN_11.into(),
        p.PIN_12.into(),
        p.PIN_26,
        p.ADC,
        Irqs,
    );

    let pot = actuator.read_pot().await;
    info!("Initial actuator position value: {:?}", pot);

    // 1. Move the actuator to the most outward (forward) endpoint.
    // 2. Move the actuator all the way to most inward (backward) endpoint.
    // 3. Move the actuator 10mm at a time, 10 times.
    //    a. Move the actuator 2mm forward.
    //    b. Move the actuator 2mm backward.
    // 4. Move the actuator to each of the gear modes.
    //    loop {
    // Move the actuator to the outward-most position before we begin.
    let mut direction: Direction = Direction::Backward;
    info!("Move actuator to the outward-most position before we begin");
    actuator
        .move_actuator(TIME_THROW_TOTAL + 50, direction)
        .await;

    // Read the start position value.
    debug!(
        "Actuator potentiometer value (#1): {}Ω",
        actuator.read_pot().await
    );

    // Reverse the direction - go forward-most endpoint.
    direction = Direction::Forward;
    info!("Move actuator to the other forward-most position");
    actuator
        .move_actuator(TIME_THROW_TOTAL + 50, direction)
        .await;

    // Read the end position value.
    debug!(
        "Actuator potentiometer value (#2): {}Ω",
        actuator.read_pot().await
    );

    // Move the actuator 10mm at a time
    direction = Direction::Backward;
    info!("Move actuator backward 10mm at a time, 10 times");
    for i in 1..=10 {
        info!("Move={}", i);

        actuator
            .move_actuator((TIME_THROW_1MM as u64) * 10, direction)
            .await;

        debug!(
            "Actuator potentiometer value (#3/{}): {}Ω",
            i,
            actuator.read_pot().await
        );

        Timer::after_secs(1).await;

        // Run the actuator test (move back and forth 2mm).
        actuator.test_actuator().await;
    }

    // Move the actuator one gear mode at a time, starting with `P`.
    for mode in Button::iterator() {
        info!("Mode={}", mode);
        actuator
            .change_gear_mode(GearModes::from(Button::from_button(mode)))
            .await;
    }
    //    }
}
