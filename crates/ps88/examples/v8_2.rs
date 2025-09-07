use deno_core::v8;
use ps88::js::core::*;
use std::cell::RefCell;
use std::rc::Rc;

// === ここからユーザーコード ===

struct MyApi {
    audio_callback: Rc<RefCell<Option<v8::Global<v8::Function>>>>,
}

impl MyApi {
    fn audio(&mut self, mut info: CallbackInfo) {
        let callback = match info.args.get(0).try_cast::<v8::Function>() {
            Ok(callback) => callback,
            Err(_) => {
                let msg = v8::String::new(info.scope, "argument must be a function")
                    .unwrap_or(v8::String::empty(info.scope));
                let err = v8::Exception::type_error(info.scope, msg);
                info.scope.throw_exception(err);
                return;
            }
        };
        let callback = v8::Global::new(info.scope, callback);
        *self.audio_callback.borrow_mut() = Some(callback);
        info.rv.set(v8::Number::new(info.scope, 123f64).into());
    }
}

impl Api for MyApi {
    fn reset(&mut self) {
        self.audio_callback.borrow_mut().take();
    }
}

impl Drop for MyApi {
    fn drop(&mut self) {
        println!("MyApi dropped");
    }
}

/*
struct MyRuntime {
    runtime: JsRuntime<MyStatus>,
    audio_callback: Rc<RefCell<Option<v8::Global<v8::Function>>>>,
}

impl MyRuntime {
    fn new() -> Self {
        let audio_callback = Rc::new(RefCell::new(None));
        let data = MyStatus {
            audio_callback: audio_callback.clone(),
        };
        Self {
            runtime: JsRuntime::new(data),
            audio_callback,
        }
    }
    fn reset(&mut self) {
        self.audio_callback.borrow_mut().take();
        self.runtime.reset();
    }
    fn compile(&mut self, code: &str) -> Result<()> {
        let api = Api::new().add("audio", MyStatus::audio);
        self.runtime.add_api("ps88", &api)?;
        self.runtime.run(code)?;
        Ok(())
    }
    fn audio(
        &mut self,
        audio: &mut [&mut [f32]], // audio[ch][sample]
        _sampling_rate: f32,
        _midi: &[[u8; 7]],
    ) -> Result<()> {
        let scope = &mut self.runtime.scope();

        // audio を Vec<Float32Array> に変換
        // TODO: 毎回メモリ確保するのではなく backing_store をメンバ変数に抱えておく
        let mut audio_js = Vec::with_capacity(audio.len());
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
                return Err(Box::new(JsRuntimeError::UnexpectedError(
                    "failed to create Float32Array".to_string(),
                )));
            };
            audio_js.push(float32array);
        }

        // midi を Vec<Uint8Array> に変換
        // TODO: 実装

        // callback 呼び出し
        let audio_callback = self.audio_callback.borrow_mut();
        let Some(audio_callback) = audio_callback.as_ref() else {
            return Ok(());
        };
        let audio_callback = v8::Local::new(scope, audio_callback);
        let this = v8::undefined(scope).into();
        let arg = v8::Number::new(scope, 42f64).into();
        {
            let try_catch = &mut v8::TryCatch::new(scope);
            let Some(_) = audio_callback.call(try_catch, this, &[arg]) else {
                return Err(Box::new(JsRuntimeError::ProcessError(report_exceptions(
                    try_catch,
                ))));
            };
        }

        // 結果を audio に書き戻す
        for (ch, ch_js) in audio.iter_mut().zip(audio_js.iter()) {
            let Some(array_buffer) = ch_js.buffer(scope) else {
                return Err(Box::new(JsRuntimeError::UnexpectedError(
                    "failed to get ArrayBuffer from Float32Array".to_string(),
                )));
            };
            let backing_store = array_buffer.get_backing_store();
            if let Some(pointer) = backing_store.data() {
                unsafe {
                    std::ptr::copy(pointer.as_ptr() as *const f32, ch.as_mut_ptr(), ch.len());
                }
            }
        }

        // 結果を midi に書き戻す
        // TODO: 実装

        Ok(())
    }
    //fn gui(&mut self, area: &Pos2, mouse: &Mouse) -> Result<Vec<Shape>> {
    //    todo!();
    //}
    //fn add_logger(&mut self, logger: Box<dyn Fn(String) -> bool + Sync + Send>) -> Result<()>;
}
*/

fn main() {
    let audio = Rc::new(RefCell::new(None));
    let data = MyApi {
        audio_callback: audio.clone(),
    };
    let callbacks = Callbacks::new().add("audio", MyApi::audio);
    let mut app = JsRuntime::new(data);
    app.reset();
    app.set_logger(|msg| {
        println!("Console log: {}", msg);
    });
    if let Err(e) = app.add_callbacks("ps88", &callbacks) {
        eprintln!("Failed to add api: {}", e);
        return;
    }
    if let Err(e) = app.run("let a = 100; let b = ps88.audio((n) => (n + a)); console.log(b);") {
        eprintln!("Failed to run script: {}", e);
        return;
    }

    {
        let scope = &mut app.scope();
        let audio = audio.borrow_mut();
        let Some(audio) = audio.as_ref() else {
            eprintln!("audio callback is not set");
            return;
        };
        let callback = v8::Local::new(scope, audio);
        let this = v8::undefined(scope).into();
        let arg = v8::Number::new(scope, 42f64).into();
        let Some(result) = callback.call(scope, this, &[arg]) else {
            eprintln!("Failed to call audio callback");
            return;
        };
        let result = match result.try_cast::<v8::Number>() {
            Ok(result) => result.value(),
            Err(e) => {
                eprintln!("Callback result is not a number: {}", e);
                return;
            }
        };
        println!("Callback result: {:?}", result);
    }

    drop(app);
}
