use super::super::core;
use super::status::*;
use deno_core::v8;
use std::cell::RefCell;
use std::rc::Rc;

pub(super) struct Api {
    pub(super) status: Rc<RefCell<Status>>,
}
impl Api {
    pub(super) fn audio(&mut self, info: core::CallbackInfo) {
        let Ok(callback) = info.args.get(0).try_cast::<v8::Function>() else {
            return core::v8throw_type_error(info.scope, "argument must be a function");
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.audio_callback = Some(callback);
    }

    pub(super) fn gui(&mut self, info: core::CallbackInfo) {
        let Ok(callback) = info.args.get(0).try_cast::<v8::Function>() else {
            return core::v8throw_type_error(info.scope, "argument must be a function");
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.gui_callback = Some(callback);
    }
    pub(super) fn save(&mut self, _info: core::CallbackInfo) {
        todo!()
    }
    pub(super) fn load(&mut self, _info: core::CallbackInfo) {
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
