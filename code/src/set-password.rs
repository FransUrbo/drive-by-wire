#![no_std]
#![no_main]

use defmt::{debug, error, info};

use embassy_executor::Spawner;
use embassy_rp::pio::Pio;
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

use r503::{Status, R503};
use ws2812::{Colour, Ws2812};

pub mod lib_resources;
use crate::lib_resources::*;

// ================================================================================

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let r = split_resources! {p};

    info!("Start");

    // Initialize the fingerprint scanner.
    let mut r503 = R503::new(
        r.fpscan.uart,
        Irqs,
        r.fpscan.send_pin,
        r.fpscan.send_dma,
        r.fpscan.recv_pin,
        r.fpscan.recv_dma,
        r.fpscan.wakeup.into(),
    );
    r503.password = 0x00000000; // CURRENT password.
    let new_pw = 0x00100000; // NEW password.

    // Initialize the multi-colour LED.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(r.neopixel.pio, Irqs);
    let mut ws2812 = Ws2812::new(&mut common, sm0, r.neopixel.dma, Irqs, r.neopixel.pin);

    debug!("NeoPixel OFF");
    ws2812.set_colour(Colour::BLACK).await;
    Timer::after_secs(1).await;

    debug!("NeoPixel ON");
    ws2812.set_colour(Colour::BLUE).await;
    Timer::after_secs(1).await;

    // First verify the old passwor - "login" as it where.
    match r503.VfyPwd(r503.password).await {
        Status::CmdExecComplete => {
            info!("Fingerprint scanner password matches");
            ws2812.set_colour(Colour::GREEN).await;
        }
        Status::ErrorReceivePackage => {
            error!("Package receive");
            ws2812.set_colour(Colour::ORANGE).await;
        }
        Status::ErrorPassword => {
            error!("Wrong password");
            ws2812.set_colour(Colour::RED).await;
        }
        stat => {
            info!("ERROR: code='{=u8:#04x}'", stat as u8);
        }
    }

    // .. then change it
    match r503.SetPwd(new_pw).await {
        Status::CmdExecComplete => {
            info!("Fingerprint scanner password set");
            ws2812.set_colour(Colour::GREEN).await;
        }
        Status::ErrorReceivePackage => {
            error!("package receive");
            ws2812.set_colour(Colour::ORANGE).await;
        }
        Status::ErrorPassword => {
            error!("Wrong password");
            ws2812.set_colour(Colour::RED).await;
        }
        stat => {
            error!("code='{=u8:#04x}'", stat as u8);
            ws2812.set_colour(Colour::RED).await;
        }
    }
}
