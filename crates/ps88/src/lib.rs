mod file_watcher;
mod gui;
mod js;
mod params;

use gui::editor;
use nih_plug::prelude::*;
use std::sync::Arc;

// 1 回の process() で扱う MIDI イベント数の目安。
// これを超えるとオーディオスレッドでバッファの再確保が発生する。
const MIDI_EVENT_CAPACITY: usize = 1024;

pub struct PS88 {
    // プラグイン内で保持するデータ
    params: Arc<params::PS88Params>,

    // JavaScript のランタイム
    runtime: Arc<js::ps88js::RuntimeActor>,

    pos_samples: i64,

    // process() 内でのアロケーションを避けるための MIDI イベント用バッファ
    midi: Vec<js::ps88js::NoteEvent>,
}

impl Default for PS88 {
    fn default() -> Self {
        let params = params::PS88Params::default();
        let runtime: Arc<js::ps88js::RuntimeActor> =
            Arc::new(js::ps88js::RuntimeActor::new(params.userdata.clone()).unwrap());
        Self {
            params: Arc::new(params),
            runtime,
            pos_samples: 0,
            midi: Vec::with_capacity(MIDI_EVENT_CAPACITY),
        }
    }
}

impl Plugin for PS88 {
    const NAME: &'static str = "ps88";
    const VENDOR: &'static str = "ps88";
    const URL: &'static str = env!("CARGO_PKG_REPOSITORY");
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;
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
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // デフォルトのスクリプトをコンパイル
        {
            if let Err(err) = self
                .runtime
                .compile(&*self.params.code.lock().unwrap().clone())
            {
                log::error!("{}", err);
            }
        }
        self.pos_samples = 0;
        true
    }

    fn reset(&mut self) {
        self.pos_samples = 0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // イベントを取得
        // NOTE: オーディオスレッドでのアロケーションを避けるため、
        // 事前確保したバッファを再利用する。
        // 事前確保した容量を超えた場合のみアロケーションが発生する。
        let midi = &mut self.midi;
        midi.clear();
        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::NoteOn {
                    timing,
                    voice_id,
                    channel,
                    note,
                    velocity,
                } => {
                    util::permit_alloc(|| {
                        midi.push(js::ps88js::NoteEvent::NoteOn {
                            timing,
                            voice_id,
                            channel,
                            note,
                            velocity,
                        })
                    });
                }
                NoteEvent::NoteOff {
                    timing,
                    voice_id,
                    channel,
                    note,
                    velocity,
                } => {
                    util::permit_alloc(|| {
                        midi.push(js::ps88js::NoteEvent::NoteOff {
                            timing,
                            voice_id,
                            channel,
                            note,
                            velocity,
                        })
                    });
                }
                // TODO: 他のイベントも処理する
                _ => {}
            };
        }

        // スクリプトを実行
        {
            let transport = context.transport();
            if let Err(e) = self.runtime.audio(
                buffer.as_slice(),
                midi,
                transport.sample_rate as f64,
                transport.pos_samples().unwrap_or(self.pos_samples) as u64,
                transport.tempo.unwrap_or(0.0),
            ) {
                log::error!("{}", e);
            }
            self.pos_samples += buffer.samples() as i64;
        }

        // midi の内容を context に書き戻す
        for event in midi.drain(..) {
            match event {
                js::ps88js::NoteEvent::NoteOn {
                    timing,
                    voice_id,
                    channel,
                    note,
                    velocity,
                } => {
                    context.send_event(NoteEvent::NoteOn {
                        timing,
                        voice_id,
                        channel,
                        note,
                        velocity,
                    });
                }
                js::ps88js::NoteEvent::NoteOff {
                    timing,
                    voice_id,
                    channel,
                    note,
                    velocity,
                } => {
                    context.send_event(NoteEvent::NoteOff {
                        timing,
                        voice_id,
                        channel,
                        note,
                        velocity,
                    });
                }
            };
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for PS88 {
    const CLAP_ID: &'static str = "ps88";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("programmable synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}
nih_export_clap!(PS88);

impl Vst3Plugin for PS88 {
    const VST3_CLASS_ID: [u8; 16] = *b"PS88____________";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}
nih_export_vst3!(PS88);
