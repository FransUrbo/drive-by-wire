use defmt::{debug, error, info, trace, Format};

use embassy_rp::{
    flash::{Async, Flash, ERASE_SIZE},
    peripherals::FLASH,
};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    channel::Channel as SyncChannel,
    mutex::Mutex,
};

use static_cell::StaticCell;

// External "defines".
use crate::lib_resources::{PeriFlash, ADDR_OFFSET, FLASH_SIZE};
use crate::Button;

pub type FlashMutex = Mutex<NoopRawMutex, Flash<'static, FLASH, Async, FLASH_SIZE>>;

#[derive(PartialEq)]
pub enum FlashConfigMessages {
    ValetOn,
    ValetOff,
    ButtonP,
    ButtonR,
    ButtonN,
    ButtonD,
    ReadValet,
    ReadButton,
}

impl FlashConfigMessages {
    // Translate a button to config message.
    pub fn from(b: Button) -> FlashConfigMessages {
        match b {
            Button::P => FlashConfigMessages::ButtonP,
            Button::R => FlashConfigMessages::ButtonR,
            Button::N => FlashConfigMessages::ButtonN,
            Button::D => FlashConfigMessages::ButtonD,
        }
    }

    pub fn to_button(m: &Self) -> Button {
        match m {
            Self::ButtonP => Button::P,
            Self::ButtonR => Button::R,
            Self::ButtonN => Button::N,
            Self::ButtonD => Button::D,
            _ => Button::P,
        }
    }

    pub fn to_valet(m: Self) -> bool {
        match m {
            Self::ValetOn => true,
            Self::ValetOff => false,
            _ => false,
        }
    }
}

pub static CHANNEL_FLASH: SyncChannel<CriticalSectionRawMutex, FlashConfigMessages, 64> =
    SyncChannel::new();

// What we store in flash.
#[derive(Format)]
pub struct DbwConfig<'d> {
    flash: Flash<'d, FLASH, Async, FLASH_SIZE>,
    active_button: Button,
    valet_mode: bool,
}

impl<'d> DbwConfig<'d> {
    pub fn new(r: PeriFlash) -> Self {
        info!("Initializing the flash drive");

        let f = Flash::<_, Async, FLASH_SIZE>::new(r.peri, r.dma);

        Self {
            flash: f,
            active_button: Button::P,
            valet_mode: false,
        }
    }

    // Translate a button to config message.
    pub fn from(&mut self, b: Button) -> FlashConfigMessages {
        match b {
            Button::P => FlashConfigMessages::ButtonP,
            Button::R => FlashConfigMessages::ButtonR,
            Button::N => FlashConfigMessages::ButtonN,
            Button::D => FlashConfigMessages::ButtonD,
        }
    }

    fn as_array(&mut self) -> [u8; 2] {
        [self.active_button as u8, self.valet_mode as u8]
    }

    fn read(&mut self) -> bool {
        let mut read_buf = [0u8; ERASE_SIZE];

        match self
            .flash
            .blocking_read(ADDR_OFFSET + ERASE_SIZE as u32, &mut read_buf)
        {
            Ok(_) => {
                debug!("Flash read successful");

                // Translate the u8's.
                self.active_button = match read_buf[0] {
                    0 => Button::P,
                    1 => Button::R,
                    2 => Button::N,
                    3 => Button::D,
                    _ => Button::P, // Never going to happen, but just to keep the compiler happy with resonable default
                };

                self.valet_mode = match read_buf[1] {
                    0 => false,
                    1 => true,
                    _ => false, // Never going to happen, but just to keep the compiler happy with resonable default
                };

                return true;
            }
            Err(e) => {
                error!("Flash read failed: {}", e);
                return false;
            }
        }
    }

    fn write(&mut self) -> bool {
        // Read the flash..
        if self.read() {
            debug!(
                "Config (before write): active_button={:?}, valet_mode={:?}",
                self.active_button, self.valet_mode
            );
        } else {
            error!("Failed to read (before write)");
            return false;
        }

        // Erase the flash..
        match self.flash.blocking_erase(
            ADDR_OFFSET + ERASE_SIZE as u32,
            ADDR_OFFSET + ERASE_SIZE as u32 + ERASE_SIZE as u32,
        ) {
            Ok(_) => trace!("Flash erase successful"),
            Err(e) => {
                error!("Flash erase failed: {}", e);
                return false;
            }
        }

        // Write the flash..
        let buf: [u8; 2] = self.as_array(); // Convert our struct to an array, so we can loop through it easier.
        for (j, b) in buf.into_iter().enumerate() {
            match self
                .flash
                .blocking_write(ADDR_OFFSET + ERASE_SIZE as u32 + j as u32, &[b] as &[u8])
            {
                Ok(_) => trace!("Flash write {} successful", j),
                Err(e) => {
                    error!("Flash write {} failed: {}", j, e);
                    return false;
                }
            }
        }

        if self.read() {
            debug!(
                "Config (after write): active_button={:?}, valet_mode={:?}",
                self.active_button, self.valet_mode
            );
        } else {
            error!("Failed to read (before write)");
            return false;
        }

        return true;
    }
}

#[embassy_executor::task]
pub async fn flash_control(r: PeriFlash) {
    let mut config = DbwConfig::new(r);

    config.read(); // Read the values from the flash before we begin.
    loop {
        match CHANNEL_FLASH.receive().await {
            // We're asked to *change* a value.
            FlashConfigMessages::ValetOn => config.valet_mode = true,
            FlashConfigMessages::ValetOff => config.valet_mode = false,

            FlashConfigMessages::ButtonP => config.active_button = Button::P,
            FlashConfigMessages::ButtonR => config.active_button = Button::R,
            FlashConfigMessages::ButtonN => config.active_button = Button::N,
            FlashConfigMessages::ButtonD => config.active_button = Button::D,

            // We're asked to *read* (and return!) a value.
            // TODO: How do we send to the one messaging us!?
            FlashConfigMessages::ReadValet => {
                debug!(
                    "Message: FlashConfigMessages::ReadValet: {}",
                    config.valet_mode
                );

                // TODO: This just messages ourselves!
                // if config.valet_mode {
                //     CHANNEL_FLASH.send(FlashConfigMessages::ValetOn).await;
                // } else {
                //     CHANNEL_FLASH.send(FlashConfigMessages::ValetOff).await;
                // }
            }

            FlashConfigMessages::ReadButton => {
                debug!(
                    "Message: FlashConfigMessages::ReadButton: {}",
                    config.active_button
                );

                // TODO: This just messages ourselves!
                // match config.active_button {
                //     Button::P => CHANNEL_FLASH.send(config.from(Button::P)).await,
                //     Button::R => CHANNEL_FLASH.send(config.from(Button::R)).await,
                //     Button::N => CHANNEL_FLASH.send(config.from(Button::N)).await,
                //     Button::D => CHANNEL_FLASH.send(config.from(Button::D)).await,
                // };
            }
        };

        // TODO: We're actually writing the new value *after* we've told the sender the (new) value!
        config.write();
    }
}

pub fn init_flash(r: PeriFlash) -> &'static FlashMutex {
    let flash = Flash::<_, Async, FLASH_SIZE>::new(r.peri, r.dma);
    static FLASH: StaticCell<FlashMutex> = StaticCell::new();
    let flash: &'static FlashMutex = FLASH.init(Mutex::new(flash));

    return flash;
}
