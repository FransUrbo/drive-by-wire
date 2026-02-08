use defmt::{debug, error, info, unwrap};

use embassy_executor::Spawner;
use embassy_rp::{
    gpio::{Level, Output},
    peripherals::SPI0,
    spi::{Async, Config, Spi},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, mutex::Mutex};
use embassy_time::Timer;

use static_cell::StaticCell;

use crate::lib_resources::PeriCan;
use crate::lib_buttons::{ButtonState, CHANNEL_BUTTON_STATE};

pub enum CANMessage {
    Starting,
    InitFP,
    FPInitialized,
    InitActuator,
    ActuatorInitialized,
    ActuatorTestFailed,
    RelaysInitialized,
    ButtonsInitialized,
    ValetMode,
    EnableValetMode,
    DisableValetMode,
    StartCar,
    Authorizing,
    Authorized,
}

pub static CHANNEL_CANWRITE: Channel<CriticalSectionRawMutex, CANMessage, 64> = Channel::new();

type SpiBus = Mutex<CriticalSectionRawMutex, Spi<'static, SPI0, Async>>;

#[embassy_executor::task]
pub async fn can_manager(spawner: Spawner, can: PeriCan) {
    let mut spi_cfg = Config::default();
    spi_cfg.frequency = 12_000_000u32; // External high-speed crystal on the pico board is 12Mhz.

    let spi = Spi::new(
        can.spi,
        can.sck_pin,
        can.send_pin,
        can.recv_pin,
        can.send_dma,
        can.recv_dma,
        spi_cfg,
    );
    static SPI_BUS: StaticCell<SpiBus> = StaticCell::new();
    let spi = SPI_BUS.init(Mutex::new(spi));

    // TODO: Do I need two pins for this - one for the writer and one for the reader?
    let _cs = Output::new(can.csn_pin, Level::High);

    spawner.spawn(unwrap!(read_can(spi))); // Spawn the CAN reader.
    spawner.spawn(unwrap!(write_can(spi))); // Spawn the CAN writer.
}

// Write messages to CAN-bus.
#[embassy_executor::task]
pub async fn write_can(_spi: &'static SpiBus) {
    info!("CAN bus writer running");

    loop {
        let message = CHANNEL_CANWRITE.receive().await; // Block waiting for data.
        match message {
            CANMessage::Starting => {
                info!("=> 'Starting Drive-By-Wire system'");
            }
            CANMessage::InitFP => {
                info!("=> 'Initializing Fingerprint Scanner'");
            }
            CANMessage::FPInitialized => {
                info!("=> 'Fingerprint scanner initialized'");
            }
            CANMessage::InitActuator => {
                info!("=> 'Initializing actuator'");
            }
            CANMessage::ActuatorInitialized => {
                info!("=> 'Actuator initialized'");
            }
            CANMessage::ActuatorTestFailed => {
                error!("=> 'Actuator failed to move'");
            }
            CANMessage::RelaysInitialized => {
                info!("=> 'Relays initialized");
            }
            CANMessage::ButtonsInitialized => {
                info!("=> 'Drive buttons initialized'");
            }
            CANMessage::ValetMode => {
                info!("=> 'Valet mode, won't authorize use'");
            }
            CANMessage::EnableValetMode => {
                info!("=> 'Valet Mode Enabled'");
            }
            CANMessage::DisableValetMode => {
                info!("=> 'Valet Mode Disabled'");
            }
            CANMessage::Authorizing => {
                info!("=> 'Authorizing use'");
            }
            CANMessage::Authorized => {
                info!("=> 'Use authorized'");
            }
            CANMessage::StartCar => {
                info!("=> 'Sending start signal to car'");
            }
        }
    }
}

// Read CAN-bus messages.
#[embassy_executor::task]
pub async fn read_can(_spi: &'static SpiBus) {
    // Become a publisher on the button state channel.
    let publisher = CHANNEL_BUTTON_STATE.publisher().unwrap();

    info!("CAN bus reader running");

    // TODO: How do we know if we're on battery power?
    //       If we are, we should *not* enable buttons here, no matter what.
    loop {
        // TODO: Just test that this works.
        debug!("Testing button state publisher");
        publisher.publish_immediate(ButtonState::Stop);
        Timer::after_secs(15).await;
        publisher.publish_immediate(ButtonState::Start);

        // TODO: Read CAN-bus messages (blocking).

        // TODO: If we're moving, disable buttons.
        //publisher.publish_immediate(ButtonState::Stop);

        // TODO: If we're NOT moving, and brake pedal is NOT depressed, disable buttons.
        //publisher.publish_immediate(ButtonState::Stop);

        // TODO: If we're NOT moving, and brake pedal is depressed, enable buttons.
        //publisher.publish_immediate(ButtonState::Start);

        //Timer::after_secs(600).await; // TODO: Nothing to do yet, just sleep as long as we can, but 10 minutes should do it.
        Timer::after_secs(45).await;
    }
}
