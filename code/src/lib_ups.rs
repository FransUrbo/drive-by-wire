use defmt::{debug, info, warn};

use embassy_rp::{
    bind_interrupts,
    i2c::{AbortReason::NoAcknowledge, Config, Error::Abort, I2c, InterruptHandler},
};
use embassy_time::Timer;

use ina219::{
    address::Address,
    calibration::{IntCalibration, MicroAmpere},
    errors::{InitializationError, InitializationErrorReason::I2cError},
    SyncIna219,
};

use crate::lib_buttons::{ButtonState, CHANNEL_BUTTON_STATE};
use crate::lib_resources::{PeriPowerMonitor, UPS_ADDRESS};

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<embassy_rp::peripherals::I2C1>;
});

#[embassy_executor::task]
pub async fn ups_monitor(ups: PeriPowerMonitor) {
    let mut state_battery: bool = false;
    let mut state_power: bool = true;

    let i2c = I2c::new_async(ups.i2c, ups.scl, ups.sda, Irqs, Config::default());

    // Become a publisher on the button state channel.
    let publisher = CHANNEL_BUTTON_STATE.publisher().unwrap();

    // Resolution of 1A, and a shunt of 10mΩ.
    // The shunt resistor in the Pico UPS Hat B: R1/0.01Ω (10,000µΩ/10mΩ).
    //let calib = IntCalibration::new(MicroAmpere(1_000_000), 10_000).unwrap();
    let calib = IntCalibration::new(MicroAmpere(100), 10_000).unwrap();
    match SyncIna219::new_calibrated(i2c, Address::from_byte(UPS_ADDRESS).unwrap(), calib) {
        Err(InitializationError {
            reason: I2cError(Abort(NoAcknowledge)),
            ..
        }) => {
            warn!("Can't initialize the UPS. Not connected?");
            return ();
        }
        Ok(mut ina) => {
            info!("UPS monitor running");

            let mut cnt: u8 = 0;
            loop {
                let measurement = ina
                    .next_measurement()
                    .unwrap()
                    .expect("A measurement is ready");
                let shunt_voltage_uv = measurement.shunt_voltage.shunt_voltage_uv();

                // Calculate how much charge is left.
                let bus_voltage = measurement.bus_voltage.voltage_mv() / 1_000;
                let mut charge: i16 = (((bus_voltage - 3) as f32) / 1.2 * 100.0) as i16;
                if charge < 0 {
                    charge = 0;
                } else if charge > 100 {
                    charge = 100;
                }

                // Every 60s, let's output some stats.
                if (cnt % 60) == 0 {
                    debug!("Power:           {}", measurement.power);
                    debug!("Current:         {}", measurement.current);
                    info!("Charge:          {}%", charge);

                    debug!("Voltage (Bus):   {=f32:#02}V", bus_voltage as f32);

                    let shunt_voltage_mv = measurement.shunt_voltage.shunt_voltage_mv();
                    debug!(
                        "Voltage (Shunt): {=f32:#02}mV ({=f32:#02}µV)",
                        shunt_voltage_mv as f32, shunt_voltage_uv as f32,
                    );

                    cnt = 0; // Reset counter
                }

                if ((shunt_voltage_uv as i16) < -350) && !state_battery {
                    info!("=> On battery ({=f32:#02}µV)", shunt_voltage_uv as f32);

                    state_battery = true;
                    state_power = false;

                    publisher.publish_immediate(ButtonState::Stop);
                } else if ((shunt_voltage_uv as i16) > -50) && !state_power {
                    info!("=> On power ({=f32:#02}µV)", shunt_voltage_uv as f32);

                    state_battery = false;
                    state_power = true;

                    publisher.publish_immediate(ButtonState::Start);
                }

                cnt = cnt + 1;
                Timer::after_secs(1).await;
            }
        }
        _ => {}
    }
}
