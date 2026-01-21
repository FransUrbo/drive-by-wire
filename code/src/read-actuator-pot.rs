#![no_std]
#![no_main]

//! Connect to the actuator and read the feedback potentiometer.

use defmt::info;

use embassy_executor::Spawner;
use embassy_rp::{adc::InterruptHandler, bind_interrupts};
use embassy_time::Timer;

use actuator::Actuator;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => InterruptHandler;			// Actuator potentiometer
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut actuator = Actuator::new(
        p.PIN_10.into(), // pin_motor_plus
        p.PIN_11.into(), // pin_motor_minus
        p.PIN_12.into(), // pin_volt_select
        p.PIN_28,        // pin_pot
        p.ADC,
        Irqs,
    );

    loop {
        info!(
            "Actuator potentiometer value: {}Ω",
            actuator.read_pot().await
        );

        Timer::after_secs(5).await;
    }
}
