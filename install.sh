#!/usr/bin/env bash

set -ex

BIN="$PWD/target/release/batterybar"
PLIST="$HOME/Library/LaunchAgents/com.github.blandinw.batterybar.plist"

cargo build --release
strip "$BIN"
cp "$BIN" /usr/local/bin
launchctl unload "$PLIST"
cp com.github.blandinw.batterybar.plist "$PLIST"
launchctl load "$PLIST"