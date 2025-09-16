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
        let callback = match info.args.get(0).try_cast::<v8::Function>() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw_type_error(
                    info.scope,
                    &format!("argument must be a function: {}", err),
                );
            }
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.audio_callback = Some(callback);
    }

    pub(super) fn gui(&mut self, info: core::CallbackInfo) {
        let callback = match info.args.get(0).try_cast::<v8::Function>() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw_type_error(
                    info.scope,
                    &format!("argument must be a function: {}", err),
                );
            }
        };
        let callback = v8::Global::new(info.scope, callback);
        let mut status = self.status.borrow_mut();
        status.gui_callback = Some(callback);
    }
    pub(super) fn save(&mut self, info: core::CallbackInfo) {
        let data = match info.args.get(0).try_cast::<v8::Uint8Array>() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw_type_error(
                    info.scope,
                    &format!("argument must be a Uint8Array: {}", err),
                );
            }
        };
        let status = self.status.borrow();
        let mut userdata = match status.userdata.lock() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw_type_error(
                    info.scope,
                    &format!("unexpected mutex error: {}", err),
                );
            }
        };
        userdata.resize(data.length(), 0);
        let n = data.copy_contents(&mut userdata);
        assert_eq!(n, userdata.len());
    }
    pub(super) fn load(&mut self, mut info: core::CallbackInfo) {
        let status = self.status.borrow();
        let userdata = match status.userdata.lock() {
            Ok(v) => v.clone(),
            Err(err) => {
                return core::v8throw_type_error(
                    info.scope,
                    &format!("unexpected mutex error: {}", err),
                );
            }
        };
        let backing_store = v8::ArrayBuffer::new_backing_store_from_vec(userdata).into();
        let array_buffer = v8::ArrayBuffer::with_backing_store(info.scope, &backing_store);
        let uint8_array =
            v8::Uint8Array::new(info.scope, array_buffer, 0, backing_store.byte_length()).unwrap();
        info.rv.set(uint8_array.into());
    }
}
impl core::This<'_> for Api {
    fn reset(&mut self) {
        let mut status = self.status.borrow_mut();
        status.audio_callback.take();
        status.gui_callback.take();
    }
}
