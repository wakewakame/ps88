/*
TODO
- unwrap を使わないようにする
- テストを書く
- console.log を使えるようにする
*/

use deno_core::v8;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Once;

// JavaScript の実行環境
struct JsRuntime<Status> {
    // NOTE: context は isolate に紐づくので、context は isolate より先に drop される必要がある
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
            context,
            isolate,
            status,
        }
    }

    // JavaScript の実行環境をリセット
    fn reset(&mut self) {
        //self.isolate
        //    .memory_pressure_notification(v8::MemoryPressureLevel::Critical);
        self.context = {
            let handle_scope = &mut v8::HandleScope::new(&mut self.isolate);
            let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
            v8::Global::new(handle_scope, context)
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
    app.add_api("ps88", &api);
    app.run("let a = 100; ps88.audio((n) => (n + a));");

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
