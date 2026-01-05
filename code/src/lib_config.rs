use defmt::{debug, error, info, trace, Format};

use embassy_rp::flash::{Async, Error, Flash, ERASE_SIZE};
use embassy_rp::peripherals::FLASH;

// External "defines".
use crate::Button;

// Offset from the flash start, NOT absolute address.
const ADDR_OFFSET: u32 = 0x100000;
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

// What we store in flash.
#[derive(Format)]
pub struct DbwConfig {
    pub active_button: Button,
    pub valet_mode: bool,
}

impl DbwConfig {
    fn as_array(&self) -> [u8; 2] {
        [self.active_button as u8, self.valet_mode as u8]
    }

    pub fn read(flash: &mut Flash<'_, FLASH, Async, FLASH_SIZE>) -> Result<DbwConfig, Error> {
        let mut read_buf = [0u8; ERASE_SIZE];

        match flash.blocking_read(ADDR_OFFSET + ERASE_SIZE as u32, &mut read_buf) {
            Ok(_) => {
                info!("Flash read successful");

                // Translate the u8's.
                let active_button = match read_buf[0] {
                    0 => Button::P,
                    1 => Button::R,
                    2 => Button::N,
                    3 => Button::D,
                    _ => Button::P, // Never going to happen, but just to keep the compiler happy with resonable default
                };

                let valet_mode = match read_buf[1] {
                    0 => false,
                    1 => true,
                    _ => true, // Never going to happen, but just to keep the compiler happy with resonable default
                };

                Ok(DbwConfig {
                    active_button,
                    valet_mode,
                })
            }
            Err(e) => {
                error!("Flash read failed: {}", e);

                // Still return ok, but with resonable default instead.
                Ok(resonable_defaults())
            }
        }
    }

    pub fn write(
        flash: &mut Flash<'_, FLASH, Async, FLASH_SIZE>,
        config: Self,
    ) -> Result<(), Error> {
        // Convert our struct to an array, so we can loop through it easier.
        let buf: [u8; 2] = config.as_array();

        for (j, b) in buf.into_iter().enumerate() {
            match flash.blocking_write(ADDR_OFFSET + ERASE_SIZE as u32 + j as u32, &[b] as &[u8]) {
                Ok(_) => trace!("Flash write {} successful", j),
                Err(e) => {
                    error!("Flash write {} failed: {}", j, e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

pub async fn write_flash(flash: &mut Flash<'_, FLASH, Async, FLASH_SIZE>, buf: DbwConfig) {
    trace!("write_flash({:?})", buf);

    match DbwConfig::read(flash) {
        Ok(v) => debug!("Config (before write): {:?}", v),
        Err(e) => error!("Failed to read (before write): {:?}", e),
    }

    match flash.blocking_erase(
        ADDR_OFFSET + ERASE_SIZE as u32,
        ADDR_OFFSET + ERASE_SIZE as u32 + ERASE_SIZE as u32,
    ) {
        Ok(_) => trace!("Flash erase successful"),
        Err(e) => error!("Flash erase failed: {}", e),
    }

    match DbwConfig::write(flash, buf) {
        Ok(_) => info!("Config update successful"),
        Err(e) => error!("Config update failed: {}", e),
    }

    match DbwConfig::read(flash) {
        Ok(v) => debug!("Config (after write): {:?}", v),
        Err(e) => error!("Failed to read (before write): {:?}", e),
    }
}

pub fn resonable_defaults() -> DbwConfig {
    DbwConfig {
        active_button: Button::P,
        valet_mode: false,
    }
}
