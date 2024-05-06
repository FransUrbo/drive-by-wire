#![no_std]
#![no_main]

use defmt::{debug, info, error};

use embassy_executor::Spawner;
use embassy_rp::peripherals::{PIO1, UART0};
use embassy_rp::uart::InterruptHandler as UARTInterruptHandler;
use embassy_rp::pio::{InterruptHandler as PIOInterruptHandler, Pio};
use embassy_rp::bind_interrupts;
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

use ws2812;
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
    let mut r503 = r503::R503::new(p.UART0, Irqs, p.PIN_16, p.DMA_CH0, p.PIN_17, p.DMA_CH1, p.PIN_13.into());
    r503.password = 0x00000000; // CURRENT password.
    let new_pw    = 0x00100000;	// NEW password.

    // Initialize the multi-colour LED.
    let Pio { mut common, sm0, .. } = Pio::new(p.PIO1, Irqs);
    let mut ws2812 = ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH3, p.PIN_15);

    debug!("NeoPixel OFF");
    ws2812.write(&[(0,0,0).into()]).await;
    Timer::after_secs(1).await;

    debug!("NeoPixel ON");
    ws2812.write(&[(0,0,255).into()]).await; // BLUE
    Timer::after_secs(1).await;

    // First verify the old passwor - "login" as it where.
    match r503.VfyPwd(r503.password).await {
	Status::CmdExecComplete => {
	    info!("Fingerprint scanner password matches");
	    ws2812.write(&[(0,255,0).into()]).await; // GREEN
	}
	Status::ErrorReceivePackage => {
	    error!("Package receive");
	    ws2812.write(&[(255,130,0).into()]).await; // ORANGE
	}
	Status::ErrorPassword => {
	    error!("Wrong password");
	    ws2812.write(&[(255,0,0).into()]).await; // RED
	}
	stat => {
	    info!("ERROR: code='{=u8:#04x}'", stat as u8);
	}
    }

    // .. then change it
    match r503.SetPwd(new_pw).await {
	Status::CmdExecComplete => {
	    info!("Fingerprint scanner password set");
	    ws2812.write(&[(0,255,0).into()]).await; // GREEN
	}
	Status::ErrorReceivePackage => {
	    error!("package receive");
	    ws2812.write(&[(255,130,0).into()]).await; // ORANGE
	}
	Status::ErrorPassword => {
	    error!("Wrong password");
	    ws2812.write(&[(255,0,0).into()]).await; // RED
	}
	stat => {
	    error!("code='{=u8:#04x}'", stat as u8);
	}
    }
}
