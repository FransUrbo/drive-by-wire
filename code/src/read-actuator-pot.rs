//! Connect to the actuator and read the feedback potentiometer.

#![no_std]
#![no_main]

use defmt::info;

use embassy_executor::Spawner;
use embassy_rp::adc::InterruptHandler;
use embassy_rp::bind_interrupts;

use actuator::Actuator;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => InterruptHandler;			// Actuator potentiometer
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut actuator = Actuator::new(p.PIN_10.into(), p.PIN_11.into(), p.PIN_26, p.ADC, Irqs);
    info!(
        "Actuator potentiometer value: {}Ω",
        actuator.read_pot().await
    );

    #[allow(clippy::empty_loop)]
    loop {}
}
