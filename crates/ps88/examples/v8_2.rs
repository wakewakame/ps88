/*
TODO
- テストを書く
*/

use deno_core::v8;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Once;
use thiserror::Error;

// JavaScript の実行環境
struct JsRuntime<Status> {
    // NOTE: _inspector と context は isolate に紐づくので、これらは isolate より先に drop される必要がある
    _inspector: Option<Inspector>,
    context: v8::Global<v8::Context>,

    // NOTE: isolate は間接的に status の生ポインタを参照しているため、isolate は status より先に drop される必要がある
    isolate: v8::OwnedIsolate,

    // Rust 側のコールバック関数に渡すステータス情報
    status: Status,
}

impl<Status> JsRuntime<Status> {
    fn new(status: Status) -> Self {
        static PUPPY_INIT: Once = Once::new();
        PUPPY_INIT.call_once(move || {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });
        let mut isolate = v8::Isolate::new(Default::default());
        let context = {
            let handle_scope = &mut v8::HandleScope::new(&mut isolate);
            let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
            v8::Global::new(handle_scope, context)
        };
        Self {
            _inspector: None,
            context,
            isolate,
            status,
        }
    }

    // JavaScript の実行環境をリセット
    // set_logger() や add_api() の設定もリセットされる。
    fn reset(&mut self) {
        self._inspector = None;
        self.context = {
            let handle_scope = &mut v8::HandleScope::new(&mut self.isolate);
            let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
            v8::Global::new(handle_scope, context)
        };
    }

    // console.log() の出力を得るためのロガーを設定
    fn set_logger<F: Fn(String) + 'static>(&mut self, logger: F) {
        // NOTE:
        // 新しい inspector を作った後に古い inspector を drop すると
        // 古い inspector のデストラクタが新しい inspector に影響して
        // console.log の出力を得られなくなってしまう。
        // そのため、先にここで古いインスタンスを drop しておく。
        self._inspector = None;
        self._inspector = {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
            let context = v8::Local::new(scope, &self.context);
            let inspector = Inspector::new(scope, context, logger);
            Some(inspector)
        };
    }

    // api のコールバック関数を登録
    fn add_api(&mut self, name: &str, api: &Api<Status>) -> Result<()> {
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let context = v8::Local::new(scope, &self.context);
        let obj_t = v8::ObjectTemplate::new(scope);
        let status = v8::External::new(
            scope,
            &mut self.status as *mut Status as *mut std::ffi::c_void,
        );
        for (name, func) in api.callbacks.iter() {
            let Some(name) = v8::String::new(scope, name) else {
                return Err(Box::new(JsRuntimeError::UnexpectedError(format!(
                    "failed to create string: {}",
                    name
                ))));
            };
            let func = v8::FunctionBuilder::<v8::FunctionTemplate>::new_raw(*func)
                .data(status.into())
                .build(scope);
            obj_t.set(name.into(), func.into());
        }
        let Some(obj) = obj_t.new_instance(scope) else {
            return Err(Box::new(JsRuntimeError::UnexpectedError(
                "failed to create api object".to_string(),
            )));
        };
        let Some(name) = v8::String::new(scope, &name) else {
            return Err(Box::new(JsRuntimeError::UnexpectedError(format!(
                "failed to create string: {}",
                name
            ))));
        };
        context.global(scope).set(scope, name.into(), obj.into());
        Ok(())
    }

    // スクリプトを実行
    fn run(&mut self, code: &str) -> Result<()> {
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let Some(code) = v8::String::new(scope, code) else {
            return Err(Box::new(JsRuntimeError::UnexpectedError(
                "failed to create script string".to_string(),
            )));
        };
        let try_catch = &mut v8::TryCatch::new(scope);
        let Some(script) = v8::Script::compile(try_catch, code, None) else {
            return Err(Box::new(JsRuntimeError::CompileError(report_exceptions(
                try_catch,
            ))));
        };
        let Some(_) = script.run(try_catch) else {
            return Err(Box::new(JsRuntimeError::ProcessError(report_exceptions(
                try_catch,
            ))));
        };
        Ok(())
    }

    fn scope(&mut self) -> v8::HandleScope {
        v8::HandleScope::with_context(&mut self.isolate, &self.context)
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[derive(Debug, Error)]
enum JsRuntimeError {
    #[error("failed to compile: `{0}`")]
    CompileError(String),
    #[error("failed to process: `{0}`")]
    ProcessError(String),
    #[error("unexpected error: {0}")]
    UnexpectedError(String),
}

// JavaScript 側から呼び出される関数を登録するための構造体
struct Api<Status> {
    // JavaScript 側から呼び出される関数
    // 安全性: 呼び出し側は以下の 2 点を保証する必要がある
    // 1. 引数 `info: *const FunctionCallbackInfo` には有効なポインタを渡す
    // 2. 引数 `info.args.data()` には `&Status` を `v8::External` で囲った値が返るようにする
    callbacks: HashMap<String, v8::FunctionCallback>,

    // Status 型を保持するためのフィールド
    // Status 型を保持する理由は `add()` で間違った型の関数が登録されることを防ぐため。
    _phantom: std::marker::PhantomData<Status>,
}

impl<Status> Api<Status> {
    fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    // JavaScript 側から呼び出される関数を登録する
    fn add<F: Fn(&Status, CallbackInfo) + Sized>(mut self, name: &str, _: F) -> Self {
        // 関数の型をチェックする
        const {
            assert!(
                size_of::<F>() == 0,
                "the provided closure must not capture any variables"
            )
        }

        // Rust の関数やクロージャを v8::FunctionCallback としてラップする
        //
        // MEMO: rusty_v8 には以下のようにクロージャを登録できる仕組みがあり、これを参考に実装している。
        //
        // ```
        // v8::FunctionBuilder::<v8::FunctionTemplate>::new(
        //     |scope: &mut v8::HandleScope, args: v8::FunctionCallbackArguments, rv: v8::ReturnValue| {
        //         // implementation
        //     }
        // );
        // ```
        unsafe extern "C" fn f<Status, F: Fn(&Status, CallbackInfo) + Sized>(
            info: *const v8::FunctionCallbackInfo,
        ) {
            // 引数を取り出す
            // 安全性: `info` には有効なポインタが渡されることは呼び出し側が保証する
            // 参考: https://github.com/denoland/rusty_v8/blob/c2bac76486b5db090587e3f40988a8033ce81773/src/function.rs#L509-L513
            let info = unsafe { &*info };
            let scope = &mut unsafe { v8::CallbackScope::new(info) };
            let args = v8::FunctionCallbackArguments::from_function_callback_info(info);
            let rv = v8::ReturnValue::from_function_callback_info(info);
            let info = CallbackInfo { scope, args, rv };

            // `info.args.data()` から `&Status` を取り出す
            // 安全性: `info.args.data()` に &Status が格納されていることは呼び出し側が保証する
            let status = info.args.data().try_cast::<v8::External>().unwrap();
            let status = status.value().cast::<Status>();
            let status = unsafe { &*status };

            // コールバック関数の型から関数のインスタンスを生成する
            // 安全性: 関数のサイズは 0 である必要があるが、それは前段の assert で保証されている
            // 参考: https://github.com/denoland/rusty_v8/blob/c2bac76486b5db090587e3f40988a8033ce81773/src/support.rs#L494
            let f = unsafe { std::mem::zeroed::<F>() };

            // コールバック関数を呼び出す
            f(status, info);
        }
        self.callbacks.insert(name.to_string(), f::<Status, F>);
        self
    }
}

struct CallbackInfo<'a, 'b> {
    scope: &'a mut v8::HandleScope<'b>,
    args: v8::FunctionCallbackArguments<'a>,
    rv: v8::ReturnValue<'a>,
}

struct Inspector {
    // NOTE: _inspector は _client の参照を抱えるため client より先に drop される必要がある
    _inspector: v8::UniqueRef<v8::inspector::V8Inspector>,
    _client: Box<InspectorClient>,
}

impl Inspector {
    fn new<F: Fn(String) + 'static>(
        scope: &mut v8::HandleScope,
        context: v8::Local<v8::Context>,
        logger: F,
    ) -> Self {
        let mut client = {
            let base = v8::inspector::V8InspectorClientBase::new::<InspectorClient>();
            let logger = Box::new(logger);
            Box::new(InspectorClient { base, logger })
        };
        let inspector = {
            let mut inspector = v8::inspector::V8Inspector::create(scope, &mut *client);
            let context_name = v8::inspector::StringView::from(&b"main realm"[..]);
            let aux_data = v8::inspector::StringView::from(r#"{"isDefault": true}"#.as_bytes());
            inspector.context_created(context, 1, context_name, aux_data);
            inspector
        };
        Self {
            _inspector: inspector,
            _client: client,
        }
    }
}

struct InspectorClient {
    base: v8::inspector::V8InspectorClientBase,
    logger: Box<dyn Fn(String)>,
}

// 参考: https://github.com/denoland/deno_core/blob/75759fb5127982bdaf71e68f04dee01531d6591b/core/inspector.rs#L119
impl v8::inspector::V8InspectorClientImpl for InspectorClient {
    fn base(&self) -> &v8::inspector::V8InspectorClientBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut v8::inspector::V8InspectorClientBase {
        &mut self.base
    }

    unsafe fn base_ptr(this: *const Self) -> *const v8::inspector::V8InspectorClientBase
    where
        Self: Sized,
    {
        // SAFETY: this pointer is valid for the whole lifetime of inspector
        unsafe { std::ptr::addr_of!((*this).base) }
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
        (self.logger)(message.to_string());
    }
}

// TryCatch からエラー情報を文字列に変換する
fn report_exceptions(try_catch: &mut v8::TryCatch<v8::HandleScope>) -> String {
    let mut description = Vec::<String>::new();
    let Some(exception) = try_catch.exception() else {
        return "no error".into();
    };
    let Some(exception_string) = exception.to_string(try_catch) else {
        return "unexpected error".into();
    };
    let exception_string = exception_string.to_rust_string_lossy(try_catch);
    let Some(message) = try_catch.message() else {
        return exception_string;
    };

    // 該当箇所の出力
    // e.g.
    //   main.js:5: SyntaxError: Unexpected token '=='
    let filename = message
        .get_script_resource_name(try_catch)
        .and_then(|s| s.to_string(try_catch))
        .map(|s| s.to_rust_string_lossy(try_catch))
        .unwrap_or("(unknown)".into());
    let line_number = message
        .get_line_number(try_catch)
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
    if let Some(source_line) = message.get_source_line(try_catch) {
        let source_line = source_line.to_rust_string_lossy(try_catch);
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
        .and_then(|s| s.to_string(try_catch))
        .map(|s| s.to_rust_string_lossy(try_catch))
    {
        description.push(format!("{}", stack_trace));
    }

    return description.join("\n");
}

struct MyStatus {
    audio: Rc<RefCell<Option<v8::Global<v8::Function>>>>,
}

impl MyStatus {
    fn audio(&self, mut info: CallbackInfo) {
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
        *self.audio.borrow_mut() = Some(callback);
        info.rv.set(v8::Number::new(info.scope, 123f64).into());
    }
}

impl Drop for MyStatus {
    fn drop(&mut self) {
        println!("MyStatus dropped");
    }
}

fn main() {
    let audio = Rc::new(RefCell::new(None));
    let data = MyStatus {
        audio: audio.clone(),
    };
    let api = Api::new().add("audio", MyStatus::audio);
    let mut app = JsRuntime::new(data);
    app.reset();
    app.set_logger(|msg| {
        println!("Console log: {}", msg);
    });
    if let Err(e) = app.add_api("ps88", &api) {
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
