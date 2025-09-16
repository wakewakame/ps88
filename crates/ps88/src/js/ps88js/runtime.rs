use super::super::core;
use super::convert::*;
use super::global_api::*;
use super::gui_api::*;
use super::status::*;
use deno_core::{serde_v8, v8};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct Runtime<'a> {
    status: Rc<RefCell<Status>>,
    runtime: core::JsRuntime<'a>,
    audio_buf: Option<v8::SharedRef<v8::BackingStore>>,
    logger: Rc<RefCell<Vec<Box<dyn Fn(String) -> bool>>>>,
}

impl Runtime<'_> {
    pub fn new(userdata: Arc<Mutex<Vec<u8>>>) -> core::Result<Self> {
        let status = Rc::new(RefCell::new(Status {
            audio_callback: None,
            gui_callback: None,
            userdata,
        }));
        let api = core::Api::new(
            "ps88",
            Api {
                status: status.clone(),
            },
        )
        .add("audio", Api::audio)
        .add("gui", Api::gui)
        .add("save", Api::save)
        .add("load", Api::load);
        let logger = Rc::new(RefCell::new(Vec::<Box<dyn Fn(String) -> bool>>::new()));
        let logger2 = logger.clone();
        let logger_func = move |msg: String| {
            logger2.replace(
                logger2
                    .replace(vec![])
                    .into_iter()
                    .filter(|logger| logger(msg.clone()))
                    .collect(),
            );
        };
        let runtime = core::JsRuntimeBuilder::new()
            .add_api(api)
            .add_logger(logger_func)
            .build()?;
        Ok(Self {
            status,
            runtime,
            audio_buf: None,
            logger,
        })
    }
    // ログ関数を追加する。ログ関数が false を返すとログの受信が終了する。
    pub fn add_logger(&mut self, logger: Box<dyn Fn(String) -> bool + Sync + Send>) {
        self.logger.borrow_mut().push(logger);
    }
    pub fn compile(&mut self, code: &str) -> core::Result<()> {
        self.reset()?;
        self.runtime.run(code)?;
        Ok(())
    }
    pub fn audio(
        &mut self,
        audio: &mut [&mut [f32]], // audio[ch][sample]
        midi: &mut Vec<[u8; 7]>,
        sample_rate: f64,
        pos_samples: u64,
        bpm: f64,
    ) -> core::Result<()> {
        let result = || -> core::Result<()> {
            let scope = &mut self.runtime.scope();
            let callback = {
                let status = self.status.borrow();
                let Some(callback) = status.audio_callback.as_ref() else {
                    return Ok(()); // callback が登録されていなければ何もしない
                };
                v8::Local::new(scope, callback)
            };

            // audio, midi, sampling_rate を v8 に変換
            let audio_js = audio_to_backing_store(scope, audio, &mut self.audio_buf)?;
            let midi_js = midi_to_arr(scope, midi)?;

            // 引数を用意
            let ctx = Dict::new(scope)
                .add("audio", audio_js)?
                .add("midi", midi_js)?
                .add_number("sampleRate", sample_rate)?
                .add_number("posSamples", pos_samples as f64)?
                .add_number("bpm", bpm)?
                .value();

            // callback 呼び出し
            let this = v8::undefined(scope).into();
            {
                let try_catch = &mut v8::TryCatch::new(scope);
                callback.call(try_catch, this, &[ctx]).ok_or(
                    core::JsRuntimeError::RuntimeError(core::report_exceptions(try_catch)),
                )?;
            }

            // 結果を audio, midi に書き戻す
            backing_store_to_audio(&self.audio_buf, audio);
            *midi = arr_to_midi(scope, midi_js)?;

            Ok(())
        }();
        if result.is_err() {
            // エラーが起きたら状態をリセット
            self.reset()?;
        }
        result
    }
    pub fn gui(&mut self, args: GuiArgs) -> core::Result<Vec<Shape>> {
        let result = || -> core::Result<Vec<Shape>> {
            let scope = &mut self.runtime.scope();
            let callback = {
                let status = self.status.borrow();
                let Some(callback) = status.gui_callback.as_ref() else {
                    return Ok(vec![]); // callback が登録されていなければ何もしない
                };
                v8::Local::new(scope, callback)
            };

            // 引数を用意
            let mouse = Dict::new(scope)
                .add_number("x", args.mouse_x)?
                .add_number("y", args.mouse_y)?
                .add_bool("pressedL", args.pressed_l)?
                .add_bool("pressedR", args.pressed_r)?
                .value();
            let shapes = v8::Array::new(scope, 0);
            let add_polygon = gen_api_add_polygon(scope, shapes)?;
            let add_text = gen_api_add_text(scope, shapes)?;
            let ctx = Dict::new(scope)
                .add_number("w", args.w)?
                .add_number("h", args.h)?
                .add("mouse", mouse)?
                .add("addPolygon", add_polygon.into())?
                .add("addText", add_text.into())?
                .value();

            // callback 呼び出し
            let this = v8::undefined(scope).into();
            {
                let try_catch = &mut v8::TryCatch::new(scope);
                callback.call(try_catch, this, &[ctx]).ok_or(
                    core::JsRuntimeError::RuntimeError(core::report_exceptions(try_catch)),
                )?;
            }

            // 結果を shapes に書き戻す
            serde_v8::from_v8::<Vec<Shape>>(scope, shapes.into())
                .map_err(|e| core::JsRuntimeError::RuntimeError(format!("serde error: {}", e)))
        }();
        if result.is_err() {
            // エラーが起きたら状態をリセット
            self.reset()?;
        }
        result
    }
    fn reset(&mut self) -> core::Result<()> {
        self.runtime.reset()
    }
}

#[derive(Default, Clone)]
pub struct GuiArgs {
    pub w: f64,
    pub h: f64,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub pressed_l: bool,
    pub pressed_r: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, TryRecvError};

    #[test]
    fn test_add_logger() {
        let userdata = Arc::new(Mutex::new(vec![]));
        let mut rt = Runtime::new(userdata).unwrap();

        // ログが受信できる
        let (tx1, rx1) = channel();
        rt.add_logger(Box::new(move |msg: String| {
            tx1.send(msg).unwrap();
            false // 1 回だけ受信して終了
        }));
        let (tx2, rx2) = channel();
        rt.add_logger(Box::new(move |msg: String| {
            tx2.send(msg).unwrap();
            true // 何回でも受信する
        }));
        rt.compile("console.log('1');").unwrap();
        assert_eq!(rx1.try_recv().unwrap(), "1");
        assert_eq!(rx2.try_recv().unwrap(), "1");
        rt.compile("console.log('2');").unwrap();
        assert!(matches!(rx1.try_recv(), Err(TryRecvError::Disconnected)));
        assert_eq!(rx2.try_recv().unwrap(), "2");
        rt.compile("console.log('3');").unwrap();
        assert_eq!(rx2.try_recv().unwrap(), "3");
    }

    #[test]
    fn test_audio() {
        let userdata = Arc::new(Mutex::new(vec![]));
        let mut rt = Runtime::new(userdata).unwrap();

        // 入出力の確認
        rt.compile(
            r#"
                "use strict";
                ps88.audio((ctx) => {
                    if (ctx.sampleRate !== 48000) { throw new Error(`sampleRate: ${ctx.sampleRate}`); }
                    if (ctx.posSamples !== 1024) { throw new Error(`posSamples: ${ctx.posSamples}`); }
                    if (ctx.bpm !== 120) { throw new Error(`bpm: ${ctx.bpm}`); }
                    let audio = JSON.stringify(ctx.audio.map(ch => [...ch]));
                    if (audio !== "[[1,2,3],[4,5,6]]") { throw new Error(`audio: ${audio}`); }
                    let midi = JSON.stringify(ctx.midi);
                    if (midi !== "[[0,1,2,3,4,5,6],[7,8,9,10,11,12,13]]") { throw new Error(`midi: ${midi}`); }
                    for (let ch = 0; ch < ctx.audio.length; ch++) {
                        for (let i = 0; i < ctx.audio[ch].length; i++) {
                            ctx.audio[ch][i] *= 2.0;
                        }
                    }
                    ctx.midi.splice(0, ctx.midi.length, [14, 15, 16, 17, 18, 19, 20]);
                });
            "#,
        )
        .unwrap();
        let mut audio = [&mut [1f32, 2., 3.][..], &mut [4., 5., 6.][..]];
        let mut midi = vec![[0u8, 1, 2, 3, 4, 5, 6], [7, 8, 9, 10, 11, 12, 13]];
        rt.audio(&mut audio[..], &mut midi, 48000.0, 1024, 120.0)
            .unwrap();
        assert_eq!(audio, [[2f32, 4., 6.], [8., 10., 12.]]);
        assert_eq!(midi, vec![[14, 15, 16, 17, 18, 19, 20]]);

        // エラーが起きたら状態がリセット
        // 3 回目の呼び出しでエラーが起きる
        rt.compile(
            r#"
                "use strict";
                let count = 0;
                ps88.audio((ctx) => {
                    ctx.audio[0][0] = count += 1;
                    if (count >= 3) throw new Error("test error");
                });
            "#,
        )
        .unwrap();
        let mut midi = vec![];
        let mut audio = [&mut [0f32][..]];
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();
        assert_eq!(audio, [[1.]]);
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();
        assert_eq!(audio, [[2.]]);
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap_err(); // エラー
        assert_eq!(audio, [[2.]]);
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap(); // 特に何も起きない (関数が登録されていない状態)
        assert_eq!(audio, [[2.]]);

        // 途中で入力の配列長が変化しても対応できる
        rt.compile(
            r#"
                "use strict";
                let count = 0;
                ps88.audio((ctx) => {
                    const expect_size = [[2, 2], [2, 2], [1, 1], [3, 3], [1], [1, 1, 1]];
                    const actual_size = ctx.audio.map(ch => ch.length);
                    if (JSON.stringify(actual_size) !== JSON.stringify(expect_size[count])) {
                        throw new Error(`count: ${count}, actual_size: ${actual_size}`);
                    }
                    count += 1;
                });
            "#,
        )
        .unwrap();
        let mut midi = vec![];
        let mut audio = [&mut [0f32; 2][..], &mut [0.; 2][..]];
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();
        let mut audio = [&mut [0f32; 2][..], &mut [0.; 2][..]];
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();
        let mut audio = [&mut [0f32; 1][..], &mut [0.; 1][..]];
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();
        let mut audio = [&mut [0f32; 3][..], &mut [0.; 3][..]];
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();
        let mut audio = [&mut [0f32][..]];
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();
        let mut audio = [&mut [0f32][..], &mut [0.], &mut [0.]];
        rt.audio(&mut audio[..], &mut midi, 0., 0, 0.).unwrap();

        // コンパイル間でグローバル変数は共有されない
        rt.compile("let count = 123;").unwrap();
        rt.compile(
            r#"
                "use strict";
                try {
                    count;
                    throw new Error("count should not be defined");
                } catch (e) {
                    if (!(e instanceof ReferenceError)) {
                        throw e;
                    }
                }
            "#,
        )
        .unwrap();
    }

    #[test]
    fn test_gui() {
        let userdata = Arc::new(Mutex::new(vec![]));
        let mut rt = Runtime::new(userdata).unwrap();

        // 入出力の確認
        rt.compile(
            r#"
                "use strict";
                ps88.gui((ctx) => {
                    if (ctx.w !== 640) { throw new Error(`w: ${ctx.w}`); }
                    if (ctx.h !== 480) { throw new Error(`h: ${ctx.h}`); }
                    if (ctx.mouse.x !== 100) { throw new Error(`mouse.x: ${ctx.mouse.x}`); }
                    if (ctx.mouse.y !== 200) { throw new Error(`mouse.y: ${ctx.mouse.y}`); }
                    if (ctx.mouse.pressedL !== true) { throw new Error(`mouse.pressedL: ${ctx.mouse.pressedL}`); }
                    if (ctx.mouse.pressedR !== false) { throw new Error(`mouse.pressedR: ${ctx.mouse.pressedR}`); }
                    ctx.addPolygon([[1, 2], [3, 4]], { fill: 0xff0000, stroke: 0x00ff00, strokeWidth: 2, strokeClosed: true });
                    ctx.addText("Hello", 10, 20, { size: 30, color: 0x0000ff });
                });
            "#,
        )
        .unwrap();
        let args = GuiArgs {
            w: 640.0,
            h: 480.0,
            mouse_x: 100.0,
            mouse_y: 200.0,
            pressed_l: true,
            pressed_r: false,
        };
        let shapes = rt.gui(args).unwrap();
        assert_eq!(
            shapes,
            vec![
                Shape::Polygon {
                    path: vec![(1.0, 2.0), (3.0, 4.0)],
                    fill: Some(0xff0000),
                    stroke: Some(0x00ff00),
                    stroke_width: Some(2.0),
                    stroke_closed: Some(true),
                },
                Shape::Text {
                    text: "Hello".to_string(),
                    x: 10.0,
                    y: 20.0,
                    size: Some(30.0),
                    color: Some(0x0000ff),
                },
            ]
        );

        // エラーが起きたら状態がリセット
        // 3 回目の呼び出しでエラーが起きる
        rt.compile(
            r#"
                "use strict";
                let count = 0;
                ps88.gui((ctx) => {
                    for (let i = count += 1; i > 0; i--) ctx.addPolygon([]);
                    if (count >= 3) throw new Error("test error");
                });
            "#,
        )
        .unwrap();
        let args = GuiArgs::default();
        let shapes = rt.gui(args.clone()).unwrap();
        assert_eq!(shapes.len(), 1);
        let shapes = rt.gui(args.clone()).unwrap();
        assert_eq!(shapes.len(), 2);
        rt.gui(args.clone()).unwrap_err(); // エラー
        let shapes = rt.gui(args.clone()).unwrap(); // 特に何も起きない (関数が登録されていない状態)
        assert_eq!(shapes.len(), 0);
    }

    #[test]
    fn test_save_load() {
        let userdata = Arc::new(Mutex::new(vec![]));
        let mut rt = Runtime::new(userdata.clone()).unwrap();
        let (tx, rx) = channel();
        rt.add_logger(Box::new(move |msg: String| {
            tx.send(msg).unwrap();
            true
        }));

        // 任意のデータを保存できる
        rt.compile("ps88.save(new Uint8Array([1, 2, 3]));").unwrap();
        assert_eq!(&*userdata.lock().unwrap(), &[1, 2, 3]);

        // 保存したデータを読み込める
        rt.compile("console.log(JSON.stringify([...ps88.load()]));")
            .unwrap();
        assert_eq!(rx.recv().unwrap(), "[1,2,3]");

        // データの上書きもできる
        rt.compile("ps88.save(new Uint8Array([4, 5, 6, 7, 8]));")
            .unwrap();
        assert_eq!(&*userdata.lock().unwrap(), &[4, 5, 6, 7, 8]);
        rt.compile("console.log(JSON.stringify([...ps88.load()]));")
            .unwrap();
        assert_eq!(rx.recv().unwrap(), "[4,5,6,7,8]");
    }
}
