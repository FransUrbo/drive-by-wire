use assign_resources::assign_resources;
use embassy_rp::{peripherals, Peri};

// Offset from the flash start, NOT absolute address.
pub const ADDR_OFFSET: u32 = 0x100000;
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

assign_resources! {
    serial: PeriSerial {
        uart: UART1,
        dma: DMA_CH4,
        tx: PIN_4,
        rx: PIN_5
    },
    builtin: PeriBuiltin {
        pin: PIN_25
    },
    neopixel: PeriNeopixel {
        pio: PIO0,
        dma: DMA_CH2,
        pin: PIN_15
    },
    watchdog: PeriWatchdog {
        peri: WATCHDOG
    },
    eis: PeriEis {
        lock: PIN_19,
        start: PIN_22
    },
    flash: PeriFlash {
        peri: FLASH,
        dma: DMA_CH3
    },
    actuator: PeriActuator {
        adc: ADC,
        mplus: PIN_10,
        mminus: PIN_11,
        vsel: PIN_12,
        pot: PIN_28
    },
    fpscan: PeriFPScanner {
        uart: UART0,
        send_pin: PIN_16,  // UART0
        send_dma: DMA_CH0,
        recv_pin: PIN_17, // UART0
        recv_dma: DMA_CH1,
        wakeup: PIN_13  // UART0
    },
    buttons: PeriButtons {
        p_but: PIN_2,
        p_led: PIN_14,
        r_but: PIN_3,
        r_led: PIN_18,
        n_but: PIN_0, // UART0
        n_led: PIN_8, // UART1
        d_but: PIN_1, // UART0
        d_led: PIN_9 // UART1
    },
    can: PeriCan {
        tx: PIN_20,
        rx: PIN_21
    },
    ups: PeriPowerMonitor {
        sda: PIN_6,
        scl: PIN_7,
        i2c: I2C1
    }
}

// Free pins:
// * PIN_26,
// * PIN_27
