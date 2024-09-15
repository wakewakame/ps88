pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub trait ScriptRuntime {
    //fn init(&mut self, param: ());
    fn compile(&mut self, code: &str) -> Result<()>;
    fn audio(&mut self, audio: &mut [f32]) -> Result<()>;
}
