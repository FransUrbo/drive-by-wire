use defmt::{info, unwrap};

use embassy_executor::Spawner;

use crate::lib_can_bus::can_manager;
use crate::lib_resources::{PeriCan, PeriPowerMonitor, PeriWatchdog};
use crate::lib_ups::ups_monitor;
use crate::lib_watchdog::feed_watchdog;

#[embassy_executor::task]
pub async fn core1_tasks(
    spawner: Spawner,
    watchdog: PeriWatchdog,
    ups: PeriPowerMonitor,
    can: PeriCan,
) {
    info!("Spawning tasks on CORE1");

    #[cfg_attr(any(), rustfmt::skip)]
    {
        spawner.spawn(unwrap!(feed_watchdog(watchdog)));	// Spawn Watchdog.
        spawner.spawn(unwrap!(can_manager(spawner, can)));	// Spawn the CAN manager.
        spawner.spawn(unwrap!(ups_monitor(ups)));		// Spawn the UPS monitor.
    }
}
