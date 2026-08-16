# PS88

[English](README.md) | [日本語](README.ja.md)

A synthesizer that lets you describe waveforms in JavaScript.
It runs in both the browser and as a VST3 plugin.

<p align="center">
  <img src="./docs/logo/logo.svg" width="160">
</p>

- Try it in the browser: https://wakewakame.github.io/ps88web
- Download VST3: TODO

# Usage

To play a 440 Hz sine wave, write code like the example below.

```js
// Elapsed time in seconds
let time = 0;

// Register the audio callback function
ps88.audio((ctx) => {
  // Length of the output waveform
  const length = ctx.audio[0]?.length ?? 0;

  for (let i = 0; i < length; i++) {
    // Generate a 440 Hz sine wave
    let wave = Math.sin(time * 440 * 2 * Math.PI);

    // Output the same waveform to every channel
    for (let ch of ctx.audio) {
      ch[i] = wave;
    }
    time += 1 / ctx.sampleRate;
  }
});
```

You can also use microphone and MIDI input, and even render GUIs.

- [API docs](https://wakewakame.github.io/ps88web/docs/variables/ps88.html)
- [examples](https://wakewakame.github.io/ps88web/examples/index.html)

# Build PS88

After installing [Rust](https://www.rust-lang.org/tools/install), run the following command:

```sh
git clone https://github.com/wakewakame/ps88.git
git submodule update --init --recursive
cd ps88
cargo install --git https://codeberg.org/RustAudio/nice-plug.git --rev 42f480568d383632bbcb2f29d0e4349b93566432 cargo-nice-plug
cargo nice-plug bundle ps88 --release
```

When complete, the following will be generated in `target/bundled/`:

- `ps88.clap`
- `ps88.vst3`
- `ps88` or `ps88.exe`
