use super::api::*;
use super::error::*;
use super::inspector::*;
use super::utils::*;
use deno_core::v8;
use std::rc::Rc;
use std::sync::Once;

pub struct JsRuntimeBuilder<'a> {
    api: Vec<Box<dyn ApiTrait<'a>>>,
    logger: Vec<Box<dyn Fn(String) + 'a>>,
}

impl<'a> JsRuntimeBuilder<'a> {
    pub fn new() -> Self {
        Self {
            api: Vec::new(),
            logger: Vec::new(),
        }
    }

    pub fn add_api<T: This<'a>>(mut self, api: Api<'a, T>) -> JsRuntimeBuilder<'a> {
        self.api.push(Box::new(api));
        self
    }

    pub fn add_logger<F: Fn(String) + 'a>(mut self, logger: F) -> JsRuntimeBuilder<'a> {
        self.logger.push(Box::new(logger));
        self
    }

    pub fn build(self) -> Result<JsRuntime<'a>> {
        JsRuntime::<'a>::new(self.api, self.logger)
    }
}

// JavaScript の実行環境
pub struct JsRuntime<'a> {
    // NOTE: inspector と context は isolate に紐づくので、これらは isolate より先に drop される必要がある
    inspector: Option<Inspector<'a>>,
    context: v8::Global<v8::Context>,

    // NOTE: isolate は間接的に api の生ポインタを参照しているため、isolate は api より先に drop される必要がある
    isolate: v8::OwnedIsolate,

    // Rust 側のコールバック関数に渡すステータス情報
    api: Vec<Box<dyn ApiTrait<'a>>>,

    // console.log() の出力を得るためのロガー
    logger: Rc<Vec<Box<dyn Fn(String) + 'a>>>,
}

impl<'a> JsRuntime<'a> {
    fn new(api: Vec<Box<dyn ApiTrait<'a>>>, logger: Vec<Box<dyn Fn(String) + 'a>>) -> Result<Self> {
        // V8 の初期化
        static PUPPY_INIT: Once = Once::new();
        PUPPY_INIT.call_once(move || {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });
        let mut isolate = v8::Isolate::new(Default::default());

        // context 作成
        let context = {
            let handle_scope = &mut v8::HandleScope::new(&mut isolate);
            let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
            v8::Global::new(handle_scope, context)
        };

        // api のコールバック関数を登録
        let api = {
            let scope = &mut v8::HandleScope::with_context(&mut isolate, &context);
            let context = v8::Local::new(scope, &context);
            for api in api.iter() {
                api.register(scope, context)?;
            }
            api
        };

        // console.log() の出力を得るためのロガーを設定
        let logger = Rc::new(logger);
        let inspector = {
            let scope = &mut v8::HandleScope::with_context(&mut isolate, &context);
            let context = v8::Local::new(scope, &context);
            let logger = Rc::clone(&logger);
            let inspector = Some(Inspector::new(scope, context, move |msg| {
                logger.iter().for_each(|f| f(msg.clone()));
            }));
            inspector
        };

        Ok(Self {
            inspector,
            context,
            isolate,
            api,
            logger,
        })
    }

    // JavaScript の実行環境をリセット
    pub fn reset<'b>(&'b mut self) -> Result<()> {
        // api の this をリセット
        self.api.iter().for_each(|api| api.reset());

        // NOTE:
        // 新しい inspector を作った後に古い inspector を drop すると
        // 古い inspector のデストラクタが新しい inspector に影響して
        // console.log の出力を得られなくなってしまう。
        // そのため、先にここで古いインスタンスを drop しておく。
        self.inspector = None;

        // context 作成
        self.context = {
            let handle_scope = &mut v8::HandleScope::new(&mut self.isolate);
            let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
            v8::Global::new(handle_scope, context)
        };
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let context = v8::Local::new(scope, &self.context);

        // v8 のグローバル変数に API を登録する
        for api in self.api.iter() {
            api.register(scope, context)?;
        }

        // console.log() の出力を得るためのロガーを設定
        let logger = Rc::clone(&self.logger);
        self.inspector = Some(Inspector::new(scope, context, move |msg| {
            logger.iter().for_each(|f| f(msg.clone()));
        }));

        Ok(())
    }

    // スクリプトを実行
    pub fn run(&mut self, code: &str) -> Result<v8::Local<v8::Value>> {
        let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let code = v8str(scope, code)?;
        let try_catch = &mut v8::TryCatch::new(scope);
        let script = v8::Script::compile(try_catch, code, None)
            .ok_or(JsRuntimeError::CompileError(report_exceptions(try_catch)))?;
        let result = script
            .run(try_catch)
            .ok_or(JsRuntimeError::RuntimeError(report_exceptions(try_catch)))?;
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
        let mut rt = JsRuntimeBuilder::new().build().unwrap();

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
        // console.log の出力を得られる
        let (tx, rx) = channel();
        let mut rt = JsRuntimeBuilder::new()
            .add_logger(move |msg| tx.send(msg).unwrap())
            .build()
            .unwrap();

        rt.run("console.log('log1');").unwrap();
        assert_eq!(rx.try_recv().unwrap(), "log1");
        rt.run("console.log('log2');").unwrap();
        assert_eq!(rx.try_recv().unwrap(), "log2");

        // reset 後も問題なく動く
        rt.reset().unwrap();
        rt.run("console.log('log3');").unwrap();
        assert_eq!(rx.try_recv().unwrap(), "log3");

        // runtime を drop すると logger も drop される
        drop(rt);
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Disconnected)));
    }

    #[test]
    fn test_add_callbacks() {
        // テスト用の足し算 API を定義
        let test_add = Api::new("test_add", ()).add("add", |_, mut info| {
            let mut result = 0f64;
            for i in 0..info.args.length() {
                let Ok(arg) = info.args.get(i).try_cast::<v8::Number>() else {
                    v8throw_type_error(info.scope, "Argument must be a number");
                    return;
                };
                result += arg.value();
            }
            info.rv.set(v8::Number::new(info.scope, result).into());
        });

        // テスト用のカウント API を定義
        struct TestCounter<'a> {
            count: f64,
            dropped: &'a mut bool,
        }
        impl<'a> TestCounter<'a> {
            fn add(&mut self, mut info: CallbackInfo) {
                let Ok(arg0) = info.args.get(0).try_cast::<v8::Number>() else {
                    return v8throw_type_error(info.scope, "Argument must be a number");
                };
                self.count += arg0.value();
                info.rv.set(v8::Number::new(info.scope, self.count).into());
            }
        }
        impl<'a> This<'a> for TestCounter<'a> {
            fn reset(&mut self) {
                self.count = 0f64;
            }
        }
        impl<'a> Drop for TestCounter<'a> {
            fn drop(&mut self) {
                *self.dropped = true;
            }
        }
        let mut dropped = false;
        let test_counter = Api::new(
            "test_counter",
            TestCounter {
                count: 0f64,
                dropped: &mut dropped,
            },
        )
        .add("add", TestCounter::add);

        // test_add, test_counter を API として登録
        let mut rt = JsRuntimeBuilder::new()
            .add_api(test_add)
            .add_api(test_counter)
            .build()
            .unwrap();

        // test_add.add を呼び出せる
        let result = rt.run("test_add.add(1, 2, 3);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 6f64);

        // test_counter.add を呼び出せる
        let result = rt.run("test_counter.add(1);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 1f64);
        let result = rt.run("test_counter.add(2);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 3f64);

        // 引数の型が違う場合は適切に例外が投げられる
        let result = rt.run("test_counter.add('a');");
        assert!(matches!(result, Err(JsRuntimeError::RuntimeError(_))));

        // reset 後も問題なく動く
        rt.reset().unwrap();
        let result = rt.run("test_add.add(4, 5, 6);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 15f64);
        let result = rt.run("test_counter.add(1);");
        let result = result.unwrap().try_cast::<v8::Number>().unwrap().value();
        assert_eq!(result, 1f64);

        // runtime を drop すると API のインスタンスも drop される
        drop(rt);
        assert!(dropped);
    }
}
