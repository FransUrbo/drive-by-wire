#!/bin/sh

# This is the device for my Debug Probe on my MacOS host.
export DEV="$(echo /dev/cu.usbmodem*)"

# On MacOS, the `stty` needs to come *after* the port is opened!
(cat < "${DEV}" & stty -f "${DEV}" speed 115200 -crtscts -mdmbuf) | \
    defmt-print -e ./target.elf
