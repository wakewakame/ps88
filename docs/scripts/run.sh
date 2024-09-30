#!/bin/sh -eu
cd "$(git rev-parse --show-toplevel)"
xtask bundle ps88 --release
./target/bundled/ps88.app/Contents/MacOS/ps88 --midi-input 'microKEY2-25 Air Bluetooth'
