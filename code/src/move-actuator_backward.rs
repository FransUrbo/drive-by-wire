//! Connect to the actuator and move it 10mm backward.

#![no_std]
#![no_main]

use defmt::info;

use embassy_executor::Spawner;
use embassy_rp::adc::InterruptHandler;
use embassy_rp::bind_interrupts;

use actuator::{Actuator, Direction, RESISTANCE_THROW_1MM};

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => InterruptHandler;			// Actuator potentiometer
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut actuator = Actuator::new(
        p.PIN_10.into(),
        p.PIN_11.into(),
        p.PIN_12.into(),
        p.PIN_26,
        p.ADC,
        Irqs,
    );

    info!(
        "Actuator potentiometer value (#1): {}Ω",
        actuator.read_pot().await
    );
    actuator
        .move_actuator(RESISTANCE_THROW_1MM * 10, Direction::Backward)
        .await;
    info!(
        "Actuator potentiometer value (#2): {}Ω",
        actuator.read_pot().await
    );

    #[allow(clippy::empty_loop)]
    loop {}
}
