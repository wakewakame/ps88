pub struct Api {
    audio_callback: Option<Box<dyn AudioCallback>>,
    gui_callback: Option<Box<dyn GuiCallback>>,
    save_data: Vec<u8>,
}

impl Api {
    pub fn new() -> Self {
        Api {
            audio_callback: None,
            gui_callback: None,
            save_data: Vec::new(),
        }
    }

    pub fn audio(&mut self, callback: Box<dyn AudioCallback>) {
        self.audio_callback = Some(callback);
    }

    pub fn gui(&mut self, callback: Box<dyn GuiCallback>) {
        self.gui_callback = Some(callback);
    }

    pub fn save(&mut self, data: &[u8]) {
        self.save_data = Vec::from(data);
    }

    pub fn load(&self) -> &[u8] {
        self.save_data.as_slice()
    }
}

pub trait AudioCallback: FnMut(&mut AudioContext) + Sync + Send {}

pub struct AudioContext {
    audio: Vec<Vec<f32>>,
    midi: Vec<u8>,
    sample_rate: f32,
    current_frame: u64,
}

pub trait GuiCallback: FnMut(&mut GuiContext) + Sync + Send {}

pub struct GuiContext {
    width: f32,
    height: f32,
    mouse: Mouse,
    shapes: Vec<Shape>,
}

pub struct Mouse {
    x: f32,
    y: f32,
    pressed_left: bool,
    pressed_right: bool,
}

pub enum Shape {
    Polygon(PolygonShape),
    Text(TextShape),
}

pub struct PolygonShape {
    pub shape: Vec<f32>,
    pub fill: Option<u32>,
    pub stroke: Option<u32>,
    pub stroke_width: Option<f32>,
    pub stroke_closed: Option<bool>,
}

pub struct TextShape {
    pub text: String,
    pub size: Option<f32>,
    pub pos: Option<[f32; 2]>,
    pub color: Option<u32>,
}
