console.log("hello");
//return;
console.log("world");

//save({"testtest": 100});
//console.log(savedata);

const toMono = (audio) => {
  return audio.reduce((ch1, ch2, ch) => (
    ch === 0 ?
      ch1 :
      ch1.map((val, j) => (0.5 * (val + ch2[j])))
  ));
};
const fromMono = (audio, mono) => {
  audio.map((ch) => {
    ch.map((_, i) => {
      ch[i] = mono[i];
    });
  });
};
const map = (mono, callback) => {
  mono.map((_, i) => {
    mono[i] = callback(mono[i], i);
  });
};

const Buffer = class {
  constructor(length) {
    this.length = length;
    this.buffer = [];
  }
  add(stereo) {
    for (let i = 0; i < stereo.length; i++) {
      if (this.buffer.length < i + 1) {
        this.buffer.push([...Array(this.length)].map(() => (0)));
      }
      this.buffer[i] = this.buffer[i].concat([...stereo[i]]).slice(-this.length);
    }
  }
  get() {
    return this.buffer;
  }
};
let buffer = new Buffer(4096);

const rect = (ctx, x, y, w, h, fill, stroke) => {
  ctx.addPolygon([[x, y], [x+w, y], [x+w, y+h], [x, y+h]], {fill, stroke, strokeClosed: true });
};

const show = (mono, x, y, w, h, scale) => {
  const shape = [...Array(w >>> 0)].map((_, i) => {
    const mi = Math.floor(mono.length * i / w);
    const my = (scale * mono[mi] * 0.5 + 0.5) * h;
    return [x + i, y + my];
  });
  return shape;
};

const show2 = (ctx, mono, x, y, w, h, scale) => {
  const shape = show(mono, x, y, w, h, scale);
  ctx.addPolygon(shape, {stroke: 0xFFFFFFFF});
  rect(ctx, x, y, w, h, undefined, 0xFFFFFFFF);
};

ps88.audio((ctx) => {
  buffer.add(ctx.audio);
  const mono = toMono(ctx.audio);
  map(mono, (_, i) => {
    return Math.sin(440 * 2 * Math.PI * (ctx.posSamples + i) / ctx.sampleRate) * 0.1;
  });
  fromMono(ctx.audio, mono);
});

ps88.gui((ctx) => {
  rect(ctx, 0, 0, ctx.w, ctx.h, 0xAA4488FF);

  buffer.get().map((buf, i) => {
    show2(ctx, buf, 180, 120 + 120 * i, 360, 120, 5);
  });

  ctx.addText("Hello, World!", ctx.mouse.x, ctx.mouse.y, {
    size: 16,
    color: ctx.mouse.pressedL ? 0xFF44AAFF : 0xFFFFFFFF,
  });
});

//setProcessor(audio, gui);

// TODO
// - save() / load() の実装
