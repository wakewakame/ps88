use deno_core::v8;
use std::sync::{Arc, Mutex};

pub(super) struct Status {
    pub(super) audio_callback: Option<v8::Global<v8::Function>>,
    pub(super) gui_callback: Option<v8::Global<v8::Function>>,
    pub(super) userdata: Arc<Mutex<Vec<u8>>>,
}
