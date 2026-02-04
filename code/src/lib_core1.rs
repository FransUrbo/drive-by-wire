use defmt::{info, unwrap};

use embassy_executor::Spawner;

use crate::lib_can_bus::{read_can, write_can};
use crate::lib_watchdog::feed_watchdog;
use crate::lib_resources::{PeriWatchdog, PeriPowerMonitor};
use crate::lib_ups::ups_monitor;

#[embassy_executor::task]
pub async fn core1_tasks(
    spawner: Spawner,
    watchdog: PeriWatchdog,
    ups: PeriPowerMonitor
) {
    info!("Spawning tasks on CORE1");

    // -----
    // Spawn Watchdog.
    spawner.spawn(unwrap!(feed_watchdog(watchdog)));
    info!("Watchdog timer running");

    // -----
    // Spawn the CAN reader.
    spawner.spawn(unwrap!(read_can()));
    info!("CAN bus reader running");

    // -----
    // Spawn the CAN writer.
    spawner.spawn(unwrap!(write_can()));
    info!("CAN bus reader running");

    // -----
    // Spawn the UPS monitor.
    spawner.spawn(unwrap!(ups_monitor(ups)));
    info!("UPS monitor running");
}
