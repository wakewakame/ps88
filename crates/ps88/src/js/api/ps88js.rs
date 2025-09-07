use super::super::core::*;
use deno_core::{serde_v8, v8};
use std::cell::RefCell;
use std::rc::Rc;

struct PS88JsStatus {
    audio_callback: Option<v8::Global<v8::Function>>,
    gui_callback: Option<v8::Global<v8::Function>>,
}

struct PS88JsApi {
    status: Rc<RefCell<PS88JsStatus>>,
}
impl PS88JsApi {
    fn audio(&mut self, info: CallbackInfo) {
        let Ok(callback) = info.args.get(0).try_cast::<v8::Function>() else {
            let msg = v8::String::new(info.scope, "argument must be a function")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.audio_callback = Some(callback);
    }

    fn gui(&mut self, info: CallbackInfo) {
        let Ok(callback) = info.args.get(0).try_cast::<v8::Function>() else {
            let msg = v8::String::new(info.scope, "argument must be a function")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.audio_callback = Some(callback);
    }
}
impl Api for PS88JsApi {
    fn reset(&mut self) {
        let mut status = self.status.borrow_mut();
        status.audio_callback.take();
        status.gui_callback.take();
    }
}

pub struct PS88JsRuntime {
    status: Rc<RefCell<PS88JsStatus>>,
    runtime: JsRuntime<PS88JsApi>,
    logger: fn(String),
}

impl PS88JsRuntime {
    pub fn new(logger: fn(String)) -> Self {
        let status = Rc::new(RefCell::new(PS88JsStatus {
            audio_callback: None,
            gui_callback: None,
        }));
        let runtime = JsRuntime::new(PS88JsApi {
            status: status.clone(),
        });
        Self {
            status,
            runtime,
            logger,
        }
    }
    pub fn compile(&mut self, code: &str) -> Result<()> {
        self.reset()?;
        self.runtime.run(code)?;
        Ok(())
    }
    pub fn audio(
        &mut self,
        audio: &mut [&mut [f32]], // audio[ch][sample]
        sampling_rate: f32,
        midi: &mut Vec<[u8; 7]>,
    ) -> Result<()> {
        let mut status = self.status.borrow_mut();
        let audio_callback = &mut status.audio_callback;
        let Some(callback) = audio_callback.as_ref() else {
            return Ok(());
        };

        let scope = &mut self.runtime.scope();

        // audio を Vec<Float32Array> に変換
        // TODO: 毎回メモリ確保するのではなく backing_store をメンバ変数に抱えておく
        let mut float32arrays = Vec::with_capacity(audio.len());
        for ch in audio.iter() {
            // NOTE: ArrayBuffer::new() で生成されるメモリはどうやら 16 byte 境界にアラインされるらしいので多分安全...? (ちゃんと確認していない)
            // BackingStore のメモリを自分で生成する方法もあるが、安全なのかよくわかっていないので今回はやめておく。
            let array_buffer = v8::ArrayBuffer::new(scope, ch.len() * std::mem::size_of::<f32>());
            let backing_store = array_buffer.get_backing_store();
            if let Some(pointer) = backing_store.data() {
                unsafe {
                    std::ptr::copy(ch.as_ptr(), pointer.as_ptr() as *mut f32, ch.len());
                }
            }
            let Some(float32array) = v8::Float32Array::new(scope, array_buffer, 0, ch.len()) else {
                return Err(JsRuntimeError::UnexpectedError(
                    "failed to create Float32Array".to_string(),
                ));
            };
            float32arrays.push(float32array);
        }
        let audio_js = v8::Array::new_with_elements(
            scope,
            float32arrays
                .clone()
                .into_iter()
                .map(|f32a| f32a.into())
                .collect::<Vec<v8::Local<v8::Value>>>()
                .as_slice(),
        );

        // midi を v8 に変換
        let midi_js = wrap_err(serde_v8::to_v8(scope, midi.clone()))?;

        // sampling_rate を v8 に変換
        let sampling_rate = v8::Number::new(scope, sampling_rate as f64);

        // 引数を用意
        let arg = v8::Object::new(scope);
        let key = v8str(scope, "audio")?.into();
        arg.set(scope, key, audio_js.into());
        let key = v8str(scope, "midi")?.into();
        arg.set(scope, key, midi_js.into());
        let key = v8str(scope, "sampling_rate")?.into();
        arg.set(scope, key, sampling_rate.into());

        // callback 呼び出し
        let callback = v8::Local::new(scope, callback);
        let this = v8::undefined(scope).into();
        {
            let try_catch = &mut v8::TryCatch::new(scope);
            let Some(_) = callback.call(try_catch, this, &[arg.into()]) else {
                audio_callback.take();
                return Err(JsRuntimeError::RuntimeError(report_exceptions(try_catch)));
            };
        }

        // 結果を audio に書き戻す
        for (ch, ch_js) in audio.iter_mut().zip(float32arrays.iter()) {
            let Some(array_buffer) = ch_js.buffer(scope) else {
                return Err(JsRuntimeError::UnexpectedError(
                    "failed to get ArrayBuffer from Float32Array".to_string(),
                ));
            };
            let backing_store = array_buffer.get_backing_store();
            if let Some(pointer) = backing_store.data() {
                unsafe {
                    std::ptr::copy(pointer.as_ptr() as *const f32, ch.as_mut_ptr(), ch.len());
                }
            }
        }

        // 結果を midi に書き戻す
        match serde_v8::from_v8::<Vec<[u8; 7]>>(scope, midi_js) {
            Ok(m) => {
                *midi = m;
            }
            Err(e) => {
                return Err(JsRuntimeError::UnexpectedError(format!(
                    "failed to convert midi from v8: {}",
                    e
                )))
            }
        };

        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        self.runtime.reset();
        let callbacks = Callbacks::new()
            .add("audio", PS88JsApi::audio)
            .add("gui", PS88JsApi::gui);
        self.runtime.add_callbacks("ps88", &callbacks)?;
        self.runtime.set_logger(self.logger);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio() {
        let mut rt = PS88JsRuntime::new(|_| {});
        rt.compile(
            r#"ps88.audio((arg) => {
    if (arg.sampling_rate !== 48000.0) {
        throw new Error("sampling_rate must be 48000.0");
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
        rt.audio(audio_slice.as_mut_slice(), 48000.0, &mut midi)
            .unwrap();
        assert_eq!(audio, vec![vec![0.2f32, 0.4, 0.6], vec![0.8, 1.0, 1.2]]);
        assert_eq!(
            midi,
            vec![[0, 0, 0, 0, 0x80, 69, 20], [0, 0, 0, 0, 0x80, 57, 30]]
        );
    }
}
