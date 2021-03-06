#!/usr/bin/env bash

set -ex

usage() {
    echo "$(basename "$0") <path-to-binary> <path-to-plist>"
    exit 1
}

if [ $# -lt 2 ]; then
    usage
fi

BIN="$1"
shift
PLIST="$1"
shift
INSTALLED_PLIST="$HOME/Library/LaunchAgents/$(basename "$PLIST")"
INSTALL_DIR="$HOME/Library/Application Support/${PLIST/.plist/}"

mkdir -p "$INSTALL_DIR"

cp "$BIN" "$INSTALL_DIR"
[ -f "$INSTALLED_PLIST" ] && launchctl unload "$INSTALLED_PLIST"
< "$PLIST" > "$INSTALLED_PLIST" sed -e "s,BIN,$INSTALL_DIR/$(basename "$BIN"),"
launchctl load "$INSTALLED_PLIST"