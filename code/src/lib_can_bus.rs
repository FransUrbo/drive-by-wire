use defmt::{debug, error, info};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::Timer;

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

// Write messages to CAN-bus.
#[embassy_executor::task]
pub async fn write_can(receiver: Receiver<'static, CriticalSectionRawMutex, CANMessage, 64>) {
    debug!("Started CAN write task");

    loop {
        let message = receiver.receive().await; // Block waiting for data.
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
pub async fn read_can() {
    debug!("Started CAN read task");

    loop {
        // TODO: Read CAN-bus messages (blocking).

        // TODO: If we're moving, disable buttons.

        // TODO: If we're NOT moving, and brake pedal is NOT depressed, disable buttons.

        // TODO: If we're NOT moving, and brake pedal is depressed, enable buttons.

        Timer::after_secs(600).await; // TODO: Nothing to do yet, just sleep as long as we can, but 10 minutes should do it.
    }
}
