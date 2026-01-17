#!/bin/sh

# This is the device for my Debug Probe on my MacOS host.
export DEV="$(echo /dev/cu.usbmodem*)"

[ ! -d "logs" ] && mkdir logs

# On MacOS, the `stty` needs to come *after* the port is opened!
(cat < "${DEV}" & /bin/stty -f "${DEV}" speed 115200 -crtscts -mdmbuf) | \
    defmt-print -e ./target.elf --verbose --show-skipped-frames stdin | \
    tee -p logs/run.log-raw | \
    grep --line-buffered -v '^└─ ' | \
    tee -p logs/run.log-filtered | \
    grep --line-buffered ' INFO ' | \
    tee -p logs/run.log-info
