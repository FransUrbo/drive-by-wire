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

    let calib = IntCalibration::new(MicroAmpere(1_000_000), 1_000).unwrap();
    let mut ina = SyncIna219::new_calibrated(i2c, Address::from_byte(0x43).unwrap(), calib).unwrap();

    loop {
        let measurement = ina.next_measurement().unwrap().expect("A measurement is ready");

        debug!("Voltage (Bus):   {=f32:#02} V",
              measurement.bus_voltage.voltage_mv() as f32 / 1000.0
        );
        debug!("Voltage (Shunt): {=f32:#02} mV",
              measurement.shunt_voltage.shunt_voltage_mv() as f32
        );
        debug!("Power:           {}", measurement.power);
        debug!("Current:         {}", measurement.current);

        // Checking every 15s should be more than enough.
        Timer::after_secs(15).await;
    }
}

