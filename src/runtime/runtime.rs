use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[derive(Debug, Clone, Serialize)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Mouse {
    pub x: f32,
    pub y: f32,
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Deserialize)]
pub enum Shape {
    Polygon(PolygonShape),
    Text(TextShape),
}

#[derive(Debug, Deserialize)]
pub struct PolygonShape {
    pub shape: Vec<f32>,
    pub fill: Option<u32>,
    pub stroke: Option<u32>,
    pub stroke_width: Option<f32>,
    pub stroke_closed: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct TextShape {
    pub text: String,
    pub size: Option<f32>,
    pub pos: Option<[f32; 2]>,
    pub color: Option<u32>,
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
    fn gui(&mut self, area: &Pos2, mouse: &Mouse) -> Result<Vec<Shape>>;
}
