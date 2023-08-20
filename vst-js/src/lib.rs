use nih_plug::prelude::*;
use std::sync::Arc;

struct VstJs {
    params: Arc<VstJsParams>,
}

#[derive(Params)]
struct VstJsParams {
    #[id = "vst_js"]
    pub gain: FloatParam,
}

impl Default for VstJs {
    fn default() -> Self {
        Self {
            params: Arc::new(VstJsParams::default()),
        }
    }
}

impl Default for VstJsParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}

impl Plugin for VstJs {
    const NAME: &'static str = "VstJs";
    const VENDOR: &'static str = "";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),

            aux_input_ports: &[],
            aux_output_ports: &[],

            names: PortNames::const_default(),
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for channel_samples in buffer.iter_samples() {
            let gain = self.params.gain.smoothed.next();

            for sample in channel_samples {
                *sample *= gain;
            }
        }

        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {}
}

impl Vst3Plugin for VstJs {
    const VST3_CLASS_ID: [u8; 16] = *b"VstJsPlug_______";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_vst3!(VstJs);
