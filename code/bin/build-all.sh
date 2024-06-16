#!/bin/sh

for profile in dev release release-dev; do
    cargo build --profile "${profile}"
done
