"use strict";
const keyboard = new Map();
const preview = new Float32Array(1024);

ps88.audio((ctx) => {
  for (let event of ctx.midi) {
    if (event.type === "NoteOn") {
      keyboard.set(event.note, { time: 0, ...event });
    }
    if (event.type === "NoteOff") {
      keyboard.delete(event.note);
    }
  }

  const length = ctx.audio[0]?.length ?? 0;
  preview.copyWithin(0, length);
  const previewOffset = preview.length - length;
  for (let i = 0; i < length; i++) {
    let wave = 0;
    for (let key of keyboard.values()) {
      if (i < key.timing) { continue; }
      const freq = 440 * Math.pow(2, (key.note - 69) / 12);
      wave +=
        0.4 * Math.sin(key.time * freq * 2 * Math.PI) *
        Math.pow(0.1, key.time);
      key.time += 1 / ctx.sampleRate;
    }
    preview[previewOffset + i] = 0;
    for (let ch of ctx.audio) {
      preview[previewOffset + i] += (ch[i] + wave) / ctx.audio.length;
      ch[i] = wave;
    }
  }
});

ps88.gui((ctx) => {
  const path = [];
  for (let x = 0; x <= ctx.w; x++) {
    const i = Math.floor((preview.length - 1) * x / ctx.w);
    const wave = preview[i];
    const y = (wave * 0.5 + 0.5) * ctx.h;
    path.push([x, y]);
  }
  ctx.addPolygon(path, {stroke: 0xFFFFFFFF, strokeWidth: 1});
});
