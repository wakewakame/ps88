use super::super::core;
use super::runtime::*;
use super::shape_api::*;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

// Runtime を複数スレッドから使えるようにするためのラッパー。
// Runtime は Sync, Send を持たないため、そのままでは複数スレッドから使うことはできない。
// これを解決するため、Runtime を専用スレッドで動かしチャンネル経由でメソッド呼び出しを行うようにする。
pub struct RuntimeActor {
    thread: Option<(std::thread::JoinHandle<()>, Sender<RuntimeActorMessage>)>,

    // メソッドの引数は通常チャンネルで渡すが、サイズの大きいデータや参照型などは args 経由でやり取りする
    args: Arc<Mutex<RuntimeActorArgs>>,
}
impl RuntimeActor {
    pub fn new<F: Fn(String) + Send + 'static>(logger: F) -> core::Result<Self> {
        let args = Arc::new(Mutex::new(RuntimeActorArgs {
            audio: Vec::new(),
            midi: Vec::new(),
            shapes: Shapes::new(),
        }));
        let args_clone = args.clone();
        let (tx, rx) = channel::<RuntimeActorMessage>();
        let join_handle = std::thread::spawn(move || {
            let mut runtime = Runtime::new(logger).unwrap();
            for msg in rx {
                match msg {
                    RuntimeActorMessage::Reset { result } => {
                        result.send(runtime.reset()).unwrap();
                    }
                    RuntimeActorMessage::Compile { code, result } => {
                        result.send(runtime.compile(&code)).unwrap();
                    }
                    RuntimeActorMessage::Audio {
                        sample_rate,
                        current_frame,
                        bpm,
                        result,
                    } => {
                        let RuntimeActorArgs { audio, midi, .. } = &mut *args_clone.lock().unwrap();
                        let mut audio: Vec<&mut [f32]> =
                            audio.iter_mut().map(|ch| ch.as_mut_slice()).collect();
                        result
                            .send(runtime.audio(
                                audio.as_mut_slice(),
                                midi,
                                sample_rate,
                                current_frame,
                                bpm,
                            ))
                            .unwrap();
                    }
                    RuntimeActorMessage::Gui { args, result } => {
                        let RuntimeActorArgs { shapes, .. } = &mut *args_clone.lock().unwrap();
                        result.send(runtime.gui(args, shapes)).unwrap();
                    }
                }
            }
        });
        Ok(Self {
            thread: Some((join_handle, tx)),
            args,
        })
    }
    pub fn reset(&self) -> core::Result<()> {
        let (tx, rx) = channel();
        let sender = &self.thread.as_ref().unwrap().1;
        sender
            .send(RuntimeActorMessage::Reset { result: tx })
            .unwrap();
        rx.recv().unwrap()
    }
    pub fn compile(&self, code: &str) -> core::Result<()> {
        let (tx, rx) = channel();
        let sender = &self.thread.as_ref().unwrap().1;
        sender
            .send(RuntimeActorMessage::Compile {
                code: code.to_string(),
                result: tx,
            })
            .unwrap();
        rx.recv().unwrap()
    }
    pub fn audio(
        &self,
        audio: &mut [&mut [f32]],
        midi: &mut Vec<[u8; 7]>,
        sample_rate: f64,
        current_frame: u64,
        bpm: f64,
    ) -> core::Result<()> {
        // audio & midi から args にコピー
        // audio は長さが頻繁に変化しないことが予想されるため、配列をあらかじめ確保しておく
        {
            let args = &mut *self.args.lock().unwrap();
            if audio.len() != args.audio.len()
                || audio
                    .iter()
                    .zip(args.audio.iter())
                    .any(|(a, b)| a.len() != b.len())
            {
                // audio の長さが変化した場合は再確保
                args.audio = audio.iter().map(|ch| ch.to_vec()).collect();
            } else {
                // 長さが同じ場合は内容だけコピー
                for (ch, buf) in audio.iter().zip(args.audio.iter_mut()) {
                    buf.copy_from_slice(ch);
                }
            }
            std::mem::swap(&mut args.midi, midi);
        }

        // メッセージを送信して処理を待つ
        let (tx, rx) = channel();
        let sender = &self.thread.as_ref().unwrap().1;
        sender
            .send(RuntimeActorMessage::Audio {
                sample_rate,
                current_frame,
                bpm,
                result: tx,
            })
            .unwrap();
        rx.recv().unwrap()?;

        // args から audio & midi にコピー
        {
            let args = &mut *self.args.lock().unwrap();
            for (ch, buf) in audio.iter_mut().zip(args.audio.iter()) {
                ch.copy_from_slice(buf);
            }
            std::mem::swap(midi, &mut args.midi);
        }

        Ok(())
    }
    pub fn gui(&self) -> core::Result<()> {
        todo!()
    }
}
impl Drop for RuntimeActor {
    fn drop(&mut self) {
        let (join_handle, sender) = self.thread.take().unwrap();
        drop(sender);
        join_handle.join().unwrap();
    }
}

struct RuntimeActorArgs {
    audio: Vec<Vec<f32>>,
    midi: Vec<[u8; 7]>,
    shapes: Shapes,
}

enum RuntimeActorMessage {
    Reset {
        result: Sender<core::Result<()>>,
    },
    Compile {
        code: String,
        result: Sender<core::Result<()>>,
    },
    Audio {
        sample_rate: f64,
        current_frame: u64,
        bpm: f64,
        result: Sender<core::Result<()>>,
    },
    Gui {
        args: GuiArgs,
        result: Sender<core::Result<()>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio() {
        let rt = RuntimeActor::new(|_| {}).unwrap();
        rt.compile(
            r#"ps88.audio((arg) => {
    if (arg.sampleRate !== 48000.0) {
        throw new Error("sampleRate must be 48000.0");
    }
    if (arg.currentFrame !== 1024) {
        throw new Error("currentFrame must be 1024");
    }
    if (arg.bpm !== 120.0) {
        throw new Error("bpm must be 120.0");
    }
    let audio = arg.audio;
    let midi = arg.midi;
    for (let ch = 0; ch < audio.length; ch++) {
        for (let i = 0; i < audio[ch].length; i++) {
            audio[ch][i] *= 2.0;
        }
    }
    for (let ev = 0; ev < midi.length; ev++) {
        midi[ev][6] += 10;
    }
    midi.push([0, 0, 0, 0, 0x80, 57, 30]);
});"#,
        )
        .unwrap();
        let mut audio = vec![vec![0.1f32, 0.2, 0.3], vec![0.4, 0.5, 0.6]];
        let mut midi = vec![[0, 0, 0, 0, 0x80, 69, 10]];
        let mut audio_slice = audio
            .iter_mut()
            .map(|ch| ch.as_mut_slice())
            .collect::<Vec<_>>();
        rt.audio(audio_slice.as_mut_slice(), &mut midi, 48000.0, 1024, 120.0)
            .unwrap();
        assert_eq!(audio, vec![vec![0.2f32, 0.4, 0.6], vec![0.8, 1.0, 1.2]]);
        assert_eq!(
            midi,
            vec![[0, 0, 0, 0, 0x80, 69, 20], [0, 0, 0, 0, 0x80, 57, 30]]
        );
    }
}
