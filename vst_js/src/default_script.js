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
 * @param {Uint8Array} ctx.midi - MIDI 入出力
 *    1 イベントあたり 7 byte で、以下のような構造になっている。
 *      [ event1(7 byte), event2(7 byte), event3(7 byte), ... ]
 *    event の構造:
 *      0-3 byte: イベントが発生した時刻 (単位は input のインデックス番号)
 *        4 byte: 上位 4 bit: イベントの種類 (0x9: Note On, 0x8: Note Off)
 *                下位 4 bit: チャンネル番号 (0-15)
 *        5 byte: ノート番号 (0-127)
 *        6 byte: ベロシティ (1-127)
 *    また、以下のように上書きすることで MIDI 出力を書き換えることも可能。
 *      ctx.midi = new Uint8Array([0, 0, 0, 0, 0x90, 69, 127])
 */
const audio = (ctx) => {
  const half = ctx.audio.length / 2;
  for (let index = 0; index < half; index++) {
    let val = 0.0;
    val = Math.sin(count / 44100 * 2 * Math.PI * 440) * 0.02;
    //val = sawtooth(count / 44100 * 2 * Math.PI * 440) * 0.02;
    //val = triangle(count / 44100 * 2 * Math.PI * 440) * 0.02;
    //val = square(count / 44100 * 2 * Math.PI * 440) * 0.02;
    ctx.audio[index] = val;
    ctx.audio[index+half] = val;
    count += 1;
  }
  return 100;
};

const gui = () => {};
