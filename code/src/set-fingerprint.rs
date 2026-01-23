#![no_std]
#![no_main]

use defmt::{debug, error, info};

use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    peripherals::{PIO0, UART0},
    pio::{InterruptHandler as PIOInterruptHandler, Pio},
    uart::InterruptHandler as UARTInterruptHandler,
};
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

use r503::R503;
use ws2812::{Colour, Ws2812};

pub mod lib_resources;
use crate::lib_resources::{
    AssignedResources, PeriSerial, PeriBuiltin, PeriNeopixel, PeriWatchdog, PeriSteering,
    PeriStart, PeriFlash, PeriActuator, PeriFPScanner, PeriButtons
};

// For our commented out 'Empty()' below, in case we need it again.
#[allow(unused_imports)]
use r503::Status;

bind_interrupts!(pub struct Irqs {
    PIO0_IRQ_0 => PIOInterruptHandler<PIO0>;	// NeoPixel
    UART0_IRQ  => UARTInterruptHandler<UART0>;	// Fingerprint scanner
});

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
        r.fpscan.wakeup.into()
    );

    // Initialize the multi-colour LED.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(r.neopixel.pio, Irqs);
    let mut ws2812 = Ws2812::new(&mut common, sm0, r.neopixel.dma, r.neopixel.pin);

    debug!("NeoPixel OFF");
    ws2812.set_colour(Colour::BLACK).await;
    Timer::after_secs(1).await;

    //match r503.Empty().await {
    //    Status::CmdExecComplete => {
    //        info!("Library emptied");
    //    }
    //    stat => {
    //        info!("Return code: '{=u8:#04x}'", stat as u8);
    //    }
    //}

    // =====
    loop {
        debug!("NeoPixel BLUE");
        ws2812.set_colour(Colour::BLUE).await;

        if !r503.Wrapper_Enrole_Fingerprint(0x0002).await {
            error!("Can't enrole fingerprint");

            debug!("NeoPixel RED");
            ws2812.set_colour(Colour::RED).await;

            Timer::after_secs(5).await;
        } else {
            info!("Fingerprint enrolled");

            debug!("NeoPixel GREEN");
            ws2812.set_colour(Colour::GREEN).await;
            return;
        }
    }
}
