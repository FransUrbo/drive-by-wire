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

use {defmt_rtt as _, panic_probe as _};

use r503::{Parameters, SecurityLevels, Status, R503};
use ws2812::{Colour, Ws2812};

pub mod lib_resources;
use crate::lib_resources::*;

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
        r.fpscan.wakeup.into(),
    );
    info!("Fingerprint scanner initialized");

    // Initialize the multi-colour LED.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(r.neopixel.pio, Irqs);
    let mut ws2812 = Ws2812::new(&mut common, sm0, r.neopixel.dma, r.neopixel.pin);
    info!("NeoPixel LED initialized");

    // =====
    debug!("NeoPixel BLUE");
    ws2812.set_colour(Colour::BLUE).await;

    if !r503.Wrapper_Setup().await {
        error!("Can't setup scanner");
        return;
    } else {
        match r503
            .SetSysPara(Parameters::SecurityLevel as u8, SecurityLevels::One as u8)
            .await
        {
            Status::CmdExecComplete => {
                info!("Security level set");

                debug!("NeoPixel GREEN");
                ws2812.set_colour(Colour::GREEN).await;

                r503.Wrapper_AuraSet_Off().await;
            }
            Status::ErrorReceivePackage => {
                error!("Package receive: Wrapper_Setup()/SetSysPara()");

                debug!("NeoPixel RED");
                ws2812.set_colour(Colour::RED).await;

                r503.Wrapper_AuraSet_BlinkinRedMedium().await;

                return;
            }
            stat => {
                error!(
                    "Unknown return code='{=u8:#04x}': Wrapper_Setup()/SetSysPara()",
                    stat as u8
                );

                debug!("NeoPixel RED");
                ws2812.set_colour(Colour::RED).await;

                r503.Wrapper_AuraSet_Off().await;

                return;
            }
        }

        match r503.ReadSysPara().await {
            Status::CmdExecComplete => {
                info!("System parameters read");
                return;
            }
            Status::ErrorReceivePackage => {
                error!("Package receive: Wrapper_Setup()/ReadSysPara()");

                r503.Wrapper_AuraSet_BlinkinRedMedium().await;
                return;
            }
            stat => {
                error!(
                    "Unknown return code='{=u8:#04x}': Wrapper_Setup()/ReadSysPara()",
                    stat as u8
                );

                r503.Wrapper_AuraSet_Off().await;
                return;
            }
        }
    }
}
