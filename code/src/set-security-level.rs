#![no_std]
#![no_main]

// !! Fingerprint scanner is on PIO0, and the NeoPixel is on PIO1 !!

use defmt::{debug, error, info};

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{PIO1, UART0};
use embassy_rp::pio::{InterruptHandler as PIOInterruptHandler, Pio};
use embassy_rp::uart::InterruptHandler as UARTInterruptHandler;

use {defmt_rtt as _, panic_probe as _};

use ws2812::{Colour, Ws2812};
use r503::{R503, Parameters, SecurityLevels, Status};

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
    let mut r503 = R503::new(
        p.UART0,
        Irqs,
        p.PIN_16,
        p.DMA_CH0,
        p.PIN_17,
        p.DMA_CH1,
        p.PIN_13.into(),
    );
    info!("Fingerprint scanner initialized");

    // Initialize the multi-colour LED.
    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO1, Irqs);
    let mut ws2812 = Ws2812::new(&mut common, sm0, p.DMA_CH3, p.PIN_15);
    info!("NeoPixel LED initialized");

    // =====
    debug!("NeoPixel BLUE");
    ws2812.set_colour(Colour::BLUE).await;

    if !r503.Wrapper_Setup().await {
        error!("Can't setup scanner");
        return;
    } else {
        match r503.SetSysPara(Parameters::SecurityLevel as u8, SecurityLevels::One as u8).await {
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
                error!("Unknown return code='{=u8:#04x}': Wrapper_Setup()/SetSysPara()", stat as u8);

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
