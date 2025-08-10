use deno_core::v8;
use std::cell::RefCell;
use std::rc::Rc;

pub struct JsRuntime<Api> {
    api: std::marker::PhantomData<Api>,
}

impl<Api> JsRuntime<Api> {
    pub fn new(_api: Rc<Api>) -> Self {
        Self {
            api: std::marker::PhantomData,
        }
    }
    fn add_func(
        &mut self,
        _name: &str,
        _callback: impl FnOnce(
            &mut v8::HandleScope,
            v8::FunctionCallbackArguments,
            v8::ReturnValue,
            &mut UserData,
        ),
    ) {
        todo!();
    }
    fn compile(&mut self, _code: &str) {
        todo!();
    }
}

struct UserData {
    audio: i32,
}

impl UserData {
    fn tmp(&self) {
        todo!();
    }
}

fn main() {
    let data = Rc::new(RefCell::new(UserData { audio: 123 }));
    let mut app = JsRuntime::new(data.clone());
    app.add_func(
        "hoge",
        |_handle_scope: &mut v8::HandleScope,
         _args: v8::FunctionCallbackArguments,
         mut _rv: v8::ReturnValue,
         _user_data: &mut UserData| {},
    );
    app.compile("ps88.audio((n) => (n + 100))");
}
