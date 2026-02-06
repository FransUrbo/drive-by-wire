use defmt::{debug};

use embassy_rp::{
    bind_interrupts,
    i2c,
    i2c::InterruptHandler
};
use embassy_time::Timer;

use ina219::{
    address::Address,
    calibration::{IntCalibration, MicroAmpere},
    SyncIna219
};

use crate::lib_resources::PeriPowerMonitor;

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<embassy_rp::peripherals::I2C1>;
});

#[embassy_executor::task]
pub async fn ups_monitor(ups: PeriPowerMonitor) {
    let i2c = i2c::I2c::new_async(ups.i2c, ups.scl, ups.sda, Irqs, i2c::Config::default());

    // Resolution of 1A, and a shunt of 10mΩ.
    // The shunt resistor in the Pico UPS Hat B: R1/0.01Ω (10,000µΩ/10mΩ).
    //let calib = IntCalibration::new(MicroAmpere(1_000_000), 10_000).unwrap();
    let calib = IntCalibration::new(MicroAmpere(100), 10_000).unwrap();
    let mut ina = SyncIna219::new_calibrated(i2c, Address::from_byte(0x43).unwrap(), calib).unwrap();

    loop {
        let measurement = ina.next_measurement().unwrap().expect("A measurement is ready");

        // Calculate how much charge is left.
        let mut charge: f32 = ((measurement.bus_voltage.voltage_mv() - 3) as f32) / 1.2 * 100.0;
        if charge < 0.0 {
            charge = 0.0;
        } else {
            charge = 100.0;
        }

        debug!("Power:           {}", measurement.power);
        debug!("Current:         {}", measurement.current);
        debug!("Charge:          {}%", charge);

        debug!("Voltage (Bus):   {=f32:#02}V",
              measurement.bus_voltage.voltage_mv() as f32 / 1000.0
        );

        let shunt_voltage_mv = measurement.shunt_voltage.shunt_voltage_mv();
        let shunt_voltage_uv = measurement.shunt_voltage.shunt_voltage_uv();
        debug!("Voltage (Shunt): {=f32:#02}mV ({=f32:#02}µV)",
              shunt_voltage_mv as f32,
              shunt_voltage_uv as f32,
        );

        if (shunt_voltage_uv as i16) < -350 {
            debug!("=> On battery");
        }

        // Checking every minute should be more than enough.
        Timer::after_secs(60).await;
    }
}

