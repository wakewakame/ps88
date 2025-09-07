use super::api::*;
use super::error::*;
use super::inspector::*;
use super::utils::*;
use deno_core::v8;
use std::cell::RefCell;
use std::sync::Once;

// JavaScript の実行環境
pub struct JsRuntime<A: Api> {
    // NOTE: _inspector と context は isolate に紐づくので、これらは isolate より先に drop される必要がある
    _inspector: Option<Inspector>,
    context: v8::Global<v8::Context>,

    // NOTE: isolate は間接的に api の生ポインタを参照しているため、isolate は aip より先に drop される必要がある
    isolate: v8::OwnedIsolate,

    // Rust 側のコールバック関数に渡すステータス情報
    api: RefCell<A>,
}

// TODO:
// 設計をリファクタリングしたい
// - reset しても logger や api の設定がリセットされないようにしたい
// - set_logger() や add_callbacks() は削除して、JsRuntime::new() の引数で logger や api を渡せるようにしたい
// - JsRuntimeBuilder のようなビルダーパターンを導入して、logger や api を設定できるようにしたい
impl<A: Api> JsRuntime<A> {
    pub fn new(api: A) -> Self {
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
            api: RefCell::new(api),
        }
    }

    // JavaScript の実行環境をリセット
    // set_logger() や add_api() の設定もリセットされる。
    pub fn reset(&mut self) {
        self.api.borrow_mut().reset();
        self._inspector = None;
        self.context = {
            let handle_scope = &mut v8::HandleScope::new(&mut self.isolate);
            let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
            v8::Global::new(handle_scope, context)
        };
    }

    // console.log() の出力を得るためのロガーを設定
    pub fn set_logger<F: Fn(String) + 'static>(&mut self, logger: F) {
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
    pub fn add_callbacks(&mut self, name: &str, callbacks: &Callbacks<A>) -> Result<()> {
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let context = v8::Local::new(scope, &self.context);
        let obj_t = v8::ObjectTemplate::new(scope);
        let api = v8::External::new(
            scope,
            &mut self.api as *mut RefCell<A> as *mut std::ffi::c_void,
        );
        for (name, func) in callbacks.callbacks.iter() {
            let name = v8str(scope, name)?;
            let func = v8::FunctionBuilder::<v8::FunctionTemplate>::new_raw(*func)
                .data(api.into())
                .build(scope);
            obj_t.set(name.into(), func.into());
        }
        let Some(obj) = obj_t.new_instance(scope) else {
            return Err(JsRuntimeError::UnexpectedError(
                "failed to create api object".to_string(),
            ));
        };
        let name = v8str(scope, name)?;
        context.global(scope).set(scope, name.into(), obj.into());
        Ok(())
    }

    // スクリプトを実行
    pub fn run(&mut self, code: &str) -> Result<v8::Local<v8::Value>> {
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let code = v8str(scope, code)?;
        let try_catch = &mut v8::TryCatch::new(scope);
        let Some(script) = v8::Script::compile(try_catch, code, None) else {
            return Err(JsRuntimeError::CompileError(report_exceptions(try_catch)));
        };
        let Some(result) = script.run(try_catch) else {
            return Err(JsRuntimeError::RuntimeError(report_exceptions(try_catch)));
        };
        Ok(result)
    }

    pub fn scope(&mut self) -> v8::HandleScope {
        v8::HandleScope::with_context(&mut self.isolate, &self.context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        let mut rt = JsRuntime::new(());

        // 1 + 2 を実行して 3 が返る
        let result = rt.run("1 + 2");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 3f64);

        // 構文エラーが適切に報告される
        let result = rt.run("1 + ");
        assert!(matches!(result, Err(JsRuntimeError::CompileError(_))));

        // 実行時エラーが適切に報告される
        let result = rt.run("throw new Error('test error')");
        assert!(matches!(result, Err(JsRuntimeError::RuntimeError(_))));
    }

    #[test]
    fn test_set_logger() {
        use std::sync::mpsc::*;
        let mut rt = JsRuntime::new(());

        // set_logger で console.log の出力を得られる
        let (tx1, rx1) = channel();
        rt.set_logger(move |msg| tx1.send(msg).unwrap());
        rt.run("console.log('log1');").unwrap();
        assert_eq!(rx1.try_recv().unwrap(), "log1");
        rt.run("console.log('log2');").unwrap();
        assert_eq!(rx1.try_recv().unwrap(), "log2");

        // set_logger を再度呼ぶと新しい logger に切り替わる
        let (tx2, rx2) = channel();
        rt.set_logger(move |msg| tx2.send(msg).unwrap());
        // 古い logger は drop される
        assert!(matches!(rx1.try_recv(), Err(TryRecvError::Disconnected)));
        rt.run("console.log('log3');").unwrap();
        assert_eq!(rx2.try_recv().unwrap(), "log3");

        // reset すると logger は drop される
        rt.reset();
        assert!(matches!(rx2.try_recv(), Err(TryRecvError::Disconnected)));

        // reset 後も set_logger は問題なく動く
        let (tx3, rx3) = channel();
        rt.set_logger(move |msg| tx3.send(msg).unwrap());
        rt.run("console.log('log4');").unwrap();
        assert_eq!(rx3.try_recv().unwrap(), "log4");

        // runtime を drop すると logger も drop される
        drop(rt);
        assert!(matches!(rx3.try_recv(), Err(TryRecvError::Disconnected)));
    }

    #[test]
    fn test_add_callbacks() {
        // テスト用のカウント API を定義
        struct Counter<'a> {
            count: f64,
            dropped: &'a mut bool,
        }
        impl<'a> Counter<'a> {
            fn add(&mut self, mut info: CallbackInfo) {
                let Ok(arg0) = info.args.get(0).try_cast::<v8::Number>() else {
                    let msg = v8::String::empty(info.scope);
                    let err = v8::Exception::type_error(info.scope, msg);
                    info.scope.throw_exception(err);
                    return;
                };
                self.count += arg0.value();
                info.rv.set(v8::Number::new(info.scope, self.count).into());
            }
        }
        impl<'a> Api for Counter<'a> {
            fn reset(&mut self) {
                self.count = 0f64;
            }
        }
        impl<'a> Drop for Counter<'a> {
            fn drop(&mut self) {
                *self.dropped = true;
            }
        }

        // Counter を API として登録
        let mut dropped = false;
        let counter = Counter {
            count: 0f64,
            dropped: &mut dropped,
        };
        let mut rt = JsRuntime::new(counter);
        let callbacks = Callbacks::new().add("add", Counter::add);
        rt.add_callbacks("counter", &callbacks).unwrap();

        // counter.add を呼び出せる
        let result = rt.run("counter.add(1);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 1f64);
        let result = rt.run("counter.add(2);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 3f64);

        // 引数の型が違う場合は適切に例外が投げられる
        let result = rt.run("counter.add('a');");
        assert!(matches!(result, Err(JsRuntimeError::RuntimeError(_))));

        // reset すると counter は呼び出せなくなる
        rt.reset();
        let result = rt.run("counter.add(1);");
        assert!(matches!(result, Err(JsRuntimeError::RuntimeError(_))));

        // reset 後も add_callbacks() は問題なく動く
        rt.add_callbacks("counter", &callbacks).unwrap();
        let result = rt.run("counter.add(1);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 1f64);

        // runtime を drop すると Counter も drop される
        drop(rt);
        assert!(dropped);
    }
}
