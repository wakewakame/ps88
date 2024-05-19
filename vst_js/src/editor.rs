use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use std::sync::{Arc, Mutex};

pub fn editor(
    params: Arc<crate::params::VstJsParams>,
    runtime: Arc<Mutex<dyn crate::runtime::runtime::ScriptRuntime + Sync + Send>>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        (),
        |_, _| {},
        move |egui_ctx, setter, _state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.label("Gain");
                ui.add(widgets::ParamSlider::for_param(&params.param1, setter));
                ui.add(widgets::ParamSlider::for_param(&params.param2, setter));
                ui.add(widgets::ParamSlider::for_param(&params.param3, setter));
                ui.add(widgets::ParamSlider::for_param(&params.param4, setter));

                {
                    let mut code = params.code.lock().unwrap();
                    let prev_code = code.clone();
                    ui.add(
                        egui::TextEdit::multiline(&mut *code)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_rows(10)
                            .lock_focus(true)
                            .desired_width(f32::INFINITY),
                    );
                    if prev_code != *code {
                        // TODO: 500ms 待ってからコンパイルするようにする
                        let mut runtime = runtime.lock().unwrap();
                        if let Err(err) = (&mut runtime).compile(&*code) {
                            println!("{}", err);
                        }
                    }
                }
            });
        },
    )
}
