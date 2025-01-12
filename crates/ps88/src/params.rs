use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::{Arc, Mutex};

const DEFAULT_SCRIPT: &'static str = std::include_str!("default_script.js");

// プラグイン内で保持するデータ
#[derive(Params)]
pub struct PS88Params {
    // ユーザーが入力したコード
    #[persist = "code"]
    pub code: Arc<Mutex<String>>,

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

impl Default for PS88Params {
    fn default() -> Self {
        Self {
            code: Arc::new(Mutex::new(String::from(DEFAULT_SCRIPT))),
            param1: FloatParam::new("Param1", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            param2: FloatParam::new("Param2", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            param3: FloatParam::new("Param3", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            param4: FloatParam::new("Param4", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            editor_state: EguiState::from_size(640, 360),
        }
    }
}
