use defmt::{info, unwrap};

use embassy_executor::Spawner;
use embassy_rp::watchdog::*;
use embassy_time::Duration;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Receiver,
};

use crate::lib_can_bus::{read_can, write_can, CANMessage};
use crate::lib_watchdog::{feed_watchdog, CHANNEL_WATCHDOG};
use crate::lib_resources::{PeriWatchdog, PeriPowerMonitor};
use crate::lib_ups::ups_monitor;

#[embassy_executor::task]
pub async fn core1_tasks(
    spawner: Spawner,
    receiver: Receiver<'static, CriticalSectionRawMutex, CANMessage, 64>,
    watchdog: PeriWatchdog,
    ups: PeriPowerMonitor
) {
    info!("Spawning tasks on CORE1");

    // -----
    // Spawn Watchdog.
    let mut watchdog = Watchdog::new(watchdog.peri);
    watchdog.start(Duration::from_millis(1_050));
    spawner.spawn(unwrap!(feed_watchdog(
        CHANNEL_WATCHDOG.receiver(),
        watchdog
    )));
    info!("Watchdog timer running");

    // -----
    // Spawn the CAN reader.
    spawner.spawn(unwrap!(read_can()));
    info!("CAN bus reader running");

    // -----
    // Spawn the CAN writer.
    spawner.spawn(unwrap!(write_can(receiver)));
    info!("CAN bus reader running");

    // -----
    // Spawn the UPS monitor.
    spawner.spawn(unwrap!(ups_monitor(ups)));
    info!("UPS monitor running");
}
