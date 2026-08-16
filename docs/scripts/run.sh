#!/bin/sh -eu
cd "$(git rev-parse --show-toplevel)"
cargo nice-plug bundle ps88 --release

./target/bundled/ps88.app/Contents/MacOS/ps88
#./target/bundled/ps88.app/Contents/MacOS/ps88 --input-device '外部マイク'
#./target/bundled/ps88.app/Contents/MacOS/ps88 --midi-input 'MPK mini Play mk3'
