#![no_std]
#![no_main]

//! Connect to the actuator and read the feedback potentiometer.

use defmt::info;

use embassy_executor::Spawner;
use embassy_rp::{adc::InterruptHandler, bind_interrupts};
use embassy_time::Timer;

use actuator::Actuator;

pub mod lib_resources;
use crate::lib_resources::{
    AssignedResources, PeriActuator, PeriBuiltin, PeriButtons, PeriFPScanner, PeriFlash,
    PeriNeopixel, PeriSerial, PeriStart, PeriSteering, PeriWatchdog,
};

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => InterruptHandler;			// Actuator potentiometer
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let r = split_resources! {p};

    let mut actuator = Actuator::new(
        r.actuator.mplus.into(),  // pin_motor_plus
        r.actuator.mminus.into(), // pin_motor_minus
        r.actuator.vsel.into(),   // pin_volt_select - UART0
        r.actuator.pot,           // pin_pot         - ADC2
        r.actuator.adc,
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
