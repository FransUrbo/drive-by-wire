use defmt::{info, unwrap};

use embassy_executor::Spawner;
use embassy_rp::watchdog::*;
use embassy_time::Duration;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Receiver
};

use crate::lib_actuator::actuator_control;
use crate::lib_buttons::Button;
use crate::lib_can_bus::{read_can, CANMessage, CHANNEL_CANWRITE};
use crate::lib_watchdog::{feed_watchdog, CHANNEL_WATCHDOG};
use crate::lib_resources::PeriWatchdog;

use actuator::Actuator;

#[embassy_executor::task]
pub async fn core1_tasks(
    spawner: Spawner,
    receiver: Receiver<'static, CriticalSectionRawMutex, Button, 64>,
//    flash: &'static FlashMutex,
    actuator: Actuator<'static>,
    watchdog: PeriWatchdog
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
    // Spawn the Actuator controller.
    spawner.spawn(unwrap!(actuator_control(
        receiver,
//        flash,
        actuator
    )));
    info!("Actuator controller running");
    CHANNEL_CANWRITE.send(CANMessage::ActuatorInitialized).await;

    // -----
    // Spawn the CAN reader.
    spawner.spawn(unwrap!(read_can()));
    info!("CAN bus reader runing");
}
