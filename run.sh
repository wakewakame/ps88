#!/bin/sh -eu
cargo xtask bundle vst_js --release
./target/bundled/vst_js.app/Contents/MacOS/vst_js
