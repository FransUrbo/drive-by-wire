use defmt::debug;

use embassy_sync::channel::{Channel, Receiver};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_time::Timer;

pub enum CANMessage { Starting, InitFP, FPInitialized, InitActuator, ActuatorInitialized, Authorizing, Authorized }
pub static CHANNEL_CANWRITE: Channel<ThreadModeRawMutex, CANMessage, 64> = Channel::new();

// Write messages to CAN-bus.
#[embassy_executor::task]
pub async fn write_can(receiver: Receiver<'static, ThreadModeRawMutex, CANMessage, 64>) {
    debug!("Started CAN write task");

    loop {
	let message = receiver.receive().await; // Block waiting for data.
	match message {
	    CANMessage::Starting		=> {
		debug!("Sending message to IC: 'Starting Drive-By-Wire system'");
	    }
	    CANMessage::InitFP			=> {
		debug!("Sending message to IC: 'Initializing Fingerprint Scanner'");
	    }
	    CANMessage::FPInitialized		=> {
		debug!("Sending message to IC: 'Fingerprint scanner initialized'");
	    }
	    CANMessage::InitActuator		=> {
		debug!("Sending message to IC: 'Initializing actuator'");
	    }
	    CANMessage::ActuatorInitialized	=> {
		debug!("Sending message to IC: 'Actuator initialized'");
	    }
	    CANMessage::Authorizing		=> {
		debug!("Sending message to IC: 'Authorizing use'");
	    }
	    CANMessage::Authorized		=> {
		debug!("Sending message to IC: 'Use authorized'");
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

