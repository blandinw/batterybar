#!/usr/bin/env bash

set -ex

BIN="$( ./build.sh | tail -n1 )"
./install.sh "$BIN" com.github.blandinw.batterybar.plist