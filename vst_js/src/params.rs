use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::{Arc, Mutex};

const DEFAULT_SCRIPT: &'static str = r#"console.log("hello world");
let sum = 0;
let count = 0;
(input, output) => {
	input.forEach((v, index) => {
		output[index] = Math.sin(count / 44100 * 2 * Math.PI * 440) * 0.01;
		count += 1;
	});
	return 100;
};"#;

// VST プラグイン内で保持するデータ
#[derive(Params)]
pub struct VstJsParams {
    // ユーザーが入力したコード
    #[persist = "code"]
    pub code: Arc<Mutex<String>>,

    // code の中で読み書きされる storage
    pub code_storage: Arc<Mutex<String>>,

    // パラメータの数は固定で 4 つだけ
    #[id = "param1"]
    pub param1: FloatParam,
    #[id = "param2"]
    pub param2: FloatParam,
    #[id = "param3"]
    pub param3: FloatParam,
    #[id = "param4"]
    pub param4: FloatParam,

    // エディターの状態
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
}

impl Default for VstJsParams {
    fn default() -> Self {
        Self {
            code: Arc::new(Mutex::new(String::from(DEFAULT_SCRIPT))),
            code_storage: Arc::new(Mutex::new(String::from(""))),
            param1: FloatParam::new("Param1", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            param2: FloatParam::new("Param2", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            param3: FloatParam::new("Param3", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            param4: FloatParam::new("Param4", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            editor_state: EguiState::from_size(640, 360),
        }
    }
}
