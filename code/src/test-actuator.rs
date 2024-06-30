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
use embassy_time::Timer;

use actuator::{Actuator, Direction, THROW_TIME_PER_1MM};
use static_cell::StaticCell;

use {defmt_serial as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UART1_IRQ    => UARTInterruptHandler<UART1>;	// Serial logging
    ADC_IRQ_FIFO => ADCInterruptHandler;		// Actuator potentiometer
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Start");

    let p = embassy_rp::init(Default::default());

    // Initialize the serial UART for debug/log output.
    let uart = UartTx::new(p.UART1, p.PIN_4, p.DMA_CH4, UartConfig::default()); // => 115200/8N1
    static SERIAL: StaticCell<UartTx<'_, UART1, Blocking>> = StaticCell::new();
    defmt_serial::defmt_serial(SERIAL.init(uart));

    // Initialize the actuator.
    let mut actuator = Actuator::new(p.PIN_10.into(), p.PIN_11.into(), p.PIN_26, p.ADC, Irqs);

    // 5s should be enough to move it from one end to another..
    let move_distance = 5000; // ms
    let mut direction = Direction::Forward;

    // Move the actuator to the forward-most position before we begin.
    debug!("Move the actuator to the forward-most position before we begin");
    actuator.move_actuator(move_distance, direction).await;
    Timer::after_secs(1).await; // Give it a second to "settle". Just in case..

    loop {
        // =====
        // Reverse the direction.
        if direction == Direction::Backward {
            direction = Direction::Forward;
        } else {
            direction = Direction::Backward;
        }

        // Read the start position value.
        info!(
            "Actuator potentiometer value (#1): {}Ω",
            actuator.read_pot().await
        );

        // Move it all the way to the backward-most position.
        actuator.move_actuator(move_distance, direction).await;
        Timer::after_secs(1).await; // Give it a second to "settle". Just in case..

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

        // Move the actuator ten times 10mm.
        info!("Moving actuator 1s at a time");
        for i in 1..=10 {
            info!("Move={}", i);

            actuator.move_actuator(1000, direction).await;
            Timer::after_secs(1).await; // Give it a second to "settle". Just in case..

            info!(
                "Actuator potentiometer value (#3/{}): {}Ω",
                i,
                actuator.read_pot().await
            );
        }

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
                .move_actuator((THROW_TIME_PER_1MM as u64) * 10, direction)
                .await;
            Timer::after_secs(1).await; // Give it a second to "settle". Just in case..

            info!(
                "Actuator potentiometer value (#3/{}): {}Ω",
                i,
                actuator.read_pot().await
            );
        }
    }
}
