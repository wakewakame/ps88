use super::super::core;
use super::status::*;
use deno_core::v8;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) struct Api {
    pub(crate) status: Rc<RefCell<Status>>,
}
impl Api {
    pub(crate) fn audio(&mut self, info: core::CallbackInfo) {
        let Ok(callback) = info.args.get(0).try_cast::<v8::Function>() else {
            let msg = v8::String::new(info.scope, "argument must be a function")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.audio_callback = Some(callback);
    }

    pub(crate) fn gui(&mut self, info: core::CallbackInfo) {
        let Ok(callback) = info.args.get(0).try_cast::<v8::Function>() else {
            let msg = v8::String::new(info.scope, "argument must be a function")
                .unwrap_or(v8::String::empty(info.scope));
            let err = v8::Exception::type_error(info.scope, msg);
            info.scope.throw_exception(err);
            return;
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.audio_callback = Some(callback);
    }
    pub(crate) fn save(&mut self, _info: core::CallbackInfo) {
        todo!()
    }
    pub(crate) fn load(&mut self, _info: core::CallbackInfo) {
        todo!()
    }
}
impl core::This<'_> for Api {
    fn reset(&mut self) {
        let mut status = self.status.borrow_mut();
        status.audio_callback.take();
        status.gui_callback.take();
    }
}
