#![no_std]
#![no_main]

// !! Fingerprint scanner is on PIO0, and the NeoPixel is on PIO1 !!

use defmt::{debug, error, info};

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{PIO1, UART0};
use embassy_rp::pio::{InterruptHandler as PIOInterruptHandler, Pio};
use embassy_rp::uart::InterruptHandler as UARTInterruptHandler;
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

use ws2812::{Colour, Ws2812};

// For our commented out 'Empty()' below, in case we need it again.
#[allow(unused_imports)]
use r503::Status;

bind_interrupts!(pub struct Irqs {
    PIO1_IRQ_0 => PIOInterruptHandler<PIO1>;	// NeoPixel
    UART0_IRQ  => UARTInterruptHandler<UART0>;	// Fingerprint scanner
});

// ================================================================================

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Start");

    let p = embassy_rp::init(Default::default());

    // Initialize the fingerprint scanner.
    let mut r503 = r503::R503::new(
        p.UART0,
        Irqs,
        p.PIN_16,
        p.DMA_CH0,
        p.PIN_17,
        p.DMA_CH1,
        p.PIN_13.into(),
    );

    // Initialize the multi-colour LED.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO1, Irqs);
    let mut ws2812 = Ws2812::new(&mut common, sm0, p.DMA_CH3, p.PIN_15);

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

        if ! r503.Wrapper_Enrole_Fingerprint(0x0002).await {
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
