//! Connect to the actuator and move it 10mm forward.

#![no_std]
#![no_main]

use defmt::info;

use embassy_executor::Spawner;
use embassy_rp::adc::InterruptHandler;
use embassy_rp::bind_interrupts;

#[allow(unused_imports)]
use actuator::{Actuator, Direction, THROW_TIME_PER_1MM};

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

    //let move_time = (THROW_TIME_PER_1MM as u64) * 10;
    let move_time = 5000;

    info!(
        "Actuator potentiometer value (#1): {}Ω",
        actuator.read_pot().await
    );
    actuator.move_actuator(move_time, Direction::Forward).await;
    info!(
        "Actuator potentiometer value (#2): {}Ω",
        actuator.read_pot().await
    );

    #[allow(clippy::empty_loop)]
    loop {}
}
