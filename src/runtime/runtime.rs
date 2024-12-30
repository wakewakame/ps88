use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[derive(Debug, Clone, Serialize)]
pub struct Mouse {
    pub x: f32,
    pub y: f32,
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Deserialize)]
pub struct Shape {
    pub fill: Option<u32>,
    pub stroke: Option<(u32, f32, bool)>, // color, width, closed
    pub shape: Vec<f32>,
}

pub trait ScriptRuntime {
    //fn init(&mut self, param: ());
    fn compile(&mut self, code: &str) -> Result<()>;
    fn audio(
        &mut self,
        audio: &mut [f32],
        ch: usize,
        sampling_rate: f32,
        midi: &[u8],
    ) -> Result<()>;
    fn gui(&mut self, mouse: &Mouse) -> Result<Vec<Shape>>;
}
