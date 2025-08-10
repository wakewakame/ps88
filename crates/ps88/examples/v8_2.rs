use deno_core::v8;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Once;

pub trait Api {
    fn register(&self) -> HashMap<String, fn(&mut Self, &mut ApiArgs)>;
}

// TODO: lifetime の指定がよくわかっていないので、あとで綺麗にしたい
pub struct ApiArgs<'a, 'b, 'c, 'd, 'e, 'f> {
    handle_scope: &'a mut v8::HandleScope<'b>,
    args: &'c v8::FunctionCallbackArguments<'d>,
    rv: &'e mut v8::ReturnValue<'f>,
}

pub struct JsRuntime<Api> {
    // isolate は api を参照しているため、drop される順番は isolate -> api とする必要がある。
    // そのため、フィールドの宣言順は isolate -> api とする。
    isolate: v8::OwnedIsolate,
    api: Rc<RefCell<Api>>,
}

#[derive(Clone)]
struct JsRuntimeContext(v8::Global<v8::Context>);

impl<T: Api> JsRuntime<T> {
    pub fn new(api: Rc<RefCell<T>>) -> Self {
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

            // === WIP ===
            let funcs = self.api.borrow().register();
            for (name, _func) in funcs {
                let func = v8::FunctionBuilder::<v8::FunctionTemplate>::new(
                    |handle_scope: &mut v8::HandleScope,
                     args: v8::FunctionCallbackArguments,
                     mut rv: v8::ReturnValue| {
                        let name = args
                            .data()
                            .to_string(handle_scope)
                            .unwrap()
                            .to_rust_string_lossy(handle_scope);
                        let api = args
                            .this()
                            .get_internal_field(handle_scope, 0)
                            .unwrap()
                            .cast::<v8::External>();
                        let api = unsafe {
                            let data = api.value().cast::<RefCell<T>>();
                            &mut *data
                        };
                        let funcs = api.borrow_mut().register();
                        let func = funcs.get(&name).unwrap();
                        let mut args = ApiArgs {
                            handle_scope,
                            args: &args,
                            rv: &mut rv,
                        };
                        func(&mut *api.borrow_mut(), &mut args);
                    },
                )
                .data(v8::String::new(scope, &name).unwrap().into())
                .build(scope);
                obj_t.set(v8::String::new(scope, &name).unwrap().into(), func.into());
            }
            // === WIP ===

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

struct MyApi {
    audio: Option<v8::Global<v8::Function>>,
}

impl MyApi {
    fn audio(&mut self, args: &mut ApiArgs) {
        let callback = args.args.get(0).cast::<v8::Function>();
        let callback = v8::Global::new(args.handle_scope, callback);
        self.audio = Some(callback);
        args.rv
            .set(v8::String::new(args.handle_scope, "ok").unwrap().into());
    }
}

impl Api for MyApi {
    fn register(&self) -> HashMap<String, fn(&mut Self, &mut ApiArgs)> {
        let mut map: HashMap<String, fn(&mut Self, &mut ApiArgs)> = HashMap::new();
        map.insert("audio".to_string(), Self::audio);
        map
    }
}

impl Drop for MyApi {
    fn drop(&mut self) {
        println!("myapi dropped");
    }
}

fn main() {
    let data = Rc::new(RefCell::new(MyApi { audio: None }));
    let mut app = JsRuntime::new(data.clone());
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
