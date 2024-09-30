use crate::runtime::js;
use crate::runtime::runtime;
use crate::runtime::runtime::ScriptRuntime;

pub struct JsRuntimeBuilder {
    on_log: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
}

pub struct JsRuntime {
    message: std::sync::mpsc::Sender<Message>,
}

enum Message {
    Compile(String, std::sync::mpsc::Sender<runtime::Result<()>>),
    Audio(
        Vec<f32>,
        usize,
        f32,
        Vec<u8>,
        std::sync::mpsc::Sender<(runtime::Result<()>, Vec<f32>)>,
    ),
}

impl JsRuntimeBuilder {
    pub fn new() -> Self {
        JsRuntimeBuilder { on_log: None }
    }

    pub fn build(self) -> JsRuntime {
        let (message_tx, message_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let builder = js::JsRuntimeBuilder::new();
            let builder = if let Some(on_log) = self.on_log {
                builder.on_log(std::rc::Rc::new(move |log| {
                    on_log(log);
                }))
            } else {
                builder
            };
            let mut runtime = builder.build();
            for event in message_rx {
                match event {
                    Message::Compile(code, output_tx) => {
                        let result = runtime.compile(&code);
                        let _ = output_tx.send(result);
                    }
                    Message::Audio(mut audio, ch, sampling_rate, midi, output_tx) => {
                        // TODO: unsafe を使えば audio は参照渡しで読み書きできるかもしれない
                        let result = runtime.audio(&mut audio, ch, sampling_rate, &midi);
                        let _ = output_tx.send((result, audio));
                    }
                }
            }
        });
        JsRuntime {
            message: message_tx,
        }
    }

    pub fn on_log(mut self, on_log: std::sync::Arc<dyn Fn(String) + Send + Sync>) -> Self {
        self.on_log = Some(on_log);
        self
    }
}

impl runtime::ScriptRuntime for JsRuntime {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::runtime;

    #[test]
    fn audio() {
        // console.log の出力結果保存用
        let logs = std::sync::Arc::new(std::sync::Mutex::<Vec<String>>::new(vec![]));
        let logs_clone = logs.clone();

        // 初期化
        let runtime: std::sync::Arc<std::sync::Mutex<dyn runtime::ScriptRuntime + Send + Sync>> =
            std::sync::Arc::new(std::sync::Mutex::new(
                JsRuntimeBuilder::new()
                    .on_log(std::sync::Arc::new(move |log| {
                        //let mut logs = logs_clone.borrow_mut();
                        let mut logs = logs_clone.lock().unwrap();
                        logs.push(log);
                    }))
                    .build(),
            ));

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

        // console.log が取得できていることを確認
        let logs = logs.lock().unwrap();
        assert_eq!(logs.len(), 12);
        assert_eq!(logs[0], "init: 0");
        assert_eq!(logs[1], "init: 0, count: 0");
        assert_eq!(logs[2], "init: 0, count: 1");
        assert_eq!(logs[3], "init: 0, count: 2");
        assert_eq!(logs[4], "init: 1");
        assert_eq!(logs[5], "init: 1, count: 0");
        assert_eq!(logs[6], "init: 1, count: 1");
        assert_eq!(logs[7], "init: 1, count: 2");
        assert_eq!(logs[8], "init: 2");
        assert_eq!(logs[9], "init: 2, count: 0");
        assert_eq!(logs[10], "init: 2, count: 1");
        assert_eq!(logs[11], "init: 2, count: 2");
    }
}
