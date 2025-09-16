use super::super::core;
use deno_core::{serde_v8, v8};

pub(super) struct Dict<'a, 'b> {
    scope: &'a mut v8::HandleScope<'b>,
    obj: v8::Local<'b, v8::Object>,
}
impl<'a, 'b> Dict<'a, 'b> {
    pub(super) fn new(scope: &'a mut v8::HandleScope<'b>) -> Self {
        let obj = v8::Object::new(scope);
        Self { scope, obj }
    }
    pub(super) fn add(self, key: &str, value: v8::Local<'b, v8::Value>) -> core::Result<Self> {
        let key_js = core::v8str(self.scope, key)?;
        self.obj.set(self.scope, key_js.into(), value);
        Ok(self)
    }
    pub(super) fn add_number(self, key: &str, value: f64) -> core::Result<Self> {
        let key_js = core::v8str(self.scope, key)?;
        let value_js = v8::Number::new(self.scope, value);
        self.obj.set(self.scope, key_js.into(), value_js.into());
        Ok(self)
    }
    pub(super) fn add_bool(self, key: &str, value: bool) -> core::Result<Self> {
        let key_js = core::v8str(self.scope, key)?;
        let value_js = v8::Boolean::new(self.scope, value);
        self.obj.set(self.scope, key_js.into(), value_js.into());
        Ok(self)
    }
    pub(super) fn value(self) -> v8::Local<'b, v8::Value> {
        self.obj.into()
    }
}

pub(super) fn midi_to_arr<'a, 'b>(
    scope: &'a mut v8::HandleScope<'b>,
    midi: &'a mut Vec<[u8; 7]>,
) -> core::Result<v8::Local<'b, v8::Value>> {
    core::wrap_err(serde_v8::to_v8(scope, midi.clone()))
}

pub(super) fn arr_to_midi(
    scope: &mut v8::HandleScope,
    arr: v8::Local<v8::Value>,
) -> core::Result<Vec<[u8; 7]>> {
    match serde_v8::from_v8::<Vec<[u8; 7]>>(scope, arr) {
        Ok(midi) => Ok(midi),
        Err(e) => Err(core::JsRuntimeError::UnexpectedError(format!(
            "failed to convert midi from v8: {}",
            e
        ))),
    }
}

pub(super) fn audio_to_backing_store<'a, 'b>(
    scope: &'a mut v8::HandleScope<'b>,
    src: &'a mut [&mut [f32]], // src[ch][sample]
    dst: &'a mut Option<v8::SharedRef<v8::BackingStore>>,
) -> core::Result<v8::Local<'b, v8::Value>> {
    let bytes_per_sample: usize = std::mem::size_of::<f32>();

    // 入力のサイズが変わった場合は backing_store を再確保
    let input_size = src.iter().map(|ch| ch.len()).sum::<usize>() * bytes_per_sample;
    let alloc_size = dst.as_ref().map(|b| b.byte_length());
    if Some(input_size) != alloc_size {
        // NOTE: ArrayBuffer::new() で生成されるメモリはどうやら 16 byte 境界にアラインされるらしいので多分安全...? (ちゃんと確認していない)
        // BackingStore のメモリを自分で生成する方法もあるが、安全なのかよくわかっていないので今回はやめておく。
        let array_buffer = v8::ArrayBuffer::new(scope, input_size);
        *dst = Some(array_buffer.get_backing_store());
    }
    let backing_store = dst.as_ref().unwrap();
    let array_buffer = v8::ArrayBuffer::with_backing_store(scope, backing_store);

    // src を Vec<v8::Float32Array> に変換
    let mut float32arrays = Vec::with_capacity(src.len());
    let mut offset = 0usize;
    for ch in src.iter() {
        if let Some(pointer) = backing_store.data() {
            unsafe {
                std::ptr::copy(
                    ch.as_ptr(),
                    pointer.cast::<f32>().add(offset).as_ptr(),
                    ch.len(),
                );
            }
        } else if input_size > 0 {
            return Err(core::JsRuntimeError::UnexpectedError(
                "failed to get backing store data pointer".to_string(),
            ));
        }
        let Some(float32array) =
            v8::Float32Array::new(scope, array_buffer, offset * bytes_per_sample, ch.len())
        else {
            return Err(core::JsRuntimeError::UnexpectedError(
                "failed to create Float32Array".to_string(),
            ));
        };
        float32arrays.push(float32array);
        offset += ch.len();
    }

    // Vec<v8::Float32Array> を v8::Array<v8::Float32Array> の配列に変換
    let audio_js = v8::Array::new_with_elements(
        scope,
        float32arrays
            .clone()
            .into_iter()
            .map(|f32a| f32a.into())
            .collect::<Vec<v8::Local<v8::Value>>>()
            .as_slice(),
    );

    Ok(audio_js.into())
}

pub(super) fn backing_store_to_audio(
    src: &Option<v8::SharedRef<v8::BackingStore>>,
    dst: &mut [&mut [f32]], // audio[ch][sample]
) {
    let Some(backing_store) = src.as_ref() else {
        return;
    };
    let Some(pointer) = backing_store.data() else {
        return;
    };
    let mut offset = 0usize;
    for ch in dst.iter_mut() {
        unsafe {
            std::ptr::copy(
                pointer.cast::<f32>().add(offset).as_ptr(),
                ch.as_mut_ptr(),
                ch.len(),
            );
        }
        offset += ch.len();
    }
}
