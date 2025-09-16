use super::super::core;
use super::convert::*;
use super::global_api::*;
use super::gui_api::*;
use super::status::*;
use deno_core::{serde, serde_v8, v8};
use std::cell::RefCell;
use std::rc::Rc;

pub struct Runtime<'a> {
    status: Rc<RefCell<Status>>,
    runtime: core::JsRuntime<'a>,
    audio_buf: Option<v8::SharedRef<v8::BackingStore>>,
}

impl<'a> Runtime<'a> {
    pub fn new<F: Fn(String) + 'a>(logger: F) -> core::Result<Self> {
        let status = Rc::new(RefCell::new(Status {
            audio_callback: None,
            gui_callback: None,
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
        let runtime = core::JsRuntimeBuilder::new()
            .add_api(api)
            .add_logger(logger)
            .build()?;
        Ok(Self {
            status,
            runtime,
            audio_buf: None,
        })
    }
    pub fn reset(&mut self) -> core::Result<()> {
        self.runtime.reset()
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
        current_frame: u64,
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
            let arg = Dict::new(scope)
                .add("audio", audio_js)?
                .add("midi", midi_js)?
                .add_number("sampleRate", sample_rate)?
                .add_number("currentFrame", current_frame as f64)?
                .add_number("bpm", bpm)?
                .value();

            // callback 呼び出し
            let this = v8::undefined(scope).into();
            {
                let try_catch = &mut v8::TryCatch::new(scope);
                let Some(_) = callback.call(try_catch, this, &[arg]) else {
                    return Err(core::JsRuntimeError::RuntimeError(core::report_exceptions(
                        try_catch,
                    )));
                };
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
            let arg = Dict::new(scope)
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
                let Some(_) = callback.call(try_catch, this, &[arg]) else {
                    return Err(core::JsRuntimeError::RuntimeError(core::report_exceptions(
                        try_catch,
                    )));
                };
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
}

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

    #[test]
    fn test_audio() {
        let mut rt = Runtime::new(|_| {}).unwrap();

        // 入出力の確認
        rt.compile(
            r#"
                "use strict";
                ps88.audio((ctx) => {
                    if (ctx.sampleRate !== 48000) { throw new Error(`sampleRate: ${ctx.sampleRate}`); }
                    if (ctx.currentFrame !== 1024) { throw new Error(`currentFrame: ${ctx.currentFrame}`); }
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
        let mut rt = Runtime::new(|msg| println!("{}", msg)).unwrap();

        // 入出力の確認
        rt.compile(
            r#"
                "use strict";
                ps88.gui((arg) => {
                    if (arg.w !== 640) { throw new Error(`w: ${arg.w}`); }
                    if (arg.h !== 480) { throw new Error(`h: ${arg.h}`); }
                    if (arg.mouse.x !== 100) { throw new Error(`mouse.x: ${arg.mouse.x}`); }
                    if (arg.mouse.y !== 200) { throw new Error(`mouse.y: ${arg.mouse.y}`); }
                    if (arg.mouse.pressedL !== true) { throw new Error(`mouse.pressedL: ${arg.mouse.pressedL}`); }
                    if (arg.mouse.pressedR !== false) { throw new Error(`mouse.pressedR: ${arg.mouse.pressedR}`); }
                    arg.addPolygon([[1, 2], [3, 4]], { fill: 0xff0000 });
                    arg.addText("Hello", 10, 20, { color: 0x00ff00 });
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
                    stroke: None,
                    stroke_width: None,
                    stroke_closed: None,
                },
                Shape::Text {
                    text: "Hello".to_string(),
                    x: 10.0,
                    y: 20.0,
                    size: None,
                    color: Some(0x00ff00),
                },
            ]
        );
    }
}
