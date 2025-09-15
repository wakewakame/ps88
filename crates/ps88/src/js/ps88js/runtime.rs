use super::super::core;
use super::api::*;
use super::convert::*;
use super::shape_api::*;
use super::status::*;
use deno_core::v8;
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
    pub fn gui(&mut self, args: GuiArgs, shapes: &mut Shapes) -> core::Result<()> {
        let result = || -> core::Result<()> {
            let scope = &mut self.runtime.scope();
            let callback = {
                let status = self.status.borrow();
                let Some(callback) = status.gui_callback.as_ref() else {
                    return Ok(()); // callback が登録されていなければ何もしない
                };
                v8::Local::new(scope, callback)
            };

            let mouse = Dict::new(scope)
                .add_number("x", args.mouse_x)?
                .add_number("y", args.mouse_y)?
                .add_bool("pressedL", args.pressed_l)?
                .add_bool("pressedR", args.pressed_r)?
                .value();
            let arg = Dict::new(scope)
                .add_number("w", args.w)?
                .add_number("h", args.h)?
                .add("mouse", mouse)?
                // TODO
                //.add("addShape", v8::null(scope).into())?
                //.add("addText", v8::null(scope).into())?
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

            Ok(())
        }();
        if result.is_err() {
            // エラーが起きたら状態をリセット
            self.reset()?;
        }
        result
    }
}

pub(crate) struct GuiArgs {
    w: f64,
    h: f64,
    mouse_x: f64,
    mouse_y: f64,
    pressed_l: bool,
    pressed_r: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio() {
        let mut rt = Runtime::new(|_| {}).unwrap();
        rt.compile(
            r#"ps88.audio((arg) => {
    if (arg.sampleRate !== 48000.0) {
        throw new Error("sampleRate must be 48000.0");
    }
    if (arg.currentFrame !== 1024) {
        throw new Error("currentFrame must be 1024");
    }
    if (arg.bpm !== 120.0) {
        throw new Error("bpm must be 120.0");
    }
    let audio = arg.audio;
    let midi = arg.midi;
    for (let ch = 0; ch < audio.length; ch++) {
        for (let i = 0; i < audio[ch].length; i++) {
            audio[ch][i] *= 2.0;
        }
    }
    for (let ev = 0; ev < midi.length; ev++) {
        midi[ev][6] += 10;
    }
    midi.push([0, 0, 0, 0, 0x80, 57, 30]);
});"#,
        )
        .unwrap();
        let mut audio = vec![vec![0.1f32, 0.2, 0.3], vec![0.4, 0.5, 0.6]];
        let mut midi = vec![[0, 0, 0, 0, 0x80, 69, 10]];
        let mut audio_slice = audio
            .iter_mut()
            .map(|ch| ch.as_mut_slice())
            .collect::<Vec<_>>();
        rt.audio(audio_slice.as_mut_slice(), &mut midi, 48000.0, 1024, 120.0)
            .unwrap();
        assert_eq!(audio, vec![vec![0.2f32, 0.4, 0.6], vec![0.8, 1.0, 1.2]]);
        assert_eq!(
            midi,
            vec![[0, 0, 0, 0, 0x80, 69, 20], [0, 0, 0, 0, 0x80, 57, 30]]
        );
    }
}
