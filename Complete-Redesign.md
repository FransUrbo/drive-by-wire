# Introduction

While I was working on the new CAN design, I reliased that the RPIs processors are quite
easy to work with.

Their design only require a few discrete components, so I decided to get rid of the Pico
module, and do it "raw" instead.

As in, use the ICs directly. AND, while I was at it, since the SC18IS606 can handle THREE
SPI devices, I added two more CAN adapters. AND, also upgraded to the newer RP2354B.

None of this exists in the real world yet. As soon as I can get someone to verify the design
and the schema, then I'll have the PCBs made.

Never soldered SMDs before, so I'll need a hot air soldering station as well. But it'll be
fun! :)

The "only" thing I have to do once that's done is to get the I²C to SPI to CAN code working.

# CANs
The MB have three CAN networks:
* CAN B (Interior CAN): Used for interior electronics, comfort modules, and low-speed communication.
  Uses a "weird" 83.3Kbps bitrate.
* CAN C (Engine/Drivetrain CAN): Used for high-speed communication between the engine control unit
  (ECU), transmission (7G-Tronic), ESP, and ABS modules.
* Diagnostic CAN (or a secondary Interior CAN, depending on configuration): While CAN B and C are the
  primary operational buses, the diagnostic system (OBD port) acts as a gateway to these networks,
  allowing communication with the various control modules. 

Not sure if I'll need all three, but the chips are cheap (MCP2515: £2.23; TJA1055: £2.03) so why
not :).

I'm sure I can find a need and use for all three, now that I can actually utilise all 48 (!!) GPIOs
that I get from the RP2354B!

# Diagram
![Circuit Diagram - Chip based](./images/new-can-design/Circuit%20Diagram%20-%20Chip%20based.jpg)

# PCB Views
Yes, that connector is HUUUGE! :). But it's actually smaller, overall, than a whole bunch of smaller
ones. AND, it locks in nice and tight.

![Main - Top](images/new-can-design/Main%20-%20Top.png)
![Main - Side](images/new-can-design/Main%20-%20Side.png)
![Main - Bottom](images/new-can-design/Main%20-%20Bottom.png)
