use assign_resources::assign_resources;
use embassy_rp::{peripherals, Peri};

// Offset from the flash start, NOT absolute address.
pub const ADDR_OFFSET: u32 = 0x100000;
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

pub const UPS_ADDRESS: u8 = 0x43;

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
        lock:		PIN_21,
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
    can: PeriCan {
        // CAN Interface ICs:
        //   * MCP2518FDT-E/SL - Stand-alone Low Power CAN FD Controller w/SPI Interface Grade1.
        //   * TJA1055T/1J     - High-speed CAN transceiver.
        send_pin:	PIN_19,		// MOSI (Master Out Slave In)
        send_dma:	DMA_CH5,
        recv_pin:	PIN_16,		// MISO (Master In Slave Out)
        recv_dma:	DMA_CH6,
        csn_pin:	PIN_17,
        sck_pin:	PIN_18,
        spi:		SPI0		// Serial Peripheral Interface
    },
    ups: PeriPowerMonitor {
        sda:		PIN_6,
        scl:		PIN_7,
        i2c:		I2C1
    }
}

// # Pins:
// * PIN_0	PeriFPScanner:send_pin
// * PIN_1	PeriFPScanner:recv_pin
// * PIN_2	PeriButtons:p_but
// * PIN_3	PeriButtons:r_but
// * PIN_4	PeriSerial:tx
// * PIN_5	PeriSerial:rx		Unused
// * PIN_6	PeriPowerMonitor:sda
// * PIN_7	PeriPowerMonitor:scl
// * PIN_8	PeriButtons:n_led
// * PIN_9	PeriButtons:d_led
// * PIN_10	PeriActuator:mplus
// * PIN_11	PeriActuator:mminus
// * PIN_12	PeriActuator:vsel	Unused
// * PIN_13	PeriFPScanner:wakeup
// * PIN_14	PeriButtons:p_led
// * PIN_15	PeriNeopixel:pin
// * PIN_16	PeriCan:recv_pin
// * PIN_17	PeriCan:csn_pin
// * PIN_18	PeriCan:sck_pin
// * PIN_19	PeriCan:send_pin
// * PIN_20	PeriButtons:r_led
// * PIN_21	PeriEis:PIN_21
// * PIN_22	PeriEis:start
// * PIN_23				Unknown
// * PIN_24				Unknown
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
// * DMA_CH5	PeriCan:send_dma
// * DMA_CH6	PeriCan:recv_dma
//
// # UART
// * UART0	PeriFPScanner:uart
// * UART1	PeriSerial:uart
//
// # Other
// * PIO0	PeriNeopixel:pio
// * SPI0	PeriCan:spi
// * ADC	PeriActuator:adc
// * I2C1	PeriPowerMonitor:i2c
// * FLASH	PeriFlash:peri
// * WATCHDOG	PeriWatchdog:peri
