#![no_std]
#![no_main]

//! Connect to the actuator and move it 10mm backward.

use defmt::info;

use embassy_executor::Spawner;
use embassy_rp::{adc::InterruptHandler, bind_interrupts};

use actuator::{Actuator, RESISTANCE_THROW_1MM};

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

    info!(
        "Actuator potentiometer value (#1): {}Ω",
        actuator.read_pot().await
    );
    actuator.move_actuator(RESISTANCE_THROW_1MM * 10).await;
    info!(
        "Actuator potentiometer value (#2): {}Ω",
        actuator.read_pot().await
    );

    #[allow(clippy::empty_loop)]
    loop {}
}
