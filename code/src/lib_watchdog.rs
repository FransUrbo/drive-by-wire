use defmt::{error, info};

use embassy_rp::watchdog::Watchdog;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Channel,
};
use embassy_time::{Duration, Timer};

use crate::lib_resources::PeriWatchdog;

pub enum StopWatchdog {
    Yes,
}

pub static CHANNEL_WATCHDOG: Channel<CriticalSectionRawMutex, StopWatchdog, 64> = Channel::new();

// Doggy is hungry, needs to be feed every three quarter second, otherwise it gets cranky! :)
#[embassy_executor::task]
pub async fn feed_watchdog(doggy: PeriWatchdog) {
    info!("Starting watchdog feeder task");

    let mut watchdog = Watchdog::new(doggy.peri);
    watchdog.start(Duration::from_millis(1_050));

    // Feed the watchdog every 3/4 second to avoid reset.
    loop {
        match CHANNEL_WATCHDOG.try_receive() {
            // Only *if* there's data, receive and deal with it.
            Ok(StopWatchdog::Yes) => {
                error!("StopWatchdog = Yes received");
                return;
            }
            _ => {
                Timer::after_millis(750).await;
                watchdog.feed();
                continue;
            }
        }
    }
}
