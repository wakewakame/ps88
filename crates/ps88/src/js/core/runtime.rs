use super::api::*;
use super::error::*;
use super::inspector::*;
use super::utils::*;
use deno_core::v8;
use std::sync::Once;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

// JavaScript の実行環境
pub struct JsRuntime<Status> {
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
    pub fn reset(&mut self) {
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
    pub fn add_api(&mut self, name: &str, api: &Api<Status>) -> Result<()> {
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
    pub fn run(&mut self, code: &str) -> Result<()> {
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

    pub fn scope(&mut self) -> v8::HandleScope {
        v8::HandleScope::with_context(&mut self.isolate, &self.context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MyStatus();
    impl MyStatus {
        fn add(&self, mut _info: CallbackInfo) {
            // TODO
        }
    }
    impl Drop for MyStatus {
        fn drop(&mut self) {
            // TODO
        }
    }

    #[test]
    fn run() {
        /*
        let status = MyStatus();
        let mut runtime = JsRuntime::new(status);
        let api = Api::new().add("add", MyStatus::add);
        runtime.add_api("myapi", &api).unwrap();
        runtime
            .run(r#"console.log("Hello, world!"); myapi.add(1, 2);"#)
            .unwrap();
        */
    }
}
