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
    input: v8::Global<v8::ArrayBuffer>,
    output: v8::Global<v8::ArrayBuffer>,
    process_func: v8::Global<v8::Function>,
}

#[derive(Debug, Error)]
pub enum JsRuntimeError {
    #[error("failed to compile: `{0}`")]
    CompileError(String),
    #[error("failed to process: `{0}`")]
    ProcessError(String),
    #[error("process function is not compiled")]
    NoProcessFunction,
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

        let (input, output) = {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let input = v8::ArrayBuffer::new(scope, 0);
            let input = v8::Global::new(scope, input);
            let output = v8::ArrayBuffer::new(scope, 0);
            let output = v8::Global::new(scope, output);
            (input, output)
        };

        let process_func = {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let code = match v8::String::new(scope, code) {
                Some(code) => code,
                None => {
                    return Err(
                        JsRuntimeError::UnexpectedError("failed to allocate string".into()).into(),
                    )
                }
            };
            let process_func = {
                let mut try_catch = v8::TryCatch::new(scope);
                let script = match v8::Script::compile(&mut try_catch, code, None) {
                    Some(script) => script,
                    None => {
                        return Err(
                            JsRuntimeError::CompileError(report_exceptions(try_catch)).into()
                        );
                    }
                };
                let process_func = match script.run(&mut try_catch) {
                    Some(process_func) => process_func,
                    None => {
                        return Err(
                            JsRuntimeError::CompileError(report_exceptions(try_catch)).into()
                        );
                    }
                };
                let process_func = match v8::Local::<v8::Function>::try_from(process_func) {
                    Ok(process_func) => process_func,
                    Err(_e) => {
                        return Err(JsRuntimeError::CompileError(
                            "returned value may not be a function".to_string(),
                        )
                        .into())
                    }
                };
                process_func
            };
            let process_func = v8::Global::new(scope, process_func);
            process_func
        };

        let runtime_context = Rc::new(RefCell::new(JsRuntimeContext {
            context,
            _inspector: inspector,
            input,
            output,
            process_func,
        }));
        self.isolate.set_slot(runtime_context);

        Ok(())
    }

    fn process(&mut self, input: &[f32], output: &mut [f32]) -> runtime::Result<()> {
        let runtime_context = match self.isolate.get_slot::<Rc<RefCell<JsRuntimeContext>>>() {
            Some(runtime_context) => runtime_context,
            None => return Err(JsRuntimeError::NoProcessFunction.into()),
        };
        let context = runtime_context.clone();
        let process_func = context.borrow_mut().process_func.clone();
        {
            let context = &mut *context.borrow_mut();
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context.context);
            if v8::Local::new(scope, &context.input).byte_length() != input.len() * size_of::<f32>()
            {
                let array = v8::ArrayBuffer::new(scope, input.len() * size_of::<f32>());
                context.input = v8::Global::new(scope, array);
            }
            if v8::Local::new(scope, &context.output).byte_length()
                != output.len() * size_of::<f32>()
            {
                let array = v8::ArrayBuffer::new(scope, output.len() * size_of::<f32>());
                context.output = v8::Global::new(scope, array);
            }
            let process_func = v8::Local::new(scope, process_func);

            let input_arr = v8::Local::new(scope, &context.input);
            let output_arr = v8::Local::new(scope, &context.output);

            let backing_store = input_arr.get_backing_store();
            if let Some(pointer) = backing_store.data() {
                unsafe {
                    std::ptr::copy(input.as_ptr(), pointer.as_ptr() as *mut f32, input.len());
                }
            }

            let input_array_t = match v8::Float32Array::new(scope, input_arr, 0, input.len()) {
                Some(input_array_t) => input_array_t,
                None => {
                    return Err(JsRuntimeError::UnexpectedError(
                        "failed to create input array".into(),
                    )
                    .into())
                }
            };
            let output_array_t = match v8::Float32Array::new(scope, output_arr, 0, output.len()) {
                Some(output_array_t) => output_array_t,
                None => {
                    return Err(JsRuntimeError::UnexpectedError(
                        "failed to create output array".into(),
                    )
                    .into())
                }
            };

            let this = v8::undefined(scope).into();
            let _result = {
                let mut try_catch = v8::TryCatch::new(scope);
                match process_func.call(
                    &mut try_catch,
                    this,
                    &[input_array_t.into(), output_array_t.into()],
                ) {
                    Some(result) => result,
                    None => {
                        return Err(
                            JsRuntimeError::ProcessError(report_exceptions(try_catch)).into()
                        );
                    }
                }
            };

            let backing_store = output_arr.get_backing_store();
            if let Some(pointer) = backing_store.data() {
                unsafe {
                    std::ptr::copy(
                        pointer.as_ptr() as *const f32,
                        output.as_mut_ptr(),
                        input.len(),
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
    fn process() {
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
            let result = runtime.compile(
                r#"
                    console.log("init: ${i}");
                    let count = 0;
                    (input, output) => {
                        console.log(`init: ${i}, count: ${count++}`);
                        input.forEach((v, i) => {{ output[i] = v * 2.0; }});
                    };
                "#
                .replace("${i}", &i.to_string())
                .as_str(),
            );
            assert!(result.is_ok());

            // process の実行が 3 回行えることを確認
            for _ in 0..3 {
                // 実行ごとに入力配列の数を変える
                let input: Vec<f32> = (0..(i + 1) * 100).map(|x| x as f32).collect();
                let mut output = input.clone();
                let result = runtime.process(&input, &mut output);
                assert!(result.is_ok());
                assert_eq!(
                    output,
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
                (input, output) => {
                    throw new Error('aaa');
                };
            "#,
        );
        assert!(result.is_ok());
        let input: Vec<f32> = (0..100).map(|x| x as f32).collect();
        let mut output = input.clone();
        let result = runtime.process(&input, &mut output);
        assert!(result.is_err());
    }
}
