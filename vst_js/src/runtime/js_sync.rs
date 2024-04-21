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
    Process(
        Vec<f32>,
        usize,
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
                    Message::Process(input, output_len, output_tx) => {
                        // TODO: unsafe を使えば input / output は参照渡しで読み書きできるかもしれない
                        let mut output = vec![0f32; output_len];
                        let result = runtime.process(&input, &mut output);
                        let _ = output_tx.send((result, output));
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

    fn process(&mut self, input: &[f32], output: &mut [f32]) -> runtime::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        let input = input.to_vec();
        self.message
            .send(Message::Process(input, output.len(), tx))
            .map_err(|_| js::JsRuntimeError::UnexpectedError("failed to send".into()))?;
        match rx.recv() {
            Ok((result, out)) => {
                output.iter_mut().zip(out.iter()).for_each(|(o, v)| *o = *v);
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
    fn process() {
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
                let result = runtime2.lock().unwrap().compile(
                    r#"
                    console.log("init: ${i}");
                    let count = 0;
                    (input, output) => {
                        console.log(`init: ${i}, count: ${count++}`);
                        input.forEach((v, i) => {{ output[i] = v * 2.0; }});
                    };
                "#
                    .replace("${i}", &i.to_string())
                    .as_str(),
                );
                assert!(result.is_ok());

                // process の実行が 3 回行えることを確認
                for _ in 0..3 {
                    // 実行ごとに入力配列の数を変える
                    let input: Vec<f32> = (0..(i + 1) * 100).map(|x| x as f32).collect();
                    let mut output = input.clone();
                    let result = runtime2.lock().unwrap().process(&input, &mut output);
                    assert!(result.is_ok());
                    assert_eq!(
                        output,
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
