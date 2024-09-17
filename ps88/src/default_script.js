"use strict";
console.log("hello world");
let count = 0;
const sawtooth = (rad) => {
  let x = rad / (2 * Math.PI);
  return 2 * (x - Math.floor(x)) - 1;
};
const triangle = (rad) => {
  return sawtooth(2 * rad) * (Math.floor(rad / (2 * Math.PI)) % 2 === 0 ? 1 : -1);
};
const square = (rad) => {
  return rad % (2 * Math.PI) < Math.PI ? 1 : -1;
};

/**
 * オーディオ処理
 *
 * @param {Object} ctx
 * @param {Float32Array} ctx.audio - オーディオ入出力
 *    配列は既に確保されているため、各要素の値を変更するだけでよい。
 *    配列は初期値としてマイク等からの入力信号が入っている。
 *    ctx.ch=2 の場合、信号 は [ L, L, L, ..., R, R, R, ... ] のように並んでいる。
 * @param {number} ctx.ch - ctx.audio のチャンネル数。
 * @param {number} ctx.sampling_rate - ctx.audio のサンプリングレート。
 * @param {Uint8Array} ctx.midi - MIDI 入力
 *    1 イベントあたり 7 byte で、以下のような構造になっている。
 *      [ event1(7 byte), event2(7 byte), event3(7 byte), ... ]
 *    event の構造:
 *      0-3 byte: イベントが発生した時刻 (単位は input のインデックス番号)
 *        4 byte: 上位 4 bit: イベントの種類 (0x9: Note On, 0x8: Note Off)
 *                下位 4 bit: チャンネル番号 (0-15)
 *        5 byte: ノート番号 (0-127)
 *        6 byte: ベロシティ (1-127)
 */
const keys = new Map();
const audio = (ctx) => {
  const half = ctx.audio.length / ctx.ch;
  if (ctx.midi.length > 0) {
    for (let i = 0; i < ctx.midi.length; i += 7) {
      const time = (ctx.midi[i] << 24) | (ctx.midi[i + 1] << 16) | (ctx.midi[i + 2] << 8) | ctx.midi[i + 3];
      const type = ctx.midi[i + 4] >> 4;
      //const channel = ctx.midi[i + 4] & 0x0f;
      const note = ctx.midi[i + 5];
      const velocity = ctx.midi[i + 6];
      if (type === 0x9) {
        keys.set(note, [time, velocity, 0]);
        keys.set(note + 4, [time, velocity, 0]);
        keys.set(note + 5, [time, velocity, 0]);
        keys.set(note + 9, [time, velocity, 0]);
      } else if (type === 0x8) {
        //keys.delete(note);
      }
    }
  }
  for (let index = 0; index < half; index++) {
    let val = 0.0;
    for (const [note, value] of keys) {
      const [time, velocity, count] = value;
      if (time > index) {
        continue;
      }
      const v = (velocity / 127.0) * Math.exp(-5 * count / ctx.sampling_rate);
      if (v < 0.001) {
        //keys.delete(note);
        continue;
      }
      const freq = 440 * Math.pow(2, (note - 69) / 12);
      val += Math.sin(count / ctx.sampling_rate * 2 * Math.PI * freq) * v;
      //val += sawtooth(count / ctx.sampling_rate * 2 * Math.PI * freq) * v;
      //val += triangle(count / ctx.sampling_rate * 2 * Math.PI * freq) * v;
      //val += square(count / ctx.sampling_rate * 2 * Math.PI * freq) * v;
      value[2]++;
    }
    val *= 0.8;
    ctx.audio[index] = val;
    ctx.audio[index+half] = val;
  }
  return 100;
};

const gui = () => {};
