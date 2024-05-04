# Introduction

This is to introduce drive-by-wire buttons for Mercedes-Benz. Specifically, it is for my Mercedes-Benz SLK55 AMG.

![drive-by-wire buttons](./AstonMartin%20Drive%20Buttons.jpg)

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

> At any point, if there is a fail, sett the RED LED blinking and stop further program execution.

1. Bootup process.
     1. Light status LED (RED).												-> BOOTUP STARTED
     2. Initiate CAN bus connection.											Q: How to test connection?
         - Send message to IC: "Starting Drive-By-Wire system".
     3. Initiate fingerprint scanner connection.
         1. Send message to IC: "Initializing Fingerprint Scanner".
         2. Login to and unlock the fingerprint scanner.						CodeFunction: `VfyPwd`
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

2. Use authorization.
     1. Send message to IC: "Authorizing use".
     2. Check valet mode.
         1. If false:
             - Light fingerprint scanner LED (BLUE).						CodeFunction: `AuraLedConfig`.
             - LOOP: Wait for fingerprint.
                 - Check if fingerprint is in library.
                 - If false:
                     - Light fingerprint scanner LED (RED/FLASH).					CodeFunction: `AuraLedConfig`.
                     - If attempts >= 3: sleep for 5min.
                     - else: restart loop.
         2. else:
             - Light fingerprint scanner LED (BLUE/GRADUALLY OFF).			CodeFunction: `AuraLedConfig`.
         3. else if: we have four-colour LEDs:
             - Light status LED (BLUE? WHITE?)
     3. Close EIS relay #1 (ignition switch).								Q: What if power loss??
     4. Close EIS relay #2 (steering lock).									Q: What if power loss??
     5. Send message to IC: "Use authorized, welcome <user|valet>".
     6. Light status LED (GREEN).											-> LOGIN DONE + MAIN LOOP STARTED
     7. Send "start car" voltage signal to SAM.								Q: How do we do that? Three level relay?

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
   set in valet mode all at the same time?
Q: Can DriveByWire check CAN for certain buttons around the car
   to be pressed in sequence just like GhostImmobiliser??

# Pin layout for RaspberryPI 3-5, Raspberry Pi Pico

| Pin | Port    | Use                          |-| Pin | Port    | Use
| --: | :------ | :--------------------------- |-| --: | :------ | :------------------------------- |
|   1 | GPIO  0 | Debug (RX)                   |-| 40  | VBUS    |                                  |
|   2 | GPIO  1 | Debug (TX)                   |-| 39  | VSYS    |                                  |
|   3 | GND     |                              |-| 38  | GND     |                                  |
|   4 | GPIO  2 | Button (Switch - P)          |-| 37  | 3V3_EN  |                                  |
|   5 | GPIO  3 | Button (Switch - R)          |-| 36  | 3V3_OUT |                                  |
|   6 | GPIO  4 | Button (Switch - N)          |-| 35  | VDC_REF |                                  |
|   7 | GPIO  5 | Button (Switch - D)          |-| 34  | GPIO 28 | Actuator - Motor Relay (+)       |
|   8 | GND     |                              |-| 33  | GND     |                                  |
|   9 | GPIO  6 | Button (Telltale - P)        |-| 32  | GPIO 27 | Actuator - Motor Relay (-)       |
|  10 | GPIO  7 | Button (Telltale - R)        |-| 31  | GPIO 26 | Actuator - Potentiometer Brush   |
|  11 | GPIO  8 | Button (Telltale - N)        |-| 30  | RUN     |                                  |
|  12 | GPIO  9 | Button (Telltale - D)        |-| 29  | GPIO 22 | EIS Relay (#3 - start)           |
|  13 | GND     | [GPIO 29]                    |-| 28  | GND     | [GPIO 23]                        |
|  14 | GPIO 10 | CAN #1 (RX)                  |-| 27  | GPIO 21 | CAN #0 (RX)                      |
|  15 | GPIO 11 | CAN #1 (TX)                  |-| 26  | GPIO 20 | CAN #0 (TX)                      |
|  16 | GPIO 12 |                              |-| 25  | GPIO 19 | EIS Relay (#1 - ignition switch) |
|  17 | GPIO 13 | Fingerprint Scanner (WAKEUP) |-| 24  | GPIO 18 | EIS Relay (#2 - steering lock)   |
|  18 | GND     | [GPIO 25]                    |-| 23  | GND     | [GPIO 24]                        |
|  19 | GPIO 14 | Status LED (Data OUT)        |-| 22  | GPIO 17 | Fingerprint Scanner (TX)         |
|  20 | GPIO 15 | Status LED (Data IN)         |-| 21  | GPIO 16 | Fingerprint Scanner (RX)         |

LED | GPIO 25

[Olimex RP2040-PICO30-16](https://thepihut.com/products/olimex-rp2040-pico30-16) also exposes GPIO 23-25, 29 by sacrificing four GROUND pins.

## External contacts

### Buttons and their LEDs

* 4x Buttons (Switch)
* 4x Button LEDs (Telltale)
* 1x GND
=> [ 9 pin](https://www.ebay.co.uk/itm/325261653847)

### Status LED

* 2x Status LED (Data IN+OUT)
* 1x 3V3
=> [3 pin](https://www.ebay.co.uk/itm/174775342997).

Or those two combined, [buttons and their LEDs and the status LED](https://www.ebay.co.uk/itm/174775342997).

### Fingerprint scanner

* 1x WAKEUP
* 2x Data (TX+RX)
* 1x GND
* 1x 3V3 (power)
* 1x 3V3 (touch induction power)
=> [6 pin](https://www.ebay.co.uk/itm/174775342997)

### EIS Relay

* 1x Control #1 (ignition switch)
* 1x Control #2 (steering lock)
* 1x Control #3 (start signal)
* 1x 5V
* 1x GND
=> [5 pin](https://www.ebay.co.uk/itm/174775342997)

### Actuator

* 1x Actuator (Potentiometer Brush)
* 1x Actuator (Motor Relay +)
* 1x 5V
* 1x GND
=> [4 pin](https://www.ebay.co.uk/itm/174775342997)

### CAN bus #0 and #1

* 2x CAN-L
* 2x CAN-H
=> [4 pin](https://www.ebay.co.uk/itm/174775342997)

### Total

31 pins out from system.

# Parts

## Actuation

| Part | Price |
| :--- | :---  |
| [Actuator w/ feedback (potentiometer)](https://www.progressiveautomations.com/products/linear-actuator-with-potentiometer?variant=18277322424387) | £115 ($145) |
| [Aston Martin drive select buttons](https://www.ebay.co.uk/sch/i.html?_dkr=1&iconV2Request=true&_blrs=recall_filtering&_ssn=hillsmotors&store_name=hillsmotors&_oac=1&_nkw=gear%20select%20switch) | £35	(*4 = £140) |
| [GX20 Aviation Plug  4-pin Male+Female Panel Mount](https://www.ebay.co.uk/itm/174775342997?var=473951323019) | £4 |
| [GX20 Aviation Plug  5-pin Male+Female Panel Mount](https://www.ebay.co.uk/itm/174775342997?var=473951323020) | £4 |
| [GX20 Aviation Plug  6-pin Male+Female Panel Mount](https://www.ebay.co.uk/itm/174775342997?var=473951323021) | £4 |
| [GX20 Aviation Plug 12-pin Male+Female Panel Mount](https://www.ebay.co.uk/itm/174775342997?var=473951323026) | £5 |

## Controller

| Part | Price |
| :--- | :---  |
| [Raspberry Pi Pico (w/ headers)](https://thepihut.com/products/raspberry-pi-pico?variant=41925332566211) | £5 |
| [DC-DC Buck Converter 7-24V to 5V 4A](https://thepihut.com/products/dc-dc-buck-converter-7-24v-to-5v-4a?variant=39865627607235) | £5 |
| [Fingerprint scanner (panel mount)](https://thepihut.com/products/panel-mount-fingerprint-sensor-with-bi-colour-led-ring-r503?variant=41727311675587) | £23 |
| [MOSFET Power Controller](https://thepihut.com/products/gravity-mosfet-power-controller) | £4 (*3 = £12) |
| [1Channel CAN bus extension board](https://thepihut.com/products/can-board-sn65hvd230?variant=40242101256387) | £6 (*2 = £12) |
| [NeoPixel Diffused 5mm LED](https://thepihut.com/products/neopixel-diffused-5mm-through-hole-led-5-pack?variant=27739696529) | £5 |
| [LED holder 5mm](https://thepihut.com/products/5mm-plastic-flat-led-holder-pack-of-5?variant=27739774353) | £1 |

## For development

| Part | Price |
| :--- | :---  |
| [Raspberry Pi Debug Probe](https://thepihut.com/products/raspberry-pi-debug-probe?variant=42380171870403) | £12 |
| [120-Piece Ultimate Jumper Bumper Pack](https://thepihut.com/products/thepihuts-jumper-bumper-pack-120pcs-dupont-wire?variant=13530244284478) | £6 |
| [575-Piece Ultimate Resistor Kit](https://thepihut.com/products/ultimate-resistor-kit?variant=36476117073) | £6 |
| [Half-Size Breadboard](https://thepihut.com/products/breadboard-400-point-clear?variant=31986026381374) | £3 (*3 => £9) |
| [Breadboard for Pico](https://thepihut.com/products/breadboard-for-pico?variant=39819276386499) | £4 |
| [Short Plug Headers](https://thepihut.com/products/short-plug-headers-for-raspberry-pi-pico-2-x-20-pin-male?variant=42182974505155) | £1 |
| [10K potentiometer](https://thepihut.com/products/panel-mount-10k-potentiometer-breadboard-friendly?variant=27740444817) | £1 |
| [Tactile Switch Buttons](https://thepihut.com/products/tactile-switch-buttons-6mm-tall-x-10-pack?variant=27739414097) | £3 |
| [Breakout for 6-pin JST SH-Style Connector - Side Entry](https://thepihut.com/products/breakout-for-6-pin-jst-sh-style-connector-side-entry?variant=42438253871299) | £1 |
| [Extra-long break-away 0.1" 16-pin strip male header (5 pieces)](https://thepihut.com/products/extra-long-break-away-0-1-16-pin-strip-male-header-5-pieces?variant=27740420881) | £3 |
| [220V Power Supply Adapter (12V/10A)](https://www.ebay.co.uk/itm/234147120198?var=533767190848) | £21 |
| [DB9 Breakout Board PCB – Male](https://thepihut.com/products/db9-breakout-board-pcb-male?variant=41727856148675) | £2 |

## Other

These aren't things needed, but maybe I'll have a need for them one day..

| Part | Price |
| :--- | :---  |
| [1 Channel Relay for RPi](https://thepihut.com/products/grove-relay?variant=40341004746947) | £3 |
| [2 Channel Relay Breakout](https://thepihut.com/products/2-channel-relay-breakout-12v) | £8 |
| [2 Channel Isolated Relay Breakout](https://thepihut.com/products/2-channel-isolated-relay-breakout-12v) | £12 |
| [4 Channel Relay Breakout](https://thepihut.com/products/4-channel-relay-breakout-12v) | £16 |
| [2 Channel Latching Relay](https://thepihut.com/products/grove-2-coil-latching-relay) | £7 |
| [9A/28V SPDT MOSFET Switch](https://thepihut.com/products/moswitch-9a-28v-spdt-mosfet-switch) | £5 |
| [DC-DC Buck-Mode Power Module (8~28V to 5V 1.6A)](https://thepihut.com/products/dc-dc-buck-mode-power-module-8-28v-to-5v-1-6a) | £3 |
| [DC-DC Buck-Mode Power Module (5.5~28V to 3.3V 2.4A)](https://thepihut.com/products/dc-dc-buck-mode-power-module-5-5-28v-to-3-3v-2-4a) | £3 |
| [5V Buck Converter Unit (ME3116AM6G)](https://thepihut.com/products/5v-buck-converter-unit-me3116am6g) | £4 |
| [High Precision Capacitive Fingerprint Reader](https://thepihut.com/products/high-precision-capacitive-fingerprint-reader-b) | £61 |
| [I2C GPIO Expander](https://thepihut.com/products/adafruit-pcf8574-i2c-gpio-expander-breakout-stemma-qt-qwiic) | £5 |
| [MRK CAN Shield Arduino](https://www.pcbway.com/project/shareproject/MRK_CAN_Shield_Arduino_133f7666.html) | - |

## Small footprint controllers

The Pico is for development. Makes things easier when it's in a bigger format. However, some of these below might be used for
the actual "production" device. But to get all the GPIO needed, an I2C GPIO expander (see above) would be needed.

Some of the signals I need will probably be to fast for the I2C bus, so those would have to come in through the board GPIO,
not the I2C GPIOs.

| Part | Price |
| :--- | :---  |
| [Seeed XIAO RP2040](https://thepihut.com/products/seeed-xiao-rp2040) | £6 |
| [Tiny 2040](https://thepihut.com/products/tiny-2040?variant=41359025897667) | £12 |
| [Waveshare RP2040 Tiny](https://thepihut.com/products/waveshare-rp2040-tiny?variant=42483599507651) | £5 |

# Circuit diagram

This is still work in progress, but this looks about right. That's what I've wired on the breadboard.

![Circuit diagram](./Circuit%20Diagram.png)

## Wiring on bread boards

Those CAN bus adaptors I can't apparently used. They're for a high-speed CAN, but the MB I have have a slow-speed
CAN :(. I'll figure something out..

Also, I'm missing the headers for the fingerprint scanner and the actuator in the upper right breadboard.

![Initial wiring on bread boards](./2024-04-20%2021.50.37.jpg)

### Latest wiring on bread boards

This what it looks like now. I built a box :D :D.

![Latest wiring on bread boards](./2024-05-04%2011.09.21.jpg)

# Source code

I've barely started on this, but I have the bare-bones of it in these repos:

* How to control the LEDs, including the NeoPixel (multi-colour LED): [pico-rust-test_1-LEDS](https://github.com/FransUrbo/pico-rust-test_1-LEDS)
* How to read the buttons and control their LEDs: [pico-rust-test_2-BUTTONS-LEDS](https://github.com/FransUrbo/pico-rust-test_2-BUTTONS-LEDS)
  Yes, those are the genuine Aston Martin drive buttons! :).
* How to read and write to the fingerprint scanner: [pico-rust-test_3-FP_SCANNER](https://github.com/FransUrbo/pico-rust-test_3-FP_SCANNER)
* How to control the three MOSFET "relays": [pico-rust-test_4-MOSFET_RELAYS](https://github.com/FransUrbo/pico-rust-test_4-MOSFET_RELAYS)
* How to setup and trigger the built-in watchdog on the RPi: [pico-rust-test_6-WATCHDOG-LED](https://github.com/FransUrbo/pico-rust-test_6-WATCHDOG-LED)

The actual DriveByWire source code is getting underway, it's in [the code directory](./code).
I verify fingerprint, read buttons, turn on LEDs correctly and I simulate actuator movement with two
LEDs - RED and GREEN.

This was a [recording of the screen](https://www.dropbox.com/scl/fi/bi3qf4g1nu1k6bnatyuem/Screen-Recording-2024-05-03-at-20.10.48.mov?rlkey=vi5vw7pl20p2h9n0wq28tuy4a&st=hylgbs2c&dl=0) while it was running.
It's a day old, and I've done some modifications to the code since them, but this demostrates it fairly
well.

And this is what it [looked like in action](https://www.dropbox.com/scl/fi/nsdj958atposke2wdfzk9/2024-05-03-20.12.23.mov?rlkey=e7vu1sx3g0xffbefloeaspzul&st=kw81bt22&dl=0).
They where to big for GitHub, so had to put them on my Dropbox account.

The yellow LED in the upper left corner of the box, that's connected to the MOSFET "relay", is "start the car".
I *THINK* it is enough to send +12V to the ECU on the "start position" pin. We'll see, have to do some
experimentation. In this car, I only have to trigger it for the ECU to take over and start the car. So
I'm only turning on the MOSFET "relay" for a second. Should be more than enough.

The steady orange (well, it's not very orange, is it!?? :) and then green LED in the top middle is orange =>
"starting up" (the module) and when it turns green, it means "all is well". Had the fingerprint not matched,
it and the aura around the fingerprint scanner turned red.

The two blinking LEDs, the red and green to the right of the multi-colour status LED, is the simulation of
moving the actuator. Green is "move forward" and red is "move backwards". It blinks five times in either
direction, because that's what I told it to :).

Eventually, when I get the actuator, I'll be able to read the position of the actuator on GPIO26, which is
set as an Input. Then I can from there calculate how much to move the actuator in either direction to select
the desired gear.

In the meantime, this is now simulated by knowing what button is enabled and substracting the button selected.
From there, we get a positive or negative value, and we use that to simulate the move of the actuator.

If I press the same, already selected, drive button, its LED will just blink three times and not do anything
other than that.

It will block all button presses while the actuator is moving, to make sure we don't do something ..
bad.

# Additional information

* [RND-ASH Mercedes hacking docs](https://github.com/rnd-ash/mercedes-hacking-docs)
* [RND-ASH deciphering of CAN bus messages](https://github.com/rnd-ash/MBUX-Port/blob/master/203_b.txt)
* [RND-ASH Open Vehicle Diagnostic project](https://github.com/rnd-ash/OpenVehicleDiag)
* [Konstantin Weitz home-made roof opening device](https://github.com/konne88/slk)
