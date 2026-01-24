#!/bin/sh

if [ -z "${1}" ]; then
    echo "Usage: $(basename "${0}") <binary>"
    echo "Binaries available:"
    grep ^name Cargo.toml | \
	sort | \
	uniq | \
	sed -e "s@.*\"\(.*\)\"@\1@" | \
	while read bin; do
	    echo "  ${bin}"
	done
    exit 1
elif ! grep -q "^name = \"${1}\"" Cargo.toml; then
    echo "ERROR: No such binary"
    exit 1
else
    BIN="${1}"
fi

cargo run --bin "${BIN}" | \
    tee -p logs/run.log-raw | \
    grep --line-buffered -v '^└─ '
    tee -p logs/run.log-filtered
