use defmt::{Format, debug, error, info, trace};

use embassy_rp::flash::{Async, Error, ERASE_SIZE};
use embassy_rp::peripherals::FLASH;

// Offset from the flash start, NOT absolute address.
const ADDR_OFFSET: u32 = 0x100000;
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

// What we store in flash.
#[derive(Format)]
pub struct DbwConfig {
    pub active_button:	u8,
    pub valet_mode:	u8
}

impl DbwConfig {
    pub fn as_array(&self) -> [u8; 2] {
	[self.active_button, self.valet_mode]
    }

    pub fn read(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>) -> Result<DbwConfig, Error> {
	let mut read_buf = [0u8; ERASE_SIZE];

	match flash.blocking_read(ADDR_OFFSET + ERASE_SIZE as u32, &mut read_buf) {
	    Ok(_) => {
		debug!("Flash read successful");
		return Ok(DbwConfig { active_button: read_buf[0], valet_mode: read_buf[1] });
	    }
	    Err(e) => {
		error!("Flash read failed: {}", e);
		return Err(e);
	    }
	}
    }

    pub fn write(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>, config: Self) -> Result<(), Error> {
	// Convert our struct to an array, so we can loop through it easier.
	let buf: [u8; 2] = config.as_array();

	let mut j = 0; // Keep track of offset in flash.
	for i in 0..buf.len() {
	    match flash.blocking_write(ADDR_OFFSET + ERASE_SIZE as u32 + j, &[buf[i]]) {
		Ok(_)  => trace!("Flash write {} successful", j),
		Err(e) => {
		    error!("Flash write {} failed: {}", j, e);
		    return Err(e);
		}
	    }

	    j = j + 1;
	}

	Ok(())
    }
}

pub async fn write_flash(flash: &mut embassy_rp::flash::Flash<'_, FLASH, Async, FLASH_SIZE>, buf: DbwConfig) {
    trace!("write_flash({:?})", buf);

    match DbwConfig::read(flash) {
	Ok(v)  => debug!("Config (before write): {:?}", v),
	Err(e) => error!("Failed to read (before write): {:?}", e)
    }

    match flash.blocking_erase(
	ADDR_OFFSET + ERASE_SIZE as u32,
	ADDR_OFFSET + ERASE_SIZE as u32 + ERASE_SIZE as u32)
    {
	Ok(_)  => trace!("Flash erase successful"),
	Err(e) => error!("Flash erase failed: {}", e)
    }

    match DbwConfig::write(flash, buf) {
	Ok(_)  => info!("Config update successful"),
	Err(e) => error!("Config update failed: {}", e)
    }

    match DbwConfig::read(flash) {
	Ok(v)  => debug!("Config (after write): {:?}", v),
	Err(e) => error!("Failed to read (before write): {:?}", e)
    }
}
