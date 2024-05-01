#![no_std]
#![no_main]
#![allow(unused)]

use defmt::info;
use embassy_executor::Spawner;

use ws2312;
use debounce;
use r503;

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Start");
}
