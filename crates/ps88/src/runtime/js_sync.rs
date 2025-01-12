use crate::runtime::js;
use crate::runtime::runtime;
use crate::runtime::runtime::ScriptRuntime;

pub struct JsRuntime {
    message: std::sync::mpsc::Sender<Message>,
    handle: std::thread::JoinHandle<()>,
}

enum Message {
    Reset,
    Compile(String, std::sync::mpsc::Sender<runtime::Result<()>>),
    Audio(
        Vec<f32>,
        usize,
        f32,
        Vec<u8>,
        std::sync::mpsc::Sender<(runtime::Result<()>, Vec<f32>)>,
    ),
    Gui(
        runtime::Pos2,
        runtime::Mouse,
        std::sync::mpsc::Sender<runtime::Result<Vec<runtime::Shape>>>,
    ),
    AddLogger(Box<dyn Fn(String) -> bool + Send + Sync>),
}

impl JsRuntime {
    pub fn new() -> Self {
        let (message_tx, message_rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut runtime = js::JsRuntime::new();
            for event in message_rx {
                match event {
                    Message::Reset => {
                        runtime.reset();
                    }
                    Message::Compile(code, output_tx) => {
                        let result = runtime.compile(&code);
                        let _ = output_tx.send(result);
                    }
                    Message::Audio(mut audio, ch, sampling_rate, midi, output_tx) => {
                        // TODO: unsafe を使えば audio は参照渡しで読み書きできるかもしれない
                        let result = runtime.audio(&mut audio, ch, sampling_rate, &midi);
                        let _ = output_tx.send((result, audio));
                    }
                    Message::Gui(area, mouse, output_tx) => {
                        let result = runtime.gui(&area, &mouse);
                        let _ = output_tx.send(result);
                    }
                    Message::AddLogger(logger) => {
                        let _ = runtime.add_logger(logger);
                    }
                }
            }
        });
        JsRuntime {
            message: message_tx,
            handle,
        }
    }
}

impl Drop for JsRuntime {
    fn drop(&mut self) {
        let message = std::mem::replace(&mut self.message, std::sync::mpsc::channel().0);
        drop(message);
        let handler = std::mem::replace(&mut self.handle, std::thread::spawn(move || {}));
        let _ = handler.join();
    }
}

impl runtime::ScriptRuntime for JsRuntime {
    fn reset(&mut self) {
        let _ = self.message.send(Message::Reset);
    }
    fn compile(&mut self, code: &str) -> runtime::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.message
            .send(Message::Compile(code.to_string(), tx))
            .map_err(|_| js::JsRuntimeError::UnexpectedError("failed to send".into()))?;
        match rx.recv() {
            Ok(result) => result,
            _ => Err(js::JsRuntimeError::UnexpectedError("failed to receive".into()).into()),
        }
    }

    fn audio(
        &mut self,
        audio: &mut [f32],
        ch: usize,
        sampling_rate: f32,
        midi: &[u8],
    ) -> runtime::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.message
            .send(Message::Audio(
                audio.to_vec(),
                ch,
                sampling_rate,
                midi.to_vec(),
                tx,
            ))
            .map_err(|_| js::JsRuntimeError::UnexpectedError("failed to send".into()))?;
        match rx.recv() {
            Ok((result, out_audio)) => {
                audio
                    .iter_mut()
                    .zip(out_audio.iter())
                    .for_each(|(o, v)| *o = *v);
                result
            }
            _ => Err(js::JsRuntimeError::UnexpectedError("failed to receive".into()).into()),
        }
    }

    fn gui(
        &mut self,
        area: &runtime::Pos2,
        mouse: &runtime::Mouse,
    ) -> runtime::Result<Vec<runtime::Shape>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.message
            .send(Message::Gui(area.clone(), mouse.clone(), tx))
            .map_err(|_| js::JsRuntimeError::UnexpectedError("failed to send".into()))?;
        match rx.recv() {
            Ok(result) => result,
            _ => Err(js::JsRuntimeError::UnexpectedError("failed to receive".into()).into()),
        }
    }

    fn add_logger(
        &mut self,
        logger: Box<dyn Fn(String) -> bool + Sync + Send>,
    ) -> runtime::Result<()> {
        self.message
            .send(Message::AddLogger(logger))
            .map_err(|_| js::JsRuntimeError::UnexpectedError("failed to send".into()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::runtime;

    #[test]
    fn audio() {
        // console.log の出力結果保存用
        let (tx, rx) = std::sync::mpsc::channel();

        // 初期化
        let runtime: std::sync::Arc<std::sync::Mutex<dyn runtime::ScriptRuntime + Send + Sync>> =
            std::sync::Arc::new(std::sync::Mutex::new(JsRuntime::new()));
        runtime
            .lock()
            .unwrap()
            .add_logger(Box::new(move |log| tx.send(log).is_ok()))
            .unwrap();

        // compile が 3 回行えることを確認
        let runtime2 = runtime.clone();
        let th = std::thread::spawn(move || {
            for i in 0..3 {
                runtime2
                    .lock()
                    .unwrap()
                    .compile(
                        r#"
                    "use strict";
                    console.log("init: ${i}");
                    let count = 0;
                    const audio = (ctx) => {
                        console.log(`init: ${i}, count: ${count++}`);
                        for (let i = 0; i < ctx.audio.length; i++) {
                            ctx.audio[i] = ctx.audio[i] * 2.0;
                        }
                    };
                    const gui = () => {};
                "#
                        .replace("${i}", &i.to_string())
                        .as_str(),
                    )
                    .unwrap();

                // audio の実行が 3 回行えることを確認
                for _ in 0..3 {
                    // 実行ごとに入力配列の数を変える
                    let mut audio: Vec<f32> = (0..(i + 1) * 100).map(|x| x as f32).collect();
                    runtime2
                        .lock()
                        .unwrap()
                        .audio(&mut audio, 2, 48000.0, &[])
                        .unwrap();
                    assert_eq!(
                        audio,
                        (0..(i + 1) * 100)
                            .map(|x| (x * 2) as f32)
                            .collect::<Vec<f32>>()
                    );
                }
            }
        });
        th.join().unwrap();
        drop(runtime);

        // console.log が取得できていることを確認
        assert_eq!(rx.try_recv(), Ok("init: 0".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 0, count: 0".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 0, count: 1".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 0, count: 2".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 1".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 1, count: 0".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 1, count: 1".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 1, count: 2".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 2".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 2, count: 0".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 2, count: 1".to_string()));
        assert_eq!(rx.try_recv(), Ok("init: 2, count: 2".to_string()));
        assert_eq!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        );
    }
}
