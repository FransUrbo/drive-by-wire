use defmt::{debug, info};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::Timer;

pub enum StopWatchdog {
    Yes,
}

pub static CHANNEL_WATCHDOG: Channel<CriticalSectionRawMutex, StopWatchdog, 64> = Channel::new();

// Doggy is hungry, needs to be feed every three quarter second, otherwise it gets cranky! :)
#[embassy_executor::task]
pub async fn feed_watchdog(
    control: Receiver<'static, CriticalSectionRawMutex, StopWatchdog, 64>,
    mut wd: embassy_rp::watchdog::Watchdog,
) {
    debug!("Started watchdog feeder task");

    // Feed the watchdog every 3/4 second to avoid reset.
    loop {
        match control.try_receive() {
            // Only *if* there's data, receive and deal with it.
            Ok(StopWatchdog::Yes) => {
                info!("StopWatchdog = Yes received");
                return;
            }
            _ => {
                Timer::after_millis(750).await;
                wd.feed();
                continue;
            }
        }
    }
}
