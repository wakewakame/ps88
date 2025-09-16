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
        let status = self.status.borrow();
        let mut userdata = match status.userdata.lock() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw(info.scope, &format!("unexpected mutex error: {}", err));
            }
        };
        if let Ok(data) = info.args.get(0).try_cast::<v8::Uint8Array>() {
            let mut bytes = vec![0u8; data.length()];
            let n = data.copy_contents(&mut bytes[..]);
            assert_eq!(n, bytes.len());
            *userdata = UserData::Bytes(bytes);
            return;
        }
        if let Ok(data) = info.args.get(0).try_cast::<v8::String>() {
            let text = data.to_rust_string_lossy(info.scope);
            *userdata = UserData::Text(text);
            return;
        }
        return core::v8throw_type_error(
            info.scope,
            &format!("argument must be a Uint8Array or String"),
        );
    }
    pub(super) fn load(&mut self, mut info: core::CallbackInfo) {
        let status = self.status.borrow();
        let userdata = match status.userdata.lock() {
            Ok(v) => v,
            Err(err) => {
                return core::v8throw(info.scope, &format!("unexpected mutex error: {}", err));
            }
        };
        match &*userdata {
            UserData::None => {
                info.rv.set_null();
                return;
            }
            UserData::Text(text) => {
                let v8_str = match v8::String::new(info.scope, text) {
                    Some(v) => v,
                    None => {
                        return core::v8throw(info.scope, "failed to create string");
                    }
                };
                info.rv.set(v8_str.into());
                return;
            }
            UserData::Bytes(bytes) => {
                let backing_store =
                    v8::ArrayBuffer::new_backing_store_from_vec(bytes.clone()).into();
                let array_buffer = v8::ArrayBuffer::with_backing_store(info.scope, &backing_store);
                let Some(uint8_array) =
                    v8::Uint8Array::new(info.scope, array_buffer, 0, backing_store.byte_length())
                else {
                    return core::v8throw(info.scope, "failed to create Uint8Array");
                };
                info.rv.set(uint8_array.into());
                return;
            }
        };
    }
}
impl core::This<'_> for Api {
    fn reset(&mut self) {
        let mut status = self.status.borrow_mut();
        status.audio_callback.take();
        status.gui_callback.take();
    }
}
