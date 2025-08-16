/*
TODO
- unwrap を使わないようにする
- テストを書く
*/

use deno_core::v8;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Once;

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
    pub fn new(status: Status) -> Self {
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
    fn add_api(&mut self, name: &str, api: &Api<Status>) {
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let context = v8::Local::new(scope, &self.context);
        let obj_t = v8::ObjectTemplate::new(scope);
        let status = v8::External::new(
            scope,
            &mut self.status as *mut Status as *mut std::ffi::c_void,
        );
        for (name, func) in api.callbacks.iter() {
            let name = v8::String::new(scope, name).unwrap();
            let func = v8::FunctionBuilder::<v8::FunctionTemplate>::new_raw(*func)
                .data(status.into())
                .build(scope);
            obj_t.set(name.into(), func.into());
        }
        let obj = obj_t.new_instance(scope).unwrap();
        let name = v8::String::new(scope, &name).unwrap();
        context.global(scope).set(scope, name.into(), obj.into());
    }

    // スクリプトを実行
    fn run(&mut self, code: &str) {
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let code = v8::String::new(scope, code).unwrap();
        let script = v8::Script::compile(scope, code, None).unwrap();
        script.run(scope).unwrap();
    }
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

struct MyStatus {
    audio: Rc<RefCell<Option<v8::Global<v8::Function>>>>,
}

impl MyStatus {
    fn audio(&self, mut info: CallbackInfo) {
        let callback = info.args.get(0).try_cast::<v8::Function>().unwrap();
        let callback = v8::Global::new(info.scope, callback);
        *self.audio.borrow_mut() = Some(callback);
        info.rv
            .set(v8::String::new(info.scope, "ok").unwrap().into());
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
    app.add_api("ps88", &api);
    app.run("let a = 100; ps88.audio((n) => (n + a)); console.log('hogehoge');");

    {
        let scope = &mut v8::HandleScope::with_context(&mut app.isolate, &app.context);
        let audio = audio.borrow_mut();
        let callback = v8::Local::new(scope, audio.as_ref().unwrap());
        let this = v8::undefined(scope).into();
        let arg = v8::Number::new(scope, 42f64).into();
        let result = callback.call(scope, this, &[arg]).unwrap();
        println!(
            "Callback result: {:?}",
            result.try_cast::<v8::Number>().unwrap().value()
        );
    }

    drop(app);
}
