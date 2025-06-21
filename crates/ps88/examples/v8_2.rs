use deno_core::v8;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Once;

pub struct JsRuntime<Api> {
    // isolate は api を参照しているため、drop される順番は isolate -> api とする必要がある。
    // そのため、フィールドの宣言順は isolate -> api とする。
    isolate: v8::OwnedIsolate,
    api: Rc<Api>,
}

#[derive(Clone)]
struct JsRuntimeContext(v8::Global<v8::Context>);

impl<Api> JsRuntime<Api> {
    pub fn new(api: Rc<Api>) -> Self {
        static PUPPY_INIT: Once = Once::new();
        PUPPY_INIT.call_once(move || {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });
        let isolate = v8::Isolate::new(Default::default());
        Self { api, isolate }
    }

    fn reset(&mut self) {
        self.isolate.remove_slot::<JsRuntimeContext>();
    }

    fn compile(
        &mut self,
        code: &str,
        api_functions: impl Fn(&mut v8::HandleScope, v8::Local<v8::ObjectTemplate>),
    ) {
        self.reset();
        let context = {
            let handle_scope = &mut v8::HandleScope::new(&mut self.isolate);
            let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
            v8::Global::new(handle_scope, context)
        };
        {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let context = v8::Local::new(scope, &context);
            let obj_t = v8::ObjectTemplate::new(scope);
            api_functions(scope, obj_t);
            obj_t.set_internal_field_count(1);
            let obj = obj_t.new_instance(scope).unwrap();
            obj.set_internal_field(
                0,
                v8::External::new(scope, Rc::as_ptr(&self.api) as *mut std::ffi::c_void).into(),
            );
            let key = v8::String::new(scope, "ps88").unwrap().into();
            context.global(scope).set(scope, key, obj.into());
        }
        {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let code = v8::String::new(scope, code).unwrap();
            let script = v8::Script::compile(scope, code, None).unwrap();
            let result = script.run(scope).unwrap();
            println!(
                "result: {:?}",
                result.cast::<v8::String>().to_rust_string_lossy(scope)
            );
        }
        self.isolate.set_slot(JsRuntimeContext(context));
    }

    fn context(&self) -> v8::Global<v8::Context> {
        self.isolate
            .get_slot::<JsRuntimeContext>()
            .unwrap()
            .0
            .clone()
    }
}

struct UserData {
    audio: Option<v8::Global<v8::Function>>,
}

impl Drop for UserData {
    fn drop(&mut self) {
        println!("UserData dropped");
    }
}

fn main() {
    let data = Rc::new(RefCell::new(UserData { audio: None }));
    let mut app = JsRuntime::new(data.clone());
    app.compile("ps88.audio((n) => (n + 100))", |scope, obj_t| {
        let func = |handle_scope: &mut v8::HandleScope,
                    args: v8::FunctionCallbackArguments,
                    mut rv: v8::ReturnValue| {
            let data = args
                .this()
                .get_internal_field(handle_scope, 0)
                .unwrap()
                .cast::<v8::External>();
            let data = unsafe {
                let data = data.value().cast::<RefCell<UserData>>();
                &mut *data
            };
            let callback = args.get(0).cast::<v8::Function>();
            let callback = v8::Global::new(handle_scope, callback);
            data.borrow_mut().audio = Some(callback);
            rv.set(v8::String::new(handle_scope, "ok").unwrap().into());
        };
        let func = v8::FunctionBuilder::<v8::FunctionTemplate>::new(func).build(scope);
        obj_t.set(v8::String::new(scope, "audio").unwrap().into(), func.into());
    });

    {
        let data = data.borrow_mut();
        let context = app.context();
        let scope = &mut v8::HandleScope::with_context(&mut app.isolate, &context);
        let callback = data.audio.as_ref().unwrap();
        let callback = v8::Local::new(scope, callback);
        let this = v8::undefined(scope).into();
        let arg = v8::Number::new(scope, 42f64).into();
        let result = callback.call(scope, this, &[arg]).unwrap();
        println!("Callback result: {:?}", result.cast::<v8::Number>().value());
    }
    drop(app);
}
