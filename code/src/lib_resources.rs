use assign_resources::assign_resources;
use embassy_rp::{peripherals, Peri};

// Offset from the flash start, NOT absolute address.
pub const ADDR_OFFSET: u32 = 0x100000;
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

pub const UPS_ADDRESS: u8 = 0x43; // I²C address to the UPS hat.
pub const SPI_ADDRESS: u8 = 0x00; // I²C address to the I²C to SPI bridge. TODO

#[cfg_attr(any(), rustfmt::skip)]
assign_resources! {
    serial: PeriSerial {
        uart:		UART1,
        dma:		DMA_CH4,
        tx:		PIN_4,
        rx:		PIN_5
    },
    builtin: PeriBuiltin {
        pin:		PIN_25
    },
    neopixel: PeriNeopixel {
        pio:		PIO0,
        dma:		DMA_CH2,
        pin:		PIN_15
    },
    watchdog: PeriWatchdog {
        peri:		WATCHDOG
    },
    eis: PeriEis {
        lock:		PIN_16,
        switch:		PIN_21,
        start:		PIN_22
    },
    flash: PeriFlash {
        peri:   	FLASH,
        dma:    	DMA_CH3
    },
    actuator: PeriActuator {
        adc:		ADC,
        mplus:		PIN_10,
        mminus:		PIN_11,
        vsel:		PIN_12,
        pot:		PIN_28
    },
    fpscan: PeriFPScanner {
        uart:		UART0,
        send_pin:	PIN_0,		// UART0
        send_dma:	DMA_CH0,
        recv_pin:	PIN_1,		// UART0
        recv_dma:	DMA_CH1,
        wakeup:		PIN_13		// UART0
    },
    buttons: PeriButtons {
        p_but:		PIN_2,
        p_led:		PIN_14,
        r_but:		PIN_3,
        r_led:		PIN_20,
        n_but:		PIN_26,		// UART0
        n_led:		PIN_8,		// UART1
        d_but:		PIN_27,		// UART0
        d_led:		PIN_9		// UART1
    },
    i2c: PeriI2C {
        peri:		I2C1,
        sda:		PIN_6,
        scl:		PIN_7
    }
}

// # Pins:
// * PIN_0	PeriFPScanner:send_pin
// * PIN_1	PeriFPScanner:recv_pin
// * PIN_2	PeriButtons:p_but
// * PIN_3	PeriButtons:r_but
// * PIN_4	PeriSerial:tx
// * PIN_5	PeriSerial:rx		Unused
// * PIN_6	PeriI2C:sda
// * PIN_7	PeriI2C:scl
// * PIN_8	PeriButtons:n_led
// * PIN_9	PeriButtons:d_led
// * PIN_10	PeriActuator:mplus
// * PIN_11	PeriActuator:mminus
// * PIN_12	PeriActuator:vsel
// * PIN_13	PeriFPScanner:wakeup
// * PIN_14	PeriButtons:p_led
// * PIN_15	PeriNeopixel:pin
// * PIN_16	PeriEis:lock
// * PIN_17				Unused
// * PIN_18				Unused
// * PIN_19				Unused
// * PIN_20	PeriButtons:r_led
// * PIN_21	PeriEis:switch
// * PIN_22	PeriEis:start
// * PIN_23				Unused
// * PIN_24				Unused
// * PIN_25	PeriBuiltin:pin
// * PIN_26	PeriButtons:n_but
// * PIN_27	PeriButtons:d_but
// * PIN_28	PeriActuator:pot
//
// # DMA:
// * DMA_CH0	PeriFPScanner:send_dma
// * DMA_CH1	PeriFPScanner:recv_dma
// * DMA_CH2	PeriNeopixel:dma
// * DMA_CH3	PeriFlash:dma
// * DMA_CH4	PeriSerial:dma
//
// # UART
// * UART0	PeriFPScanner:uart
// * UART1	PeriSerial:uart
//
// # Other
// * PIO0	PeriNeopixel:pio
// * ADC	PeriActuator:adc
// * I2C1	PeriI2C:peri
// * FLASH	PeriFlash:peri
// * WATCHDOG	PeriWatchdog:peri
