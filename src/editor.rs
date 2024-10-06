use super::file_watcher::Watcher;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};
use std::io::Read;
use std::sync::{Arc, Mutex};

pub fn editor(
    params: Arc<crate::params::PS88Params>,
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
                ui.add(CustomButton());
            });
        },
    )
}

struct CustomButton();
impl egui::Widget for CustomButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let pos = ui.input(|s| s.pointer.hover_pos().unwrap_or_default());
        let vertex_default = egui::epaint::Vertex {
            pos: egui::pos2(0.0, 0.0),
            color: egui::Color32::WHITE,
            uv: egui::pos2(0.0, 0.0),
        };
        let p = |x: f32, y: f32, g: u8| egui::epaint::Vertex {
            pos: egui::pos2(x, y),
            color: egui::Color32::from_rgba_premultiplied(g, 0, 0, 255),
            ..vertex_default
        };
        let shape = egui::Shape::Mesh(egui::Mesh {
            //indices: vec![0, 1, 2, 0, 2, 3],
            indices: vec![0, 1, 2, 0, 2, 3],
            vertices: vec![
                p(10.0, 110.0, 128),
                p(90.0, 110.0, 255),
                p(10.0, 190.0, 90),
                p(pos.x, pos.y, 198),
            ],
            texture_id: egui::TextureId::Managed(0),
        });
        let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        painter.add(shape);
        response
    }
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
