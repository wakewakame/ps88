mod editor;
mod params;
mod runtime;

use nih_plug::prelude::*;
use std::sync::{Arc, Mutex};

pub struct VstJs {
    // VST プラグイン内で保持するデータ
    params: Arc<params::VstJsParams>,

    // JavaScript のランタイム
    runtime: Arc<Mutex<dyn runtime::runtime::ScriptRuntime + Sync + Send>>,

    sample_rate: f32,
    time: u64,
}

impl Default for VstJs {
    fn default() -> Self {
        let runtime: Arc<Mutex<dyn runtime::runtime::ScriptRuntime + Sync + Send>> =
            Arc::new(Mutex::new(
                runtime::js_sync::JsRuntimeBuilder::new()
                    .on_log(std::sync::Arc::new(|log| {
                        println!("{}", log);
                    }))
                    .build(),
            ));
        Self {
            params: Arc::new(params::VstJsParams::default()),
            runtime,
            sample_rate: 1.0,
            time: 0,
        }
    }
}

impl Plugin for VstJs {
    const NAME: &'static str = "vst_js";
    const VENDOR: &'static str = "vst_js";
    const URL: &'static str = env!("CARGO_PKG_REPOSITORY");
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::editor(self.params.clone(), self.runtime.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // デフォルトのスクリプトをコンパイル
        {
            let mut runtime = self.runtime.lock().unwrap();
            if let Err(err) = (&mut runtime).compile(&*self.params.code.lock().unwrap().clone()) {
                println!("{}", err);
            }
        }
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn reset(&mut self) {
        self.time = 0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        //let mut next_event = context.next_event();

        let input = vec![0f32; buffer.as_slice()[0].len()];
        {
            let mut runtime = self.runtime.lock().unwrap();
            if let Err(e) = (&mut runtime).process(&input, buffer.as_slice()[0]) {
                println!("process error: {}", e);
            }
        }

        /*
        for (_sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            self.time += 1;
            let time = self.time as f32 / self.sample_rate;
            let wave = (time * 2f32 * std::f32::consts::PI * 440f32).sin() * 0.01f32;

            for sample in channel_samples {
                *sample = wave;
            }
        }
        */

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for VstJs {
    const VST3_CLASS_ID: [u8; 16] = *b"VstJs___________";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Instrument];
}

nih_export_vst3!(VstJs);
