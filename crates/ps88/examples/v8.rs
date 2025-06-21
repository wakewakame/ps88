use deno_core::{serde_v8, v8};

fn main() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());

    fn exec<'s>(scope: &mut v8::HandleScope<'s>, src: &str) -> v8::Local<'s, v8::Value> {
        let code = v8::String::new(scope, src).unwrap();
        let script = v8::Script::compile(scope, code, None).unwrap();
        script.run(scope).unwrap()
    }

    // ArrayBuffer -> JsBuffer
    {
        let handle_scope = &mut v8::HandleScope::new(isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let v = v8::ArrayBuffer::new(scope, 3 * size_of::<f32>());
        let v2 = v8::Float32Array::new(scope, v, 0, 3);
        let arr = serde_v8::from_v8::<serde_v8::JsBuffer>(scope, v.into()).unwrap();
        let arr2 = unsafe { arr.align_to::<f32>().1 };
        println!("v = {v2:?}");
        println!("arr = {arr2:?}");
    }

    // function -> JsBuffer
    {
        use serde::Deserialize;
        #[derive(Deserialize)]
        struct MathOp {
            pub a: serde_v8::GlobalValue,
        }

        let handle_scope = &mut v8::HandleScope::new(isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let v = exec(scope, "function f() { return 42; }; ({a: f})");
        match serde_v8::from_v8::<MathOp>(scope, v) {
            Ok(arr) => {
                let f: v8::Local<'_, v8::Value> = v8::Local::new(scope, arr.a.v8_value);
                let f = v8::Local::<v8::Function>::try_from(f).unwrap();
                println!("ok, f = {:?}", f);
            }
            Err(e) => {
                println!("Error: {e}");
            }
        }
    }

    // function -> JsBuffer
    {
        let handle_scope = &mut v8::HandleScope::new(isolate);
        let object_templ = v8::ObjectTemplate::new(handle_scope);
        object_templ.set(
            v8::String::new(handle_scope, "yo").unwrap().into(),
            v8::String::new(handle_scope, "hoge").unwrap().into(),
        );
        let context = v8::Context::new(
            handle_scope,
            v8::ContextOptions {
                global_template: Some(object_templ),
                ..Default::default()
            },
        );
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let v = exec(scope, "yo");
        match serde_v8::from_v8::<serde_v8::AnyValue>(scope, v) {
            Ok(arr) => {
                println!("ok, f = {arr:?}");
            }
            Err(e) => {
                println!("Error: {e}");
            }
        }
    }
}
