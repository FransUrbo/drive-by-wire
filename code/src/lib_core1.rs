use defmt::{info, unwrap};

use embassy_executor::Spawner;

use crate::lib_can_bus::{read_can, write_can};
use crate::lib_resources::{PeriPowerMonitor, PeriWatchdog};
use crate::lib_ups::ups_monitor;
use crate::lib_watchdog::feed_watchdog;

#[embassy_executor::task]
pub async fn core1_tasks(spawner: Spawner, watchdog: PeriWatchdog, ups: PeriPowerMonitor) {
    info!("Spawning tasks on CORE1");

    #[cfg_attr(any(), rustfmt::skip)]
    {
        spawner.spawn(unwrap!(feed_watchdog(watchdog)));	// Spawn Watchdog.
        spawner.spawn(unwrap!(read_can()));			// Spawn the CAN reader.
        spawner.spawn(unwrap!(write_can()));			// Spawn the CAN writer.
        spawner.spawn(unwrap!(ups_monitor(ups)));		// Spawn the UPS monitor.
    }
}
