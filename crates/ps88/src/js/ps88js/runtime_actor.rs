use super::super::core;
use super::gui_api::*;
use super::runtime::*;
use super::status::*;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

// Runtime を複数スレッドから使えるようにするためのラッパー。
// Runtime は Sync, Send を持たないため、そのままでは複数スレッドから使うことはできない。
// これを解決するため、Runtime を専用スレッドで動かしチャンネル経由でメソッド呼び出しを行うようにする。
pub struct RuntimeActor {
    handle: std::thread::JoinHandle<()>,
    sender: Sender<RuntimeActorMessage>,

    // メソッドの引数は通常チャンネルで渡すが、サイズの大きいデータや参照型などは args 経由でやり取りする
    args: Arc<Mutex<RuntimeActorArgs>>,
}
impl RuntimeActor {
    pub fn new(userdata: Arc<Mutex<UserData>>) -> core::Result<Self> {
        let args = Arc::new(Mutex::new(RuntimeActorArgs {
            audio: Vec::new(),
            midi: Vec::new(),
        }));
        let args_clone = args.clone();
        let (tx, rx) = channel::<RuntimeActorMessage>();
        let handle = std::thread::spawn(move || {
            let mut runtime = Runtime::new(userdata).unwrap();
            for msg in rx {
                match msg {
                    RuntimeActorMessage::AddLogger { logger, result } => {
                        result.send(runtime.add_logger(logger)).unwrap();
                    }
                    RuntimeActorMessage::Reset { result } => {
                        result.send(runtime.reset()).unwrap();
                    }
                    RuntimeActorMessage::Compile { code, result } => {
                        result.send(runtime.compile(&code)).unwrap();
                    }
                    RuntimeActorMessage::Audio {
                        sample_rate,
                        pos_samples,
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
                                pos_samples,
                                bpm,
                            ))
                            .unwrap();
                    }
                    RuntimeActorMessage::Gui { args, result } => {
                        result.send(runtime.gui(args)).unwrap();
                    }
                }
            }
        });
        Ok(Self {
            handle,
            sender: tx,
            args,
        })
    }
    // ログ関数を追加する。ログ関数が false を返すとログの受信が終了する。
    pub fn add_logger(&self, logger: Box<dyn Fn(String) -> bool + Sync + Send>) {
        let (tx, rx) = channel();
        self.sender
            .send(RuntimeActorMessage::AddLogger {
                logger: logger,
                result: tx,
            })
            .unwrap();
        rx.recv().unwrap()
    }
    pub fn reset(&self) -> core::Result<()> {
        let (tx, rx) = channel();
        self.sender
            .send(RuntimeActorMessage::Reset { result: tx })
            .unwrap();
        rx.recv().unwrap()
    }
    pub fn compile(&self, code: &str) -> core::Result<()> {
        let (tx, rx) = channel();
        self.sender
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
        pos_samples: u64,
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
        self.sender
            .send(RuntimeActorMessage::Audio {
                sample_rate,
                pos_samples,
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
    pub fn gui(&self, args: GuiArgs) -> core::Result<Vec<Shape>> {
        let (tx, rx) = channel();
        self.sender
            .send(RuntimeActorMessage::Gui { args, result: tx })
            .unwrap();
        rx.recv().unwrap()
    }
}
impl Drop for RuntimeActor {
    fn drop(&mut self) {
        let sender = std::mem::replace(&mut self.sender, std::sync::mpsc::channel().0);
        drop(sender);
        let handle = std::mem::replace(&mut self.handle, std::thread::spawn(move || {}));
        handle.join().unwrap();
    }
}

struct RuntimeActorArgs {
    audio: Vec<Vec<f32>>,
    midi: Vec<[u8; 7]>,
}

enum RuntimeActorMessage {
    AddLogger {
        logger: Box<dyn Fn(String) -> bool + Sync + Send>,
        result: Sender<()>,
    },
    Reset {
        result: Sender<core::Result<()>>,
    },
    Compile {
        code: String,
        result: Sender<core::Result<()>>,
    },
    Audio {
        sample_rate: f64,
        pos_samples: u64,
        bpm: f64,
        result: Sender<core::Result<()>>,
    },
    Gui {
        args: GuiArgs,
        result: Sender<core::Result<Vec<Shape>>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_logger() {
        use std::sync::mpsc::*;
        let userdata = Arc::new(Mutex::new(UserData::None));
        let rt = RuntimeActor::new(userdata).unwrap();

        // ログが受信できる
        let (tx1, rx1) = channel();
        rt.add_logger(Box::new(move |msg: String| {
            tx1.send(msg).unwrap();
            false // 1 回だけ受信して終了
        }));
        let (tx2, rx2) = channel();
        rt.add_logger(Box::new(move |msg: String| {
            tx2.send(msg).unwrap();
            true // 何回でも受信する
        }));
        rt.compile("console.log('1');").unwrap();
        assert_eq!(rx1.try_recv().unwrap(), "1");
        assert_eq!(rx2.try_recv().unwrap(), "1");
        rt.compile("console.log('2');").unwrap();
        assert!(matches!(rx1.try_recv(), Err(TryRecvError::Disconnected)));
        assert_eq!(rx2.try_recv().unwrap(), "2");
        rt.compile("console.log('3');").unwrap();
        assert_eq!(rx2.try_recv().unwrap(), "3");
    }

    #[test]
    fn test_audio() {
        let userdata = Arc::new(Mutex::new(UserData::None));
        let rt = RuntimeActor::new(userdata).unwrap();

        // 入出力の確認
        rt.compile(
            r#"
                "use strict";
                ps88.audio((ctx) => {
                    if (ctx.sampleRate !== 48000) { throw new Error(`sampleRate: ${ctx.sampleRate}`); }
                    if (ctx.posSamples !== 1024) { throw new Error(`posSamples: ${ctx.posSamples}`); }
                    if (ctx.bpm !== 120) { throw new Error(`bpm: ${ctx.bpm}`); }
                    let audio = JSON.stringify(ctx.audio.map(ch => [...ch]));
                    if (audio !== "[[1,2,3],[4,5,6]]") { throw new Error(`audio: ${audio}`); }
                    let midi = JSON.stringify(ctx.midi);
                    if (midi !== "[[0,1,2,3,4,5,6],[7,8,9,10,11,12,13]]") { throw new Error(`midi: ${midi}`); }
                    for (let ch = 0; ch < ctx.audio.length; ch++) {
                        for (let i = 0; i < ctx.audio[ch].length; i++) {
                            ctx.audio[ch][i] *= 2.0;
                        }
                    }
                    ctx.midi.splice(0, ctx.midi.length, [14, 15, 16, 17, 18, 19, 20]);
                });
            "#,
        )
        .unwrap();
        let mut audio = [&mut [1f32, 2., 3.][..], &mut [4., 5., 6.][..]];
        let mut midi = vec![[0u8, 1, 2, 3, 4, 5, 6], [7, 8, 9, 10, 11, 12, 13]];
        rt.audio(&mut audio[..], &mut midi, 48000.0, 1024, 120.0)
            .unwrap();
        assert_eq!(audio, [[2f32, 4., 6.], [8., 10., 12.]]);
        assert_eq!(midi, vec![[14, 15, 16, 17, 18, 19, 20]]);
    }

    #[test]
    fn test_gui() {
        let userdata = Arc::new(Mutex::new(UserData::None));
        let rt = RuntimeActor::new(userdata).unwrap();

        // 入出力の確認
        rt.compile(
            r#"
                "use strict";
                ps88.gui((ctx) => {
                    if (ctx.w !== 640) { throw new Error(`w: ${ctx.w}`); }
                    if (ctx.h !== 480) { throw new Error(`h: ${ctx.h}`); }
                    if (ctx.mouse.x !== 100) { throw new Error(`mouse.x: ${ctx.mouse.x}`); }
                    if (ctx.mouse.y !== 200) { throw new Error(`mouse.y: ${ctx.mouse.y}`); }
                    if (ctx.mouse.pressedL !== true) { throw new Error(`mouse.pressedL: ${ctx.mouse.pressedL}`); }
                    if (ctx.mouse.pressedR !== false) { throw new Error(`mouse.pressedR: ${ctx.mouse.pressedR}`); }
                    ctx.addPolygon([[1, 2], [3, 4]], { fill: 0xff0000, stroke: 0x00ff00, strokeWidth: 2, strokeClosed: true });
                    ctx.addText("Hello", 10, 20, { size: 30, color: 0x0000ff });
                });
            "#,
        )
        .unwrap();
        let args = GuiArgs {
            w: 640.0,
            h: 480.0,
            mouse_x: 100.0,
            mouse_y: 200.0,
            pressed_l: true,
            pressed_r: false,
        };
        let shapes = rt.gui(args).unwrap();
        assert_eq!(
            shapes,
            vec![
                Shape::Polygon {
                    path: vec![(1.0, 2.0), (3.0, 4.0)],
                    fill: Some(0xff0000),
                    stroke: Some(0x00ff00),
                    stroke_width: Some(2.0),
                    stroke_closed: Some(true),
                },
                Shape::Text {
                    text: "Hello".to_string(),
                    x: 10.0,
                    y: 20.0,
                    size: Some(30.0),
                    color: Some(0x0000ff),
                },
            ]
        );
    }

    #[test]
    fn test_save_load() {
        let userdata = Arc::new(Mutex::new(UserData::None));
        let mut rt = Runtime::new(userdata.clone()).unwrap();
        let (tx, rx) = channel();
        rt.add_logger(Box::new(move |msg: String| {
            tx.send(msg).unwrap();
            true
        }));

        // 任意のデータを書き込み/読み込みできる
        rt.compile("ps88.save(new Uint8Array([1, 2]));").unwrap();
        assert_eq!(&*userdata.lock().unwrap(), &UserData::Bytes(vec![1, 2]));
        rt.compile("console.log(JSON.stringify([...ps88.load()]));")
            .unwrap();
        assert_eq!(rx.recv().unwrap(), "[1,2]");
    }
}
