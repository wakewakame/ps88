mod file_watcher;
mod gui;
mod js;
mod params;

use gui::editor;
use nih_plug::prelude::*;
use std::sync::Arc;

pub struct PS88 {
    // プラグイン内で保持するデータ
    params: Arc<params::PS88Params>,

    // JavaScript のランタイム
    runtime: Arc<js::ps88js::RuntimeActor>,

    pos_samples: i64,
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
        let mut midi = Vec::<js::ps88js::NoteEvent>::new();
        while let Some(event) = context.next_event() {
            if let Some(event) = note_event_to_js(event) {
                midi.push(event);
            }
        }

        // スクリプトを実行
        {
            let transport = context.transport();
            if let Err(e) = self.runtime.audio(
                buffer.as_slice(),
                &mut midi,
                transport.sample_rate as f64,
                transport.pos_samples().unwrap_or(self.pos_samples) as u64,
                transport.tempo.unwrap_or(0.0),
            ) {
                log::error!("{}", e);
            }
            self.pos_samples += buffer.samples() as i64;
        }

        // midi の内容を context に書き戻す
        for event in midi {
            context.send_event(note_event_from_js(event));
        }

        ProcessStatus::Normal
    }
}

// nih_plug の NoteEvent を JavaScript 側に渡す NoteEvent に変換する。
// 未対応のイベントは None を返す。
// TODO: 他のイベントも処理する
fn note_event_to_js(event: NoteEvent<()>) -> Option<js::ps88js::NoteEvent> {
    match event {
        NoteEvent::NoteOn {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        } => Some(js::ps88js::NoteEvent::NoteOn {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        }),
        NoteEvent::NoteOff {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        } => Some(js::ps88js::NoteEvent::NoteOff {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        }),
        _ => None,
    }
}

// JavaScript 側の NoteEvent を nih_plug の NoteEvent に変換する
fn note_event_from_js(event: js::ps88js::NoteEvent) -> NoteEvent<()> {
    match event {
        js::ps88js::NoteEvent::NoteOn {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        } => NoteEvent::NoteOn {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        },
        js::ps88js::NoteEvent::NoteOff {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        } => NoteEvent::NoteOff {
            timing,
            voice_id,
            channel,
            note,
            velocity,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_event_conversion() {
        // NoteOn / NoteOff は相互変換できる
        let note_on = NoteEvent::<()>::NoteOn {
            timing: 10,
            voice_id: Some(1),
            channel: 2,
            note: 60,
            velocity: 0.5,
        };
        let js_event = note_event_to_js(note_on).unwrap();
        assert_eq!(
            js_event,
            js::ps88js::NoteEvent::NoteOn {
                timing: 10,
                voice_id: Some(1),
                channel: 2,
                note: 60,
                velocity: 0.5,
            }
        );
        assert_eq!(note_event_from_js(js_event), note_on);

        let note_off = NoteEvent::<()>::NoteOff {
            timing: 20,
            voice_id: None,
            channel: 3,
            note: 61,
            velocity: 0.25,
        };
        let js_event = note_event_to_js(note_off).unwrap();
        assert_eq!(note_event_from_js(js_event), note_off);

        // 未対応のイベントは None になる
        let choke = NoteEvent::<()>::Choke {
            timing: 0,
            voice_id: None,
            channel: 0,
            note: 0,
        };
        assert_eq!(note_event_to_js(choke), None);
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
