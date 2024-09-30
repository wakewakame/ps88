use crate::runtime::runtime;
use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;
use std::sync::Once;
use thiserror::Error;
use v8;

pub struct JsRuntimeBuilder {
    on_log: Option<Rc<dyn Fn(String)>>,
}

pub struct JsRuntime {
    isolate: v8::OwnedIsolate,
    on_log: Option<Rc<dyn Fn(String)>>,
}

struct JsRuntimeContext {
    context: v8::Global<v8::Context>,
    _inspector: Option<Rc<RefCell<InspectorClient>>>,
    audio: v8::Global<v8::ArrayBuffer>,
    audio_func: v8::Global<v8::Function>,
    gui_func: v8::Global<v8::Function>,
}

#[derive(Debug, Error)]
pub enum JsRuntimeError {
    #[error("failed to compile: `{0}`")]
    CompileError(String),
    #[error("failed to process: `{0}`")]
    ProcessError(String),
    #[error("not compiled")]
    NotCompiled,
    #[error("unexpected error: {0}")]
    UnexpectedError(String),
}

impl JsRuntimeBuilder {
    pub fn new() -> Self {
        JsRuntimeBuilder { on_log: None }
    }

    pub fn build(self) -> JsRuntime {
        static PUPPY_INIT: Once = Once::new();
        PUPPY_INIT.call_once(move || {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });
        let isolate = v8::Isolate::new(Default::default());
        JsRuntime {
            isolate,
            on_log: self.on_log,
        }
    }

    pub fn on_log(mut self, on_log: Rc<dyn Fn(String)>) -> Self {
        self.on_log = Some(on_log);
        self
    }
}

impl runtime::ScriptRuntime for JsRuntime {
    fn compile(&mut self, code: &str) -> runtime::Result<()> {
        // MEMO:
        //   新しい inspector を作った後に set_slot で古い inspector を drop すると
        //   古い inspector のデストラクタが新しい inspector に影響して console.log
        //   の出力を得られなくなってしまうため、先にここで古いインスタンスを drop しておく。
        self.isolate.remove_slot::<Rc<RefCell<JsRuntimeContext>>>();

        let context = {
            let handle_scope = &mut v8::HandleScope::new(&mut self.isolate);
            let context = v8::Context::new(handle_scope);
            v8::Global::new(handle_scope, context)
        };

        let on_log = self.on_log.clone();
        let inspector = if let Some(on_log) = on_log {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let context = v8::Local::new(scope, &context);
            let inspector = InspectorClient::new(scope, context, on_log)?;
            Some(inspector)
        } else {
            None
        };

        let audio = {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let audio = v8::ArrayBuffer::new(scope, 0);
            let audio = v8::Global::new(scope, audio);
            audio
        };

        let (audio_func, gui_func) = {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let Some(code) = v8::String::new(scope, code) else {
                return Err(
                    JsRuntimeError::UnexpectedError("failed to allocate string".into()).into(),
                );
            };
            let funcs = {
                let mut try_catch = v8::TryCatch::new(scope);
                let Some(script) = v8::Script::compile(&mut try_catch, code, None) else {
                    return Err(JsRuntimeError::CompileError(report_exceptions(try_catch)).into());
                };
                if script.run(&mut try_catch).is_none() {
                    return Err(JsRuntimeError::CompileError(report_exceptions(try_catch)).into());
                };

                const EXPECTED_FUNCTION_NAMES: [&str; 2] = ["audio", "gui"];
                let expected_functions: runtime::Result<Vec<v8::Local<v8::Function>>> =
                    EXPECTED_FUNCTION_NAMES
                        .iter()
                        .map(|name| {
                            let mut try_catch = v8::TryCatch::new(&mut try_catch);
                            let Some(code) = v8::String::new(&mut try_catch, name) else {
                                return Err(JsRuntimeError::CompileError(format!(
                                    "failed to allocate string: '{}'",
                                    name
                                ))
                                .into());
                            };
                            let Some(script) = v8::Script::compile(&mut try_catch, code, None)
                            else {
                                return Err(JsRuntimeError::CompileError(report_exceptions(
                                    try_catch,
                                ))
                                .into());
                            };
                            let Some(variable) = script.run(&mut try_catch) else {
                                return Err(JsRuntimeError::CompileError(report_exceptions(
                                    try_catch,
                                ))
                                .into());
                            };
                            if variable.is_undefined() {
                                return Err(JsRuntimeError::CompileError(format!(
                                    "'{}' function is not defined",
                                    name,
                                ))
                                .into());
                            }
                            let Ok(func) = v8::Local::<v8::Function>::try_from(variable) else {
                                return Err(JsRuntimeError::CompileError(format!(
                                    "'{}' is not a function",
                                    name,
                                ))
                                .into());
                            };
                            return Ok(func);
                        })
                        .collect();
                expected_functions
            }?;
            let funcs: Vec<v8::Global<v8::Function>> =
                funcs.iter().map(|f| v8::Global::new(scope, *f)).collect();
            let Some([audio_func, gui_func]) = funcs.get(0..2) else {
                return Err(
                    JsRuntimeError::UnexpectedError("failed to get functions".into()).into(),
                );
            };
            (audio_func.clone(), gui_func.clone())
        };

        let runtime_context = Rc::new(RefCell::new(JsRuntimeContext {
            context,
            _inspector: inspector,
            audio,
            audio_func,
            gui_func,
        }));
        self.isolate.set_slot(runtime_context);

        Ok(())
    }

    fn audio(
        &mut self,
        audio: &mut [f32],
        ch: usize,
        sampling_rate: f32,
        midi: &[u8],
    ) -> runtime::Result<()> {
        let Some(runtime_context) = self.isolate.get_slot::<Rc<RefCell<JsRuntimeContext>>>() else {
            return Err(JsRuntimeError::NotCompiled.into());
        };
        let context = runtime_context.clone();
        let audio_func = context.borrow_mut().audio_func.clone();
        {
            let context = &mut *context.borrow_mut();
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context.context);
            if v8::Local::new(scope, &context.audio).byte_length() != audio.len() * size_of::<f32>()
            {
                let array = v8::ArrayBuffer::new(scope, audio.len() * size_of::<f32>());
                context.audio = v8::Global::new(scope, array);
            }
            let audio_arr = v8::Local::new(scope, &context.audio);
            let midi_arr = v8::ArrayBuffer::new(scope, midi.len() * size_of::<f32>());
            let audio_backing_store = audio_arr.get_backing_store();
            let midi_backing_store = midi_arr.get_backing_store();
            if let Some(pointer) = audio_backing_store.data() {
                unsafe {
                    std::ptr::copy(audio.as_ptr(), pointer.as_ptr() as *mut f32, audio.len());
                }
            }
            if let Some(pointer) = midi_backing_store.data() {
                unsafe {
                    std::ptr::copy(midi.as_ptr(), pointer.as_ptr() as *mut u8, midi.len());
                }
            }
            let Some(audio_array_t) = v8::Float32Array::new(scope, audio_arr, 0, audio.len())
            else {
                return Err(
                    JsRuntimeError::UnexpectedError("failed to create audio array".into()).into(),
                );
            };
            let Some(midi_array_t) = v8::Uint8Array::new(scope, midi_arr, 0, midi.len()) else {
                return Err(
                    JsRuntimeError::UnexpectedError("failed to create midi array".into()).into(),
                );
            };
            let ctx = v8::Object::new(scope);
            let audio_key = v8::String::new(scope, "audio").unwrap();
            let ch_key = v8::String::new(scope, "ch").unwrap();
            let sampling_rate_key = v8::String::new(scope, "sampling_rate").unwrap();
            let midi_key = v8::String::new(scope, "midi").unwrap();
            let ch = v8::Integer::new(scope, ch as i32);
            let sampling_rate = v8::Number::new(scope, sampling_rate as f64);
            ctx.set(scope, audio_key.into(), audio_array_t.into());
            ctx.set(scope, ch_key.into(), ch.into());
            ctx.set(scope, sampling_rate_key.into(), sampling_rate.into());
            ctx.set(scope, midi_key.into(), midi_array_t.into());

            let audio_func = v8::Local::new(scope, audio_func);
            let this = v8::undefined(scope).into();
            let _result = {
                let mut try_catch = v8::TryCatch::new(scope);
                match audio_func.call(&mut try_catch, this, &[ctx.into()]) {
                    Some(result) => result,
                    None => {
                        return Err(
                            JsRuntimeError::ProcessError(report_exceptions(try_catch)).into()
                        );
                    }
                }
            };

            let audio_backing_store = audio_arr.get_backing_store();
            if let Some(pointer) = audio_backing_store.data() {
                unsafe {
                    std::ptr::copy(
                        pointer.as_ptr() as *const f32,
                        audio.as_mut_ptr(),
                        audio.len(),
                    );
                }
            }
        }

        Ok(())
    }
}

struct InspectorClient {
    v8_inspector_client: v8::inspector::V8InspectorClientBase,
    v8_inspector: Rc<RefCell<v8::UniquePtr<v8::inspector::V8Inspector>>>,
    on_log: Rc<dyn Fn(String)>,
}

impl InspectorClient {
    fn new(
        scope: &mut v8::HandleScope,
        context: v8::Local<v8::Context>,
        on_log: Rc<dyn Fn(String)>,
    ) -> runtime::Result<Rc<RefCell<Self>>> {
        let v8_inspector_client = v8::inspector::V8InspectorClientBase::new::<Self>();
        let self__ = Rc::new(RefCell::new(Self {
            v8_inspector_client,
            v8_inspector: Default::default(),
            on_log,
        }));
        {
            // MEMO: self__ が drop される前に client が無効な参照になると segfault するので注意
            let mut self_ = self__.borrow_mut();
            let client = &mut *self_;
            self_.v8_inspector = Rc::new(RefCell::new(
                v8::inspector::V8Inspector::create(scope, client).into(),
            ));
            let context_name = v8::inspector::StringView::from(&b"main realm"[..]);
            let aux_data = r#"{"isDefault": true}"#;
            let aux_data_view = v8::inspector::StringView::from(aux_data.as_bytes());
            match self_.v8_inspector.borrow_mut().as_mut() {
                Some(v8_inspector) => {
                    v8_inspector.context_created(context, 1, context_name, aux_data_view)
                }
                None => {
                    return Err(JsRuntimeError::UnexpectedError(
                        "failed to create inspector".into(),
                    )
                    .into())
                }
            };
        }

        Ok(self__)
    }
}

impl v8::inspector::V8InspectorClientImpl for InspectorClient {
    fn base(&self) -> &v8::inspector::V8InspectorClientBase {
        &self.v8_inspector_client
    }

    fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
        &mut self.v8_inspector_client
    }

    unsafe fn base_ptr(this: *const Self) -> *const v8::inspector::V8InspectorClientBase
    where
        Self: Sized,
    {
        // SAFETY: this pointer is valid for the whole lifetime of inspector
        unsafe { std::ptr::addr_of!((*this).v8_inspector_client) }
    }

    fn console_api_message(
        &mut self,
        _context_group_id: i32,
        _level: i32,
        message: &v8::inspector::StringView,
        _url: &v8::inspector::StringView,
        _line_number: u32,
        _column_number: u32,
        _stack_trace: &mut v8::inspector::V8StackTrace,
    ) {
        // ログメッセージの出力
        (self.on_log)(message.to_string());
    }
}

// TryCatch からエラー情報を文字列に変換する
fn report_exceptions(mut try_catch: v8::TryCatch<v8::HandleScope>) -> String {
    let mut description = Vec::<String>::new();
    let Some(exception) = try_catch.exception() else {
        return "no error".into();
    };
    let Some(exception_string) = exception.to_string(&mut try_catch) else {
        return "unexpected error".into();
    };
    let exception_string = exception_string.to_rust_string_lossy(&mut try_catch);
    let Some(message) = try_catch.message() else {
        return exception_string;
    };

    // 該当箇所の出力
    // e.g.
    //   main.js:5: SyntaxError: Unexpected token '=='
    let filename = message
        .get_script_resource_name(&mut try_catch)
        .and_then(|s| s.to_string(&mut try_catch))
        .map(|s| s.to_rust_string_lossy(&mut try_catch))
        .unwrap_or("(unknown)".into());
    let line_number = message
        .get_line_number(&mut try_catch)
        .map(|n| n.to_string())
        .unwrap_or("(unknown)".into());
    description.push(format!(
        "{}:{}: {}",
        filename, line_number, exception_string
    ));

    // 該当箇所のコードを出力
    // e.g.
    //   let a == 1;
    //         ^^
    if let Some(source_line) = message.get_source_line(&mut try_catch) {
        let source_line = source_line.to_rust_string_lossy(&mut try_catch);
        let start_column = message.get_start_column();
        let end_column = message.get_end_column();
        description.push(format!(
            "\n{}\n{}{}\n",
            source_line,
            " ".repeat(start_column),
            "^".repeat(end_column - start_column)
        ));
    }

    // スタックトレースを出力
    // e.g.
    //   Error: aaa
    //       at f3 (<anonymous>:4:26)
    //       at f2 (<anonymous>:3:20)
    //       at f1 (<anonymous>:2:20)
    //       at main (<anonymous>:1:22)
    //       at <anonymous>:5:1
    if let Some(stack_trace) = try_catch
        .stack_trace()
        .and_then(|s| s.to_string(&mut try_catch))
        .map(|s| s.to_rust_string_lossy(&mut try_catch))
    {
        description.push(format!("{}", stack_trace));
    }

    return description.join("\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::runtime;

    #[test]
    fn audio() {
        // console.log の出力結果保存用
        let logs = Rc::new(RefCell::<Vec<String>>::new(vec![]));
        let logs_clone = logs.clone();

        // 初期化
        let mut runtime: Box<dyn runtime::ScriptRuntime> = Box::new(
            JsRuntimeBuilder::new()
                .on_log(Rc::new(move |log| {
                    let mut logs = logs_clone.borrow_mut();
                    logs.push(log);
                }))
                .build(),
        );

        // compile が 3 回行えることを確認
        for i in 0..3 {
            runtime
                .compile(
                    r#"
                    "use strict";
                    console.log("init: ${i}");
                    let count = 0;
                    const audio = (ctx) => {
                        console.log(`init: ${i}, count: ${count++}`);
                        for (let i = 0; i < ctx.audio.length; i++) {
                            ctx.audio[i] = ctx.audio[i] * 2.0;
                        }
                    };
                    const gui = () => {};
                "#
                    .replace("${i}", &i.to_string())
                    .as_str(),
                )
                .unwrap();

            // audio の実行が 3 回行えることを確認
            for _ in 0..3 {
                // 実行ごとに入力配列の数を変える
                let mut audio: Vec<f32> = (0..(i + 1) * 100).map(|x| x as f32).collect();
                runtime.audio(&mut audio, 2, 48000.0, &[]).unwrap();
                assert_eq!(
                    audio,
                    (0..(i + 1) * 100)
                        .map(|x| (x * 2) as f32)
                        .collect::<Vec<f32>>()
                );
            }
        }

        // console.log が取得できていることを確認
        let logs = logs.borrow();
        assert_eq!(logs.len(), 12);
        assert_eq!(logs[0], "init: 0");
        assert_eq!(logs[1], "init: 0, count: 0");
        assert_eq!(logs[2], "init: 0, count: 1");
        assert_eq!(logs[3], "init: 0, count: 2");
        assert_eq!(logs[4], "init: 1");
        assert_eq!(logs[5], "init: 1, count: 0");
        assert_eq!(logs[6], "init: 1, count: 1");
        assert_eq!(logs[7], "init: 1, count: 2");
        assert_eq!(logs[8], "init: 2");
        assert_eq!(logs[9], "init: 2, count: 0");
        assert_eq!(logs[10], "init: 2, count: 1");
        assert_eq!(logs[11], "init: 2, count: 2");
    }

    #[test]
    fn compile_error() {
        let mut runtime: Box<dyn runtime::ScriptRuntime> =
            Box::new(JsRuntimeBuilder::new().build());

        // 不正な構文
        let result = runtime.compile("let a == 1;");
        assert!(result.is_err());

        // 関数を返す前に例外
        let result = runtime.compile("new Error('aaa');");
        assert!(result.is_err());

        // 返される値が関数でない
        let result = runtime.compile("undefined;");
        assert!(result.is_err());
    }

    #[test]
    fn process_error() {
        let mut runtime: Box<dyn runtime::ScriptRuntime> =
            Box::new(JsRuntimeBuilder::new().build());

        // 処理中に例外
        let result = runtime.compile(
            r#"
                "use strict";
                const audio = (_) => {
                    throw new Error('aaa');
                };
                const gui = () => {};
            "#,
        );
        assert!(result.is_ok());
        let mut audio: Vec<f32> = (0..100).map(|x| x as f32).collect();
        let result = runtime.audio(&mut audio, 2, 48000.0, &[]);
        assert!(result.is_err());
    }
}
