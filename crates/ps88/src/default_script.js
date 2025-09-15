"use strict";
console.log("hello world");
let sample = 0;
ps88.audio((ctx) => {
  // 正弦波を鳴らす
  for (let ch of ctx.audio) {
    for (let i = 0; i < ch.length; i++) {
      ch[i] = 0.01 * Math.sin(((i + sample) / ctx.sampleRate) * 440 * 2 * Math.PI);
    }
  }
  if (ctx.audio.length >= 1) {
    sample += ctx.audio[0].length;
  }
});

// TODO
// - gui() の実装
// - save() / load() の実装
