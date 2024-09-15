use super::file_watcher::Watcher;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use std::io::Read;
use std::sync::{Arc, Mutex};

pub fn editor(
    params: Arc<crate::params::VstJsParams>,
    runtime: Arc<Mutex<dyn crate::runtime::runtime::ScriptRuntime + Sync + Send>>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        Arc::<Mutex<Option<Box<dyn super::file_watcher::Watcher + Sync + Send>>>>::new(Mutex::new(
            None,
        )),
        |_, _| {},
        move |egui_ctx, setter, state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        let runtime = runtime.clone();
                        let state = state.clone();
                        let param_code = params.code.clone();
                        std::thread::spawn(move || {
                            let result = rfd::FileDialog::new().pick_file();
                            if let Some(path) = result {
                                if let Ok(watcher) = load_script(&path, move |code| {
                                    if let Err(err) = runtime.lock().unwrap().compile(&*code) {
                                        println!("{}", err);
                                    }
                                    if let Ok(mut param_code) = param_code.lock() {
                                        *param_code = code;
                                    }
                                }) {
                                    let mut state = state.lock().unwrap();
                                    *state = Some(watcher);
                                }
                            }
                        });
                        ui.close_menu();
                    }
                    if ui.button("Save Script").clicked() {
                        ui.close_menu();
                    }
                });
                ui.label("Gain");
                ui.add(widgets::ParamSlider::for_param(&params.param1, setter));
                ui.add(widgets::ParamSlider::for_param(&params.param2, setter));
                ui.add(widgets::ParamSlider::for_param(&params.param3, setter));
                ui.add(widgets::ParamSlider::for_param(&params.param4, setter));
            });
        },
    )
}

fn load_script<F: Fn(String) + Sync + Send + 'static>(
    path: &std::path::Path,
    callback: F,
) -> Result<Box<dyn super::file_watcher::Watcher + Send + Sync>, ()> {
    let Ok(mut file) = std::fs::File::open(&path) else {
        return Err(());
    };
    let mut code = String::new();
    file.read_to_string(&mut code).unwrap();
    callback(code);

    let mut watcher: Box<dyn Watcher + Send + Sync> =
        Box::new(super::file_watcher::WatcherImpl::new());
    let Ok(rx) = watcher.watch(path) else {
        return Err(());
    };
    let rx = super::file_watcher::relay_latest(rx, std::time::Duration::from_millis(100));
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let path = path;
        for _ in rx {
            let Ok(mut file) = std::fs::File::open(&path) else {
                break;
            };
            let mut code = String::new();
            file.read_to_string(&mut code).unwrap();
            callback(code);
        }
    });
    // 呼び出し元が watcher を drop することでファイル監視が終了するようにする
    return Ok(watcher);
}
