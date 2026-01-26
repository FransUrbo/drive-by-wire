#!/bin/sh

if [ -z "${1}" ]; then
    echo "Usage: $(basename "${0}") <binary>"
    echo "Binaries available:"
    grep ^name Cargo.toml | \
	sort | \
	uniq | \
	sed -e "s@.*\"\(.*\)\"@\1@" | \
	while read bin; do
	    # Skip binaries which uses `defmt_serial`. They can't run via Cargo!
	    grep -q 'defmt_serial' "src/${bin}.rs" || echo "  ${bin}"
	done
    exit 1
elif ! grep -q "^name = \"${1}\"" Cargo.toml; then
    echo "ERROR: No such binary"
    exit 1
else
    BIN="${1}"
fi

# Try to figure out log level from already built binary.
# I like to double check with `cargo build` before I run it :).
if [ -d "target" ]; then
    b="$(echo "${BIN}" | sed 's@-@_@g')" # Cargo replaces all dashes in bin names with slashes.
    files="$(find target/thumbv6m-none-eabi/debug/deps/ -type f -name "${b}*.d")"

    unset ENVS
    for file in ${files}; do
	if [ -n "${ENVS}" ]; then
	    ENVS="${ENVS}
$(grep 'env-dep:' "${file}" | sed 's@.*:@@')"
	else
	    ENVS="$(grep 'env-dep:' "${file}" | sed 's@.*:@@')"
	fi
    done
fi

eval "${ENVS}" # Safe even if it's unset.
cargo run --bin "${BIN}" | \
    tee -p logs/run.log-raw | \
    grep --line-buffered -v '^└─ '
    tee -p logs/run.log-filtered
