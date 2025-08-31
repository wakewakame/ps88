use deno_core::v8;
use std::collections::HashMap;

// JavaScript 側から呼び出される関数を登録するための構造体
pub struct Api<Status> {
    // JavaScript 側から呼び出される関数
    // 安全性: 呼び出し側は以下の 2 点を保証する必要がある
    // 1. 引数 `info: *const FunctionCallbackInfo` には有効なポインタを渡す
    // 2. 引数 `info.args.data()` には `&Status` を `v8::External` で囲った値が返るようにする
    pub(super) callbacks: HashMap<String, v8::FunctionCallback>,

    // Status 型を保持するためのフィールド
    // Status 型を保持する理由は `add()` で間違った型の関数が登録されることを防ぐため。
    _phantom: std::marker::PhantomData<Status>,
}

impl<Status> Api<Status> {
    pub fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    // JavaScript 側から呼び出される関数を登録する
    pub fn add<F: Fn(&Status, CallbackInfo) + Sized>(mut self, name: &str, _: F) -> Self {
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

pub struct CallbackInfo<'a, 'b> {
    pub scope: &'a mut v8::HandleScope<'b>,
    pub args: v8::FunctionCallbackArguments<'a>,
    pub rv: v8::ReturnValue<'a>,
}
