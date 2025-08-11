use deno_core::v8;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Once;

pub struct JsRuntime<Status> {
    // isolate は status を参照しているため、drop される順番は isolate -> status とする必要がある。
    // そのため、フィールドの宣言順は isolate -> status とする。
    isolate: v8::OwnedIsolate,
    status: Rc<RefCell<Status>>,
    callbacks: HashMap<String, v8::FunctionCallback>,
}

struct JsRuntimeContext(v8::Global<v8::Context>);

pub struct CallbackInfo<'a, 'b> {
    scope: &'a mut v8::HandleScope<'b>,
    args: v8::FunctionCallbackArguments<'a>,
    rv: v8::ReturnValue<'a>,
}

impl<Status> JsRuntime<Status> {
    pub fn new(status: Rc<RefCell<Status>>) -> Self {
        static PUPPY_INIT: Once = Once::new();
        PUPPY_INIT.call_once(move || {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });
        let isolate = v8::Isolate::new(Default::default());
        let callbacks = HashMap::new();
        Self {
            status,
            isolate,
            callbacks,
        }
    }

    fn reset(&mut self) {
        self.isolate.remove_slot::<JsRuntimeContext>();
    }

    fn add_func<F: Fn(&mut Status, CallbackInfo) + Sized>(&mut self, name: &str, _: F) {
        const {
            assert!(
                size_of::<F>() == 0,
                "the provided closure must not capture any variables"
            )
        }
        // Rust の関数やクロージャを v8::FunctionCallback としてラップする
        unsafe extern "C" fn f<Status, F: Fn(&mut Status, CallbackInfo) + Sized>(
            info: *const v8::FunctionCallbackInfo,
        ) {
            let info = unsafe { &*info };
            let scope = &mut unsafe { v8::CallbackScope::new(info) };
            let args = v8::FunctionCallbackArguments::from_function_callback_info(info);
            let rv = v8::ReturnValue::from_function_callback_info(info);
            let info = CallbackInfo { scope, args, rv };
            let status = info.args.data().cast::<v8::External>();
            let status = unsafe { &*status.value().cast::<RefCell<Status>>() };
            let f = unsafe { std::mem::zeroed::<F>() };
            f(&mut *status.borrow_mut(), info);
        }
        self.callbacks.insert(name.to_string(), f::<Status, F>);
    }

    fn compile(&mut self, code: &str) {
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
            let status =
                v8::External::new(scope, Rc::as_ptr(&self.status) as *mut std::ffi::c_void);
            for (name, func) in self.callbacks.iter() {
                let name = v8::String::new(scope, name).unwrap();
                let func = v8::FunctionBuilder::<v8::FunctionTemplate>::new_raw(*func)
                    .data(status.into())
                    .build(scope);
                obj_t.set(name.into(), func.into());
            }
            let obj = obj_t.new_instance(scope).unwrap();
            let key = v8::String::new(scope, "ps88").unwrap().into();
            context.global(scope).set(scope, key, obj.into());
        }
        {
            let scope = &mut v8::HandleScope::with_context(&mut self.isolate, &context);
            let code = v8::String::new(scope, code).unwrap();
            let script = v8::Script::compile(scope, code, None).unwrap();
            script.run(scope).unwrap();
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

struct MyStatus {
    audio: Option<v8::Global<v8::Function>>,
}

impl MyStatus {
    fn audio(&mut self, mut info: CallbackInfo) {
        let callback = info.args.get(0).cast::<v8::Function>();
        let callback = v8::Global::new(info.scope, callback);
        self.audio = Some(callback);
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
    let data = Rc::new(RefCell::new(MyStatus { audio: None }));
    let mut app = JsRuntime::new(data.clone());
    app.add_func("audio", MyStatus::audio);
    app.compile("ps88.audio((n) => (n + 100))");

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
