use deno_core::serde::{Deserialize, Serialize};
use deno_core::v8;
use std::sync::{Arc, Mutex};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub enum UserData {
    None,
    Bytes(Vec<u8>),
    Text(String),
}

pub(super) struct Status {
    pub(super) audio_callback: Option<v8::Global<v8::Function>>,
    pub(super) gui_callback: Option<v8::Global<v8::Function>>,
    pub(super) userdata: Arc<Mutex<UserData>>,
}
