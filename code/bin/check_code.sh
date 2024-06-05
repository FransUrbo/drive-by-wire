#!/bin/sh

set +e

/bin/echo -n "Checking code linting: "
cargo clippy > _check-package.log 2>&1
[ "${?}" -eq 0 ] && echo OK || echo FAIL

/bin/echo -n "Checking formating: "
cargo fmt --check > _check-fmt.log 2>&1
[ "${?}" -eq 0 ] && echo OK || echo FAIL

/bin/echo -n "Checking project: "
cargo verify-project > _check-project.log 2>&1
[ "${?}" -eq 0 ] && echo OK || echo FAIL
