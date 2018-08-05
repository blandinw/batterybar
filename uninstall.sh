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

[ -f "$INSTALLED_PLIST" ] && launchctl unload "$INSTALLED_PLIST"
rm -rf "$INSTALL_DIR"
