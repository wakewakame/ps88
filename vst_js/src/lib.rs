use atomic_float::AtomicF32;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::collections::HashMap;
use std::sync::Arc;
mod runtime;

const PEAK_METER_DECAY_MS: f64 = 150.0;

pub struct Gain {
    params: Arc<GainParams>,
    peak_meter_decay_weight: f32,
    peak_meter: Arc<AtomicF32>,
    sample_rate: f32,
    time: u64,
    midi_input: HashMap<(u8, u8), (u64, f32, f32)>,
    runtime: Box<dyn runtime::runtime::ScriptRuntime + Send>,
}

#[derive(Params)]
pub struct GainParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "gain"]
    pub gain: FloatParam,
}

impl Default for Gain {
    fn default() -> Self {
        let runtime: Box<dyn runtime::runtime::ScriptRuntime + Send> = Box::new(
            runtime::js_sync::JsRuntimeBuilder::new()
                .on_log(std::sync::Arc::new(|log| {
                    println!("{}", log);
                }))
                .build(),
        );
        Self {
            params: Arc::new(GainParams::default()),

            sample_rate: 1.0,
            time: 0,
            peak_meter_decay_weight: 1.0,
            peak_meter: Arc::new(AtomicF32::new(util::MINUS_INFINITY_DB)),
            midi_input: HashMap::new(),
            runtime,
        }
    }
}

impl Default for GainParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(640, 360),

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

impl Plugin for Gain {
    const NAME: &'static str = "vst_js";
    const VENDOR: &'static str = "vst_js";
    const URL: &'static str = "";
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
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let peak_meter = self.peak_meter.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.label("Gain");
                    ui.add(widgets::ParamSlider::for_param(&params.gain, setter));

                    let peak_meter =
                        util::gain_to_db(peak_meter.load(std::sync::atomic::Ordering::Relaxed));
                    let peak_meter_text = if peak_meter > util::MINUS_INFINITY_DB {
                        format!("{peak_meter:.1} dBFS")
                    } else {
                        String::from("-inf dBFS")
                    };

                    let peak_meter_normalized = (peak_meter + 60.0) / 60.0;
                    ui.allocate_space(egui::Vec2::splat(2.0));
                    ui.add(
                        egui::widgets::ProgressBar::new(peak_meter_normalized)
                            .text(peak_meter_text),
                    );
                });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.peak_meter_decay_weight = 0.25f64
            .powf((buffer_config.sample_rate as f64 * PEAK_METER_DECAY_MS / 1000.0).recip())
            as f32;

        if let Err(e) = (&mut *self.runtime).compile(
            r#"
                console.log("hello world");
                let sum = 0;
				let count = 0;
                (input, output) => {
                    input.forEach((v, index) => {
                        output[index] = Math.sin(count / 44100 * 2 * Math.PI * 440);
						count += 1;
                    });
                    return 100;
                };
            "#,
        ) {
            println!("compile error: {}", e);
            return false;
        };

        true
    }

    fn reset(&mut self) {
        self.time = 0;
        self.midi_input = HashMap::new();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        //let mut next_event = context.next_event();

        let input = vec![0f32; buffer.as_slice()[0].len()];
        if let Err(e) = (&mut *self.runtime).process(&input, buffer.as_slice()[0]) {
            println!("process error: {}", e);
        }

        /*
        //let input = buffer.iter_samples()[0].copied().collect::<Vec<f32>>();
        let mut output = vec![0f32; input.len()];
        if let Err(e) = (&mut *self.runtime).process(&input, &mut output) {
            println!("process error: {}", e);
            return false;
        }
        buffer
            .iter_samples_mut()
            .zip(output.iter())
            .for_each(|(s, v)| *s = *v);
        */

        /*
        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            self.time += 1;
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }

                match event {
                    NoteEvent::NoteOn {
                        channel,
                        note,
                        velocity,
                        ..
                    } => {
                        self.midi_input
                            .insert((channel, note), (self.time, velocity, 0.0));
                    }
                    NoteEvent::NoteOff { channel, note, .. } => {
                        self.midi_input.remove(&(channel, note));
                    }
                    _ => (),
                }

                next_event = context.next_event();
            }

            //let input = vec![0f32, 1f32, 2f32, 5f32];
            //let mut output = vec![0f32; 4];
            if let Err(e) = (&mut *self.runtime).process(&input, &mut output) {
                println!("process error: {}", e);
                return false;
            }
            //for (i, v) in output.iter().enumerate() {
            //    println!("output[{}]: {}", i, v);
            //}

            let mut wave = 0.0;
            for ((_, note), (time, velocity, _)) in self.midi_input.iter() {
                let time = (self.time - time) as f32 / self.sample_rate;
                let hz = 440.0 * 2_f32.powf((*note as f32 - 69.0) / 12.0);
                wave += (time * 2.0 * std::f32::consts::PI * hz).sin() * velocity;
            }

            let mut amplitude = 0.0;
            let num_samples = channel_samples.len();
            let gain = self.params.gain.smoothed.next();
            for sample in channel_samples {
                *sample += wave;
                *sample *= gain;
                amplitude += *sample;
            }

            if self.params.editor_state.is_open() {
                amplitude = (amplitude / num_samples as f32).abs();
                let current_peak_meter = self.peak_meter.load(std::sync::atomic::Ordering::Relaxed);
                let new_peak_meter = if amplitude > current_peak_meter {
                    amplitude
                } else {
                    current_peak_meter * self.peak_meter_decay_weight
                        + amplitude * (1.0 - self.peak_meter_decay_weight)
                };

                self.peak_meter
                    .store(new_peak_meter, std::sync::atomic::Ordering::Relaxed)
            }
        }
        */

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for Gain {
    const VST3_CLASS_ID: [u8; 16] = *b"VstJs___________";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Instrument];
}

nih_export_vst3!(Gain);
