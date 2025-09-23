use deno_core::serde::{Deserialize, Serialize};
use deno_core::v8;
use std::sync::{Arc, Mutex};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub enum UserData {
    None,
    Bytes(Vec<u8>),
    Text(String),
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(tag = "type")]
pub enum NoteEvent {
    #[serde(rename_all = "camelCase")]
    NoteOn {
        timing: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        velocity: f32,
    },
    #[serde(rename_all = "camelCase")]
    NoteOff {
        timing: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        velocity: f32,
    },
}

pub(super) struct Status {
    pub(super) audio_callback: Option<v8::Global<v8::Function>>,
    pub(super) gui_callback: Option<v8::Global<v8::Function>>,
    pub(super) userdata: Arc<Mutex<UserData>>,
}
