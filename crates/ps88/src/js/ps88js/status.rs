use deno_core::v8;

pub(super) struct Status {
    pub(super) audio_callback: Option<v8::Global<v8::Function>>,
    pub(super) gui_callback: Option<v8::Global<v8::Function>>,
}
