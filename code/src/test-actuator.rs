//! Connect to the actuator and move it back and forth in different ways.

#![no_std]
#![no_main]

use defmt::{debug, info};

use embassy_executor::Spawner;
use embassy_rp::adc::InterruptHandler as ADCInterruptHandler;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::UART1;
use embassy_rp::uart::{
    Blocking, Config as UartConfig, InterruptHandler as UARTInterruptHandler, UartTx,
};

use actuator::{Actuator, Direction, RESISTANCE_THROW_MIN, TIME_THROW_1MM, TIME_THROW_TOTAL};
use static_cell::StaticCell;

//use {defmt_serial as _, panic_probe as _};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UART1_IRQ    => UARTInterruptHandler<UART1>;	// Serial logging
    ADC_IRQ_FIFO => ADCInterruptHandler;		// Actuator potentiometer
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Start");

    let p = embassy_rp::init(Default::default());

    // Initialize the serial UART for debug/log output.
//    let uart = UartTx::new(p.UART1, p.PIN_4, p.DMA_CH4, UartConfig::default()); // => 115200/8N1
//    static SERIAL: StaticCell<UartTx<'_, UART1, Blocking>> = StaticCell::new();
//    defmt_serial::defmt_serial(SERIAL.init(uart));

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
    debug!("Initial actuator position value: {:?}", pot);
    let mut direction: Direction = Direction::Backward;
    if (pot > (RESISTANCE_THROW_MIN - 200)) && (pot < (RESISTANCE_THROW_MIN + 200)) {
        direction = Direction::Forward;
    }

    // 1. Move the actuator to the closest (well .. :) endpoint.
    // 2. Move the actuator all the way to the other endpoint.
    // 3. Move the actuator 10mm at a time, 10 times.
    loop {
	// Move the actuator to the outward-most position before we begin.
	debug!("Move the actuator to the outward-most position before we begin");
	actuator.move_actuator(TIME_THROW_TOTAL, direction).await;

        // Read the start position value.
        info!(
            "Actuator potentiometer value (#1): {}Ω",
            actuator.read_pot().await
        );

        // Reverse the direction.
        if direction == Direction::Backward {
            direction = Direction::Forward;
        } else {
            direction = Direction::Backward;
        }

        // Move it all the way to the other outer-most position.
        actuator.move_actuator(TIME_THROW_TOTAL, direction).await;

        // Read the end position value.
        info!(
            "Actuator potentiometer value (#2): {}Ω",
            actuator.read_pot().await
        );

        // =====
        // Reverse the direction.
        if direction == Direction::Backward {
            direction = Direction::Forward;
        } else {
            direction = Direction::Backward;
        }

        // Move the actuator 10mm at a time
        info!("Moving actuator 10mm at a time");
        for i in 1..=10 {
            info!("Move={}", i);

            actuator
                .move_actuator((TIME_THROW_1MM as u64) * 10, direction)
                .await;

            info!(
                "Actuator potentiometer value (#3/{}): {}Ω",
                i,
                actuator.read_pot().await
            );
        }
    }
}
