#!/bin/sh -eu
cd "$(git rev-parse --show-toplevel)"
xtask bundle ps88 --release

# TODO: standalone で動かすときに --input-device / --midi-input / -r が自動でいい感じに設定されるようにしたい
./target/bundled/ps88.app/Contents/MacOS/ps88 --input-device '外部マイク' -r 44100 -p 512
#./target/bundled/ps88.app/Contents/MacOS/ps88 --midi-input 'microKEY2-25 Air Bluetooth' -r 44100 -p 512
