#!/usr/bin/env bash

set -ex

BIN="$PWD/target/release/batterybar"
cargo build --release
strip "$BIN"
echo "$BIN"
