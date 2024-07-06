# Introduction

This is to introduce drive-by-wire buttons for Mercedes-Benz. Specifically, it is for my Mercedes-Benz SLK55 AMG.

![drive-by-wire buttons](./AstonMartin%20Drive%20Buttons.jpg)

## Page index

1. [Fingerprint scanner instead of Start button](#fingerprint-scanner-instead-of-start-button)
2. [Software function](#software-function)
3. [Pin layout](#pin-layout-for-raspberrypi-3-5-and-pico)
   1. [External contacts](#external-contacts)
      - [Buttons and their LEDs](#buttons-and-their-leds)
      - [Status LED](#status-led)
      - [Fingerprint scanner](#fingerprint-scanner)
      - [EIS Relay](#eis-relays)
      - [Actuator](#actuator)
      - [CAN bus](#can-bus-0)
      - [Total](#total)
      - [Leads](#leads)
        - [Actuator/Relays Lead](#actuator-relays-lead)
        - [EIS/Relay Lead](#eis-relays-lead)
        - [Control Lead](#control-lead)
        - [CAN-Bus Lead](#can-bus-lead)
   2. [Parts](#parts)
      - [Actuation](#actuation)
      - [Controller](#controller)
      - [CAN bus](#can-bus)
      - [Connectors](#connectors)
      - [For development](#for-development)
      - [Other](#other)
      - [Small footprint controllers](#small-footprint-controllers)
        - [Notes about the small footprint controllers](#notes-about-the-small-footprint-controllers)
   3. [Circuit diagram](#circuit-diagram)
      - [Wiring on bread boards](#wiring-on-bread-boards)
      - [Latest wiring on bread boards](#latest-wiring-on-bread-boards)
   4. [PCB](#pcb)
4. [Source code](#source-code)
   - [Code testing and setup](#code-testing-and-setup)
   - [DriveByWire code](#drivebywire-code)
     - [Relays](#relays)
     - [Status LED](#status-led)
5. [Updates](#updates)
   - [Update Sun 2 May 2024](#update-sun-2-may-2024)
   - [Update Sun 3 May 2024](#update-sun-3-may-2024)
   - [Update Sun 4 May 2024](#update-sun-4-may-2024)
   - [Update Sun 5 May 2024](#update-sun-5-may-2024)
   - [Update Sun 6 May 2024](#update-sun-6-may-2024)
   - [Update Sun 8 May 2024](#update-sun-8-may-2024)
   - [Update Sun 9 May 2024](#update-sun-9-may-2024)
   - [Update Thu 27 Jun 2024](#update-thu-27-jun-2024)
   - [Update Sun 30 Jun 2024](#update-sun-30-jun-2024)
   - [Update Mon 1 Jun 2024(#update-mon-1-jun-2024)
6. [Additional information](#additional-information)

## Fingerprint scanner instead of Start button

Instead of the start button (which would be *really* cool, but isn't feasable due to the way the EIS - Electronic Ignition
Switch - works with the rest of the car), I'm putting a panel mounted fingerprint scanner instead. So this will double
as an extra security feature.

The idea is to put the key fob in, turn it to position 2 ("ignition on"), but if anyone tries to turn the key fob to
position 3 ("start") without a valid fingerprint, nothing would happen. This because there's a relay between the "start"
cable and the rest of the car that is default open.

To be able to start the car, a valid fingerprint must be entered, the device will authorize this, and then close this
relay, allowing the car to start.

HOPEFULLY, I can also send the same trigger signal that the EIS would send from the device.

# Software function

> At any point, if there is a fail, set the RED LED blinking and stop further program execution.

1. Bootup process.
     1. Light status LED (RED).												-> BOOTUP STARTED
     2. Initiate CAN bus connection.										Q: How to test connection?
         - Send message to IC: "Starting Drive-By-Wire system".
     3. Initiate fingerprint scanner connection.
         1. Send message to IC: "Initializing Fingerprint Scanner".
         2. Login to and unlock the fingerprint scanner.					CodeFunction: `VfyPwd`
         3. Validate talking to the correct fingerprint scanner.
             - Do handshake.												CodeFunction: `HandShake`.
             - Check if sensor is normal.									CodeFunction: `CheckSensor`.
             - Check correct random string in the Notepad buffer.			CodeFunction: `ReadNotepad`.
             - Light fingerprint scanner LED (PURPLE).						CodeFunction: `AuraLedConfig`.
         4. Send message to IC: "Fingerprint scanner initialized".
     4. Initiate and test actuator connection and control.
         1. Send message to IC: "Initializing actuator".
         2. Get status (current position) of actuator.
         3. LOOP: Check actuator connection and function.
             - Get speed status from CAN.									Q: How to double-check??
             - Get break pedal status from CAN.								Q: How to double-check??
             - If ('not moving' && 'break pedal pressed') OR ('P'selected):
                 - Move actuator back 1mm.
                     - Validate correct movement.
                 - Move actuator forward 1mm.
                     - Validate correct movement.
                 - Check that "before test" and "current position" is the same.
             - else: restart loop.
         4. Send message to IC: "Actuator initialized".
     5. Light current drive button locator LED.
     6. Light status LED (YELLOW).											-> BOOTUP DONE + LOGIN STARTED

2. Check authorization.
     1. Send message to IC: "Authorizing use".
     2. Check valet mode.
         1. If false:
             - Verify fingerprint
                 - If not verified:
                     - Light status LED (RED).								-> FAILED LOGIN
                     - Light fingerprint scanner LED (RED/FLASH).			CodeFunction: `AuraLedConfig`.
                     - If attempts >= 3: sleep for 5min.
                     - else: restart loop.
                 - else:
                     - Turn off fingerprint scanner LED.					CodeFunction: `AuraLedConfig`.
                     - Light status LED (GREEN).							-> LOGIN DONE + MAIN LOOP STARTED
         2. else:
             - Light status LED (BLUE).										-> LOGIN DONE + MAIN LOOP STARTED
     3. Close EIS relay #1 (ignition switch).								Q: What if power loss??
     4. Close EIS relay #2 (steering lock).									Q: What if power loss??
     5. Send message to IC: "Use authorized, welcome <user|valet>".
     6. Send "start car" voltage signal to SAM.

3. LOOP: Wait for drive button press.
     1. If moving:
         - If true:  ignore button press (restart loop).
     2. If break pedal is pressed:
         - If false: ignore button press (restart loop).
     3. If NEW button != CURRENT button.
         1. Get current position of actuator.
         2. Blink NEW drive button telltale LED.
         3. Move actuator to new position (synchronous).
         4. Get current position of actuator.
         5. Check that "before change" and "current position" have changed.
         6. Turn off CURRENT drive button telltale LED.
         7. Set NEW drive buttons telltale LED.

Q: How can the DriveByWire, SmartTOP and SprintBooster all be
   set in valet mode all at the same time?<br>
Q: Can DriveByWire check CAN for certain buttons around the car
   to be pressed in sequence just like GhostImmobiliser??

# Pin layout for RaspberryPI 3-5, and Pico

| Pin | Port    | Use                          |-| Pin | Port     | Use
| --: | :------ | :--------------------------- |-| --: | :------- | :----------------------------- |
|   1 | GPIO  0 | Button (Switch - N)          |-| 40  | VBUS     |                                |
|   2 | GPIO  1 | Button (Switch - D)          |-| 39  | VSYS     |                                |
|   3 | GND     |                              |-| 38  | GND      |                                |
|   4 | GPIO  2 | Button (Switch - P)          |-| 37  | 3V3_EN   |                                |
|   5 | GPIO  3 | Button (Switch - R)          |-| 36  | 3V3_OUT  |                                |
|   6 | GPIO  4 | Debug (TX)                   |-| 35  | ADC_VREF | Actuator - +5V                 |
|   7 | GPIO  5 | Debug (RX)                   |-| 34  | ADC2     |                                |
|   8 | GND     |                              |-| 33  | AGND     | Actuator - GND                 |
|   9 | GPIO  6 | Button (Telltale - P)        |-| 32  | ADC1     |                                |
|  10 | GPIO  7 | Button (Telltale - R)        |-| 31  | ADC0     | Actuator - Potentiometer Brush |
|  11 | GPIO  8 | Button (Telltale - N)        |-| 30  | RUN      |                                |
|  12 | GPIO  9 | Button (Telltale - D)        |-| 29  | GPIO 22  | EIS Relay (#3 - start)         |
|  13 | GND     | *[GPIO 29]*                  |-| 28  | GND      | *[GPIO 23]*                    |
|  14 | GPIO 10 | Actuator - Motor Relay (#1)  |-| 27  | GPIO 21  | CAN #0 (RX)                    |
|  15 | GPIO 11 | Actuator - Motor Relay (#2)  |-| 26  | GPIO 20  | CAN #0 (TX)                    |
|  16 | GPIO 12 | Actuator - +5V/+12V select   |-| 25  | GPIO 19  | EIS Relay (#1 - steering lock) |
|  17 | GPIO 13 | Fingerprint Scanner (WAKEUP) |-| 24  | GPIO 18  |                                |
|  18 | GND     | *[GPIO 25]*                  |-| 23  | GND      | *[GPIO 24]*                    |
|  19 | GPIO 14 | Status LED (Data OUT)        |-| 22  | GPIO 17  | Fingerprint Scanner (TX)       |
|  20 | GPIO 15 | Status LED (Data IN)         |-| 21  | GPIO 16  | Fingerprint Scanner (RX)       |

LED | GPIO 25

[Olimex RP2040-PICO30-16](https://thepihut.com/products/olimex-rp2040-pico30-16) also exposes GPIO 23-25, 29 by sacrificing four GROUND pins.

The different uses are specifically this way, because I need to consider UARTs/PIOs/ADC etc, and which pins they
have connected to them. So it's not as .. "pretty" and simple as just throwing them in there and start using
the ports..

## External contacts

### Buttons and their LEDs

* 4x Buttons (Switch)
* 4x Button LEDs (Telltale)
* 1x GND

=> 9-pin

### Status LED

* 2x Status LED (Data IN+OUT)
* 1x 3V3

=> 3-pin

Or those two combined, [buttons and their LEDs and the status LED](https://www.ebay.co.uk/itm/174775342997).

### Fingerprint scanner

* 1x WAKEUP
* 2x Data (TX+RX)
* 1x GND
* 1x 3V3 (power)
* 1x 3V3 (touch induction power)

=> 6-pin

### EIS Relays

* 1x Control #2 (steering lock)
* 1x Control #3 (start signal)
* 1x 5V
* 1x GND

=> 5-pin

### Actuator

* 1x Actuator (Potentiometer Brush)
* 1x Actuator (Motor Relay +)
* 1x Actuator (Motor Relay -)
* 1x ADC 5V
* 1x ADC GND

=> 5-pin

### CAN bus #0

* 1x CAN-L
* 1x CAN-H

=> 2-pin

### Total

33 leads out from system - 26, counting only unique pins (IgnitionSwitch "relay" and CAN#2 not counted, because
those shouldn't be anyway).

### Leads

Pin on the big motherboard connector and where it goes..

#### Actuator/Relays Lead
*  2: GND					(for Actuator Motor GND)
*  4: +12V					(for Actuator Motor +12V)
*  6: +5V					(for Actuator Motor +5V)
* 10: ACTUATOR/MOTOR-RELAY_PLUS			(signal control)
* 12: ACTUATOR/MOTOR-RELAY_MINUS		(signal control)
* 14: ADC_VREF					(signal)
* 16: ADC_GND					(signal)
* 18: ACTUATOR/POTENTIOMETER-BRUSH		(signal control)
* 20: ACTUATOR/VOLTAGE-RELAY_SELECT (+5V/+12V)	(signal control)

=> 10-lead

#### EIS/Relay Lead
*  2: GND					(for relay GND)
*  8: +3V					(for relay +3V)
* 22: EIS/START					(signal control)
* 24: EIS/STEERING-LOCK				(signal control)

=> 4-lead

#### Control Lead
*  2: GND					(for status LED and fingerprint scanner GND)
*  4: +12V					(for button locate LEDs)
*  6: +5V					(for status LED +5V)
*  8: +3V					(for fingerprint scanner +3V)
* 26: FP/TX					(signal control)
* 23: FP/RX					(signal control)
* 17: FP/WAKEUP					(signal control)
* 21: STATUS_LED/IN				(signal control)
* 19: STATUS_LED/OUT				(signal control)
* 15: BUTTON_LED/D				(signal control)
* 13: BUTTON_LED/N				(signal control)
* 11: BUTTON_LED/R				(signal control)
*  9: BUTTON_LED/P				(signal control)
*  7: BUTTON/D					(signal control)
*  5: BUTTON/N					(signal control)
*  3: BUTTON/R					(signal control)
*  1: BUTTON/P					(signal control)

=> 16-lead

#### CAN-Bus Lead
* 27: CAN-L					(signal control)
* 25: CAN-H					(signal control)

=> 2-lead

# Parts

Crossed out parts are things I either didn't buy or don't need. Yet. Which is why I'm leaving them, but cross them out.

I'm saving the list of [components for the PCB](https://www.mouser.com/ProjectManager/ProjectDetail.aspx?AccessID=d9a3d6382c)
in a project list at Mouser. It's still a work in progress, so not quite correct yet.

## Actuation

| Part | Price |
| :--- | :---  |
| [Actuator w/ feedback (potentiometer)](https://www.progressiveautomations.com/products/linear-actuator-with-potentiometer?variant=18277322424387) | £115 ($145) |
| [Aston Martin drive select buttons](https://www.ebay.co.uk/sch/i.html?_dkr=1&iconV2Request=true&_blrs=recall_filtering&_ssn=hillsmotors&store_name=hillsmotors&_oac=1&_nkw=gear%20select%20switch) | £35	(*4 = £140) |

## Controller

| Part | Price |
| :--- | :---  |
| [Raspberry Pi Pico (w/ headers)](https://thepihut.com/products/raspberry-pi-pico?variant=41925332566211) | £5 |
| [DC-DC Buck Converter 7-24V to 5V 4A](https://thepihut.com/products/dc-dc-buck-converter-7-24v-to-5v-4a?variant=39865627607235) | £5 |
| [Fingerprint scanner (panel mount)](https://thepihut.com/products/panel-mount-fingerprint-sensor-with-bi-colour-led-ring-r503?variant=41727311675587) | £23 |
| [MOSFET Power Controller](https://thepihut.com/products/gravity-mosfet-power-controller) | £4 * 3 |
| [NeoPixel Diffused 5mm LED](https://thepihut.com/products/neopixel-diffused-5mm-through-hole-led-5-pack?variant=27739696529) | £5 |
| [LED holder 5mm](https://thepihut.com/products/5mm-plastic-flat-led-holder-pack-of-5?variant=27739774353) | £1 |

## CAN bus

| Part | Price |
| :--- | :---  |
| [TJA1055T/1J Fault-tolerant CAN chip](https://www.mouser.co.uk/ProductDetail/771-TJA1055T-1J) | £2
| [MCP2513FDTE/SL CAN Interface IC](https://www.mouser.co.uk/ProductDetail/579-MCP2518FDT-E-SL) | £2
| [DC Power Connector](https://www.mouser.co.uk/ProductDetail/502-RASM722X) | £1
| [Resistor -  1kΩ](https://www.mouser.co.uk/ProductDetail/710-560112132038) | £0.10 * 7
| [Resistor - 10kΩ](https://www.mouser.co.uk/ProductDetail/710-560112116005) | £0.09
| [Capacitor - 150pF](https://www.mouser.co.uk/ProductDetail/80-C1206C151F1G) | £0.08 * 2
| [Resonator - 16MHz/15pF](https://www.mouser.co.uk/ProductDetail/81-CSTNE16M0V530000R) | £0.30

## Connectors

| Part | Price |
| :--- | :---  |
| ~~[Motherboard connector - 24pin/vertical](https://www.mouser.co.uk/ProductDetail/538-213227-2410)~~ | ~~£3~~
| ~~[Motherboard connector - 24pin/horizontal](https://www.mouser.co.uk/ProductDetail/538-503148-2490)~~ | ~~£3~~
| ~~[Motherboard connector - 26pin/vertical](https://www.mouser.co.uk/ProductDetail/538-503148-2690)~~ | ~~£3~~
| ~~[Headers & Wire Housings - 24pin](https://www.mouser.co.uk/ProductDetail/538-503149-2400)~~ | ~~£0.6~~
| ~~[Headers & Wire Housings - 26pin](https://www.mouser.co.uk/ProductDetail/538-503149-2600)~~ | ~~£0.8~~
| ~~[Wire to Motherboard connector - 24pin](https://www.mouser.co.uk/ProductDetail/538-503148-2490)~~ | ~~£3~~
| ~~[Panel mount connector - 24 pin](https://www.mouser.co.uk/ProductDetail/798-DF51-24DEP-2C)~~ | ~~£1~~
| ~~[Wire to panel mount connector - 24pin](https://www.mouser.co.uk/ProductDetail/798-DF51-24DS-2C)~~ | ~~£0.3~~
| ~~[Molex MiniFit Jr Housing - 2x3pin](https://www.mouser.co.uk/ProductDetail/538-39-30-6068)~~ | ~~£1~~
| [Debug connector - 5pin/vertical](https://www.mouser.co.uk/ProductDetail/538-53398-0567) | £0.6
| [DC Power Connector](https://www.mouser.co.uk/ProductDetail/502-RASM722X) | £1.3
| [FPC Connector - 26 pin](https://www.mouser.co.uk/ProductDetail/538-52207-2660) | £2.4
| [Ribbon cable - 26 core](https://www.mouser.co.uk/ProductDetail/906-100R26-76B) | £2.8

Don't think I'm going to buy the panel and wire to panel connectors. The motherboard connector looks quite big, so it might
be better to just stick that out through the box. I'll leave them in here for now, because I might change my mind.

I want this thing to be as small as possible, easier to hide it somewhere in the car then :). I have yet to
decide if I want a vertical or a horizontal connector..

Actually, I found a nice connector on eBay ([IP68 Aviation Plug Socket](https://www.ebay.co.uk/itm/154107555672?var=454247628623)
for £21 (which is a bit much, but it's nice! :). It's quite big, but think it'll be nice to be able to lock
the connector tight. It doesn't technically need to be water proof, but it can get moist in the cabin..

For now, I think that's the best option. This allows me to use a ribbon cable between the motherboard
and that chassis connector, which minimizes the size of the motherboard connector.

![PCB - Side view (1)](./PCB%20-%20Side%20%281%29.png)
![PCB - Side view (2)](./PCB%20-%20Side%20%282%29.png)

## For development

| Part | Price |
| :--- | :---  |
| [Raspberry Pi Debug Probe](https://thepihut.com/products/raspberry-pi-debug-probe?variant=42380171870403) | £12
| [120-Piece Ultimate Jumper Bumper Pack](https://thepihut.com/products/thepihuts-jumper-bumper-pack-120pcs-dupont-wire?variant=13530244284478) | £6
| [575-Piece Ultimate Resistor Kit](https://thepihut.com/products/ultimate-resistor-kit?variant=36476117073) | £6
| [Half-Size Breadboard](https://thepihut.com/products/breadboard-400-point-clear?variant=31986026381374) | £3 * 3
| [Breadboard for Pico](https://thepihut.com/products/breadboard-for-pico?variant=39819276386499) | £4
| [Short Plug Headers](https://thepihut.com/products/short-plug-headers-for-raspberry-pi-pico-2-x-20-pin-male?variant=42182974505155) | £1
| [Tactile Switch Buttons](https://thepihut.com/products/tactile-switch-buttons-6mm-tall-x-10-pack?variant=27739414097) | £3
| [Breakout for 6-pin JST SH-Style Connector - Side Entry](https://thepihut.com/products/breakout-for-6-pin-jst-sh-style-connector-side-entry?variant=42438253871299) | £1
| [Extra-long break-away 0.1" 16-pin strip male header (5 pieces)](https://thepihut.com/products/extra-long-break-away-0-1-16-pin-strip-male-header-5-pieces?variant=27740420881) | £3
| [220V Power Supply Adapter (12V/10A)](https://www.ebay.co.uk/itm/234147120198?var=533767190848) | £21
| [DB9 Breakout Board PCB – Male](https://thepihut.com/products/db9-breakout-board-pcb-male?variant=41727856148675) | £2
| [Breadboard-friendly 2.1mm DC barrel jack](https://thepihut.com/products/breadboard-friendly-2-1mm-dc-barrel-jack?variant=27740417489) | £1
| [In-line power switch for 2.1mm barrel jack](https://thepihut.com/products/in-line-power-switch-for-2-1mm-barrel-jack?variant=27739226065) | £2
| [DB9 Right Angle MALE Connector - PCB Mount D-SUB](https://www.ebay.co.uk/itm/325261653847) | £3
| [Dupont Jump Wire F-F Jumper Breadboard Cable Lead -  6pin](https://www.ebay.co.uk/itm/275827705804?var=577580216871) | £2
| [Dupont Jump Wire F-F Jumper Breadboard Cable Lead - 10pin](https://www.ebay.co.uk/itm/275827705804?var=577580216855) | £2
| [Dupont Jump Wire M-M Jumper Breadboard Cable Lead - 10cm](https://www.ebay.co.uk/itm/275268807202?var=575537821821) | £8
| [SO14 IC to Breadboard adapter](https://www.mouser.co.uk/ProductDetail/535-LCQT-SOIC14) | £4 * 2
| [Molex MiniFit Jr connector + cable - 2x3pin, 600mm](https://www.mouser.co.uk/ProductDetail/538-215328-1063) | £6.7

## Other

These aren't things needed, but maybe I'll have a need for them one day..

| Part | Price |
| :--- | :---  |
| [1 Channel Relay for RPi](https://thepihut.com/products/grove-relay?variant=40341004746947) | £3 |
| [2 Channel Relay Breakout - 12v](https://thepihut.com/products/2-channel-relay-breakout-12v) | £8 |
| [2 Channel Relay Breakout -  5v](https://thepihut.com/products/2-channel-relay-breakout-5v) | £7 |
| [2 Channel Isolated Relay Breakout - 12v](https://thepihut.com/products/2-channel-isolated-relay-breakout-12v) | £12 |
| [2 Channel Isolated Relay Breakout -  5v](https://thepihut.com/products/2-channel-isolated-relay-breakout-5v) | £13 |
| [4 Channel Relay Breakout](https://thepihut.com/products/4-channel-relay-breakout-12v) | £16 |
| [2 Channel Latching Relay - 5V](https://thepihut.com/products/grove-2-coil-latching-relay) | £7 |
| [9A/28V SPDT MOSFET Switch](https://thepihut.com/products/moswitch-9a-28v-spdt-mosfet-switch) | £5 |
| [DC-DC Buck-Mode Power Module (8-28V to 5V 1.6A)](https://thepihut.com/products/dc-dc-buck-mode-power-module-8-28v-to-5v-1-6a) | £3 |
| [DC-DC Buck-Mode Power Module (5.5-28V to 3.3V 2.4A)](https://thepihut.com/products/dc-dc-buck-mode-power-module-5-5-28v-to-3-3v-2-4a) | £3 |
| [5V Buck Converter Unit (ME3116AM6G)](https://thepihut.com/products/5v-buck-converter-unit-me3116am6g) | £4 |
| [High Precision Capacitive Fingerprint Reader](https://thepihut.com/products/high-precision-capacitive-fingerprint-reader-b) | £61 |
| [I2C GPIO Expander](https://thepihut.com/products/adafruit-pcf8574-i2c-gpio-expander-breakout-stemma-qt-qwiic) | £5 |
| [MRK CAN Shield Arduino](https://www.pcbway.com/project/shareproject/MRK_CAN_Shield_Arduino_133f7666.html) | - |

## Small footprint controllers

The Pico is for development. Makes things easier when it's in a bigger format. However, some of these below might be used for
the actual "production" device. But to get all the GPIO needed, an I2C GPIO expander (see above) would be needed.

Some of the signals I need will probably be to fast for the I2C bus, so those would have to come in through the board GPIO,
not the I2C GPIOs. But then, the whole setup will be bigger anyway (because of the expander), so might just stick with the
Pico anyway. Besides, the whole circuit board is (going to be) about the size of my palm anyway.

| Part | Price | Note
| :--- | :---  | :---
| [Seeed XIAO RP2040](https://thepihut.com/products/seeed-xiao-rp2040) | £6 | 11 GPIO pins
| [Tiny 2040](https://thepihut.com/products/tiny-2040?variant=41359025897667) | £12 | 12 GPIO pins
| [Waveshare RP2040 Tiny](https://thepihut.com/products/waveshare-rp2040-tiny?variant=42483599507651) | £5 | 20 GPIO pins

### Notes about the small footprint controllers

As can be seen from the [pin layout](#pin-layout-for-raspberrypi-3-5-and-pico), I need more than this!
At the moment, I have TWO GPIO to spare (and two GPIO/ADC) of the 26 pins that the RPi's have!

I might even have to go with the [Olimex RP2040-PICO30-16](https://thepihut.com/products/olimex-rp2040-pico30-16) which
have an additional four GPIO pins by sacrificing four GROUND pins..

But seems like the design have stabilized now. UNLESS the CAN bus adapters I'm going to have to get need more than TX/RX.
Don't know which ones to get yet, still work in progres.

# Circuit diagram

This is still work in progress, but this looks about right. That's what I've wired on the breadboard.

![Circuit diagram](./Circuit%20Diagram.png)

This is the diagram for the Aston Martin drive button. I only use the one switch within the button (SW2).

![AstonMartin Button Diagram](./AstonMartin%20Button%20Diagram.png)

The latest version of the [ciruit diagram](https://a360.co/3L8P7J7) and the [PCB](https://a360.co/3XMMkwH) can be accessed
on the Fusion360 site. It's not very pretty (the web viewer have .. issues :). I always try to keep this repo updated with
the very latest by using screenshots, but it's also available online. I'm using a free subscription, and can't enable downloads,
so it's unfortunately view-only..

My [components library](https://a360.co/4cwNAcc) can also be found on that site. Not sure how usefull it is though..

## Wiring on bread boards

Those CAN bus adaptors I can't apparently used. They're for a high-speed CAN, but the MB I have have a low-frequency,
fault-tolerant CAN :(. Something using the TJA1055T1 chip for the CAN and a MCP2515 for interfacing with the Pico. I'll
figure something out..

Also, I'm missing the headers for the fingerprint scanner and the actuator in the upper right breadboard.

![Initial wiring on bread boards](./2024-04-20%2021.50.37.jpg)

### Latest wiring on bread boards

This what it looks like now. I built a box :D :D.

![Latest wiring on bread boards](./2024-05-04%2011.09.21.jpg)

## PCB

With the help of Fusion360, something I've used a few years now (which is free for students and personal use!), I've
managed to create a PCB layout.

It'll probably won't be the last, and I'm not sure if it's valid (not sure about all these signal lines!), but here
they are anyway.

![PCB - Bottom (without components)](./PCB%20-%20Bottom%20%281%29.png)
![PCB - Bottom (with components)](./PCB%20-%20Bottom%20%282%29.png)
![PCB - Top (without components)](./PCB%20-%20Top%20%281%29.png)
![PCB - Top (with components)](./PCB%20-%20Top%20%282%29.png)

Fusion360 can even generate a 3D object of the PCB! Very pretty! :D

![PCB - Bottom (3D)](./PCB%20-%20Bottom%20%283D%29.png)
![PCB - Top (3D)](./PCB%20-%20Top%20%283D%29.png)

.. and the perspective view.

![PCB - Bottom (3D - Perspective)](./PCB%20-%20Bottom%20%283D%20-%20Perspective%29.png)
![PCB - Top (3D - Perspective)](./PCB%20-%20Top%20%283D%20-%20Perspective%29.png)

# Source code

## Code testing and setup

These are my tests of the individual functionality that I wrote leading up to this project.
I have most of it working, the only major thing that's missing is the CAN-bus code and
hardware.

* [How to control the LEDs, including the NeoPixel (multi-colour LED)](https://github.com/FransUrbo/pico-rust-test_1-LEDS)
* [How to read the buttons and control *their* LEDs](https://github.com/FransUrbo/pico-rust-test_2-BUTTONS-LEDS)
  Yes, those are the genuine Aston Martin drive buttons! :).
* [How to read, write and verify fingerprint with the fingerprint scanner](https://github.com/FransUrbo/pico-rust-test_3-FP_SCANNER)
* [How to control the three MOSFET "relays"](https://github.com/FransUrbo/pico-rust-test_4-MOSFET_RELAYS)
* [How to setup and trigger the built-in watchdog on the RPi](https://github.com/FransUrbo/pico-rust-test_6-WATCHDOG-LED)

## DriveByWire code

The actual DriveByWire source code is getting underway, it's in [the code directory](./code).
I verify fingerprint, read buttons, turn on LEDs correctly and I move the actuator back and forth.

I do seem to have an issue with the hardware. "Something" is resetting the Pico every now and then.
It is *likely* "something" to do with the actuator, not sure what. The guess is that it's a spike, a
feedback from it or that it draws so much power that the power supply I'm using can't take it, drops
the power to much (or to fast?) that either the DC-DC converter I have or the Pico can't handle it
and resets.

This was a [recording of the screen](https://www.dropbox.com/scl/fi/bi3qf4g1nu1k6bnatyuem/Screen-Recording-2024-05-03-at-20.10.48.mov?rlkey=vi5vw7pl20p2h9n0wq28tuy4a&st=hylgbs2c&dl=0) while it was running.
It's a day old, and I've done some modifications to the code since them, but this demostrates it fairly
well.

And this is what it [looked like in action](https://www.dropbox.com/scl/fi/nsdj958atposke2wdfzk9/2024-05-03-20.12.23.mov?rlkey=e7vu1sx3g0xffbefloeaspzul&st=kw81bt22&dl=0).
They where to big for GitHub, so had to put them on my Dropbox account.

### Relays

The yellow LED in the upper left corner of the box, that's connected to the MOSFET "relay", is "start the car".
I *THINK* it is enough to send +12V to the ECU (**E**ngine **C**ontrol **U**nit) on the "start position" pin.
We'll see, have to do some experimentation. In this car, I only have to trigger it for the ECU to take over and
start the car. So I'm only turning on the MOSFET "relay" for a second. Should be more than enough.

**NOTE**: There might be an issue with the "blue" (the steering lock on/off) and the "green" (ignition switch
on/off) relays. I'm not sure what happens if (when!?) power is lost to the device, OR if it crashes and the
watchdog reboots it - for a few seconds those relays will be "off"! What happens if I'm driving and the igntion
switch is disabled and the steering lock is enabled!?? The former might not be a problem, although the ECU
might .. get confused. But the latter, the steering lock, if that's enabled,
as in locks the steering wheel from turning, that will be **BAD**!!!

I've considered using relays that stays in position and need a trigger to switch, but that might also cause
problems - if they're "on" and I've turned the car off and walked away, then they'll do no good!

### Status LED

The steady orange (well, it's not very orange, is it!?? :) and then green LED in the top middle is orange =>
"starting up" (the module) and when it turns green, it means "all is well". Had the fingerprint not matched,
it and the aura around the fingerprint scanner turns red.

# Updates

## Update Sun 2 May 2024

As of today, the module will block all button presses while the actuator is moving, to make sure we don't do
something .. nefarious :).

## Update Sun 3 May 2024

In the meantime, this is now simulated by knowing what button is enabled and substracting the button selected.
From there, we get a positive or negative value, and we use that to simulate the move of the actuator.

## Update Sun 4 May 2024

Latest code now stores the button (mode) selected after the actuator have finished moving in the flash memory
that's available in the Pico. There's only 2MB flash, but I only need one byte :D. There might be more that
I can store there as well in the future.

This flash value is then read on bootup and the correct (latest, before reboot/shutdown/reset) button/mode
is then selected automatically.

## Update Sun 5 May 2024

* Add the bare-bones of CAN-bus read and write. Doesn't actually *do* anything yet (since I don't have a CAN-bus
  adapter :), it just logs debug output on what it *would* do.
* Implement bare-bones actuator test by "moving" (i.e. blink LEDs :) the actuator 1mm backward then forward 1mm.
* Implement checking valet mode. This is now stored in the flash.

## Update Sun 6 May 2024

* Rewrite the flash code to be "smarter". Actually, easier to use :).

## Update Sun 8 May 2024

* Update the circuit diagram and PCB with a home-made CAN bus adapter, because I can't use the over-the-shelf
  ones.
* Remove the EIS/SteeringLock "relay". Can't really cut the power to EIS that way. If there's no power to it,
  it won't detect the key, and won't allow me to turn the power to the device on! :).

## Update Sun 9 May 2024

* Update the connectors, get proper CAD drawings and 3D models for them.
* Change the DEBUG connector to a vertical, 5pin, JST connector.
* Change all smaller motherboard connectors with one big one. This should then go to ONE big panel/chassis
  connector, which can then be split up into multiple leads to the different parts of the car.

## Update Thu 27 Jun 2024

* Need to protect the 5V rail from spikes from the actuator control relay, so add
  a big, fat electrolytic capacitor.
* Because of the new capacitor is HUGE (!!), better rearrange all the connectors to
  the BOTTOM layer.
* Move the two CAN-bus ICs from the BOTTOM to the TOP layer, to make room for the
  connectors on the BOTTOM layer.

These changes will allow us to use a different case, where we can put the RPi/Pico,
the 5V converter and the CAN-bus controllers on one side, and mount it as low in
the case as possible. IF we get an aluminium case, we can even use that as a heat
shield for the RPi/Pico CPU, but most importantly, for the 5V converter, which if
it draws "a lot" of power, will get hot.

## Update Sun 30 Jun 2024

* Rewrite and get the actuator functionality working. Well, mostly anyway. I might have a
  hardware problem, the Pico just "suddenly" reboots for no apparent reason (no crash etc).
  MIGHT be a feedback from the actuator, or a dip in voltage/amps when it's moving that the
  power supply can't quite handle (I DID get a very cheap one!), so the Pico doesn't get
  enough power..

## Update Mon 1 Jul 2024

* Change the motherboard connector. AGAIN!! :D
  The huge connector I've been looking at never *really* sat right with me. But found a fairly
  nice chassis connector, which means I can use a ribbon cable connector for the motherboard.
  Saves A LOT (!!) of space.

# Additional information

* [RND-ASH Mercedes hacking docs](https://github.com/rnd-ash/mercedes-hacking-docs)
* [RND-ASH deciphering of CAN bus messages](https://github.com/rnd-ash/MBUX-Port/blob/master/203_b.txt)
* [RND-ASH Open Vehicle Diagnostic project](https://github.com/rnd-ash/OpenVehicleDiag)
* [Konstantin Weitz home-made roof opening device](https://github.com/konne88/slk)
* [A Audi TT project to read speed and write to the IC via CAN](https://www.hackster.io/databus100/digital-speedometer-to-car-s-instrument-cluster-via-can-bus-66e273)
