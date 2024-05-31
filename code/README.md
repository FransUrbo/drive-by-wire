# Build code

## Profile: dev (default)

``` shell
cargo build 2>&1 | tee build.log
```

## Profile: release

``` shell
cargo build --release 2>&1 | tee build.log
```

# Run main app

## Debug mode

``` shell
cargo run --bin drive-by-wire  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```

## Load built code

``` shell
picoprobe -c 'program target.elf verify reset exit'
```

# Valet mode

## Set valet mode

``` shell
cargo run --bin set-valet-mode  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```

## Unset valet mode

``` shell
cargo run --bin unset-valet-mode  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```

# Read config

``` shell
cargo run --bin read_config  2>&1 | unbuffer -p grep -v '^└─' | unbuffer -p grep '^[0-9]' | tee /tmp/debug
```
