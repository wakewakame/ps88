use super::canvas::*;
use crate::file_watcher::*;
use crate::js;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::io::Read;
use std::sync::{Arc, Mutex};

#[derive(PartialEq)]
enum Tab {
    Main,
    Code,
}

#[derive(PartialEq)]
enum LogType {
    Info,
    Error,
}

struct UserState {
    log: Arc<Mutex<Vec<(String, LogType)>>>,
    watcher: Arc<Mutex<Option<Box<dyn Watcher + Sync + Send>>>>,
    tab: Tab,
}

impl UserState {
    fn new() -> Self {
        Self {
            log: Arc::new(Mutex::new(vec![])),
            watcher: Arc::new(Mutex::new(None)),
            tab: Tab::Main,
        }
    }
}

pub fn editor(
    params: Arc<crate::params::PS88Params>,
    runtime: Arc<js::ps88js::RuntimeActor>,
) -> Option<Box<dyn Editor>> {
    let user_state = UserState::new();
    let weak_log = Arc::downgrade(&user_state.log);
    let add_logger_result = runtime.add_logger(Box::new(move |log| {
        let Some(logger) = weak_log.upgrade() else {
            return false;
        };
        logger.lock().unwrap().push((log, LogType::Info));
        return true;
    }));
    if let Err(err) = add_logger_result {
        log::error!("failed to add logger: {}", err);
    }
    create_egui_editor(
        EguiState::from_size(640 + 200, 480 + 30),
        user_state,
        |egui_ctx, _| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "RobotoMono".to_string(),
                egui::FontData::from_static(roboto_mono::ROBOTO_MONO).into(),
            );
            fonts.font_data.insert(
                "MaterialSymbolsOutlined".to_string(),
                egui::FontData::from_static(material_design_icons::MATERIAL_SYMBOLS_OUTLINED)
                    .tweak(egui::FontTweak {
                        scale: 1.00,
                        y_offset_factor: 0.16,
                        baseline_offset_factor: -0.16,
                        ..Default::default()
                    })
                    .into(),
            );
            fonts.families.clear();
            fonts.families.insert(
                egui::FontFamily::Proportional,
                vec![
                    "RobotoMono".to_string(),
                    "MaterialSymbolsOutlined".to_string(),
                ],
            );
            fonts.families.insert(
                egui::FontFamily::Monospace,
                vec![
                    "RobotoMono".to_string(),
                    "MaterialSymbolsOutlined".to_string(),
                ],
            );
            egui_ctx.set_fonts(fonts);
        },
        move |egui_ctx, _setter, state| {
            egui::TopBottomPanel::top("tab")
                .frame(egui::Frame::new())
                .exact_height(30.0)
                .show(egui_ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("\u{e037}main").clicked() {
                            state.tab = Tab::Main;
                        }
                        if ui.button("\u{e86f}code").clicked() {
                            state.tab = Tab::Code;
                        }
                    });
                });
            egui::SidePanel::right("right_panel")
                .frame(egui::Frame::new())
                .exact_width(200.0)
                .show(egui_ctx, |ui| {
                    ui.vertical(|ui| {
                        if ui.button("clear").clicked() {
                            state.log.lock().unwrap().clear();
                        }
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .max_width(400.0)
                            .show(ui, |ui| {
                                for log in state.log.lock().unwrap().iter() {
                                    ui.separator();
                                    let color = match log.1 {
                                        LogType::Info => egui::Color32::LIGHT_GRAY,
                                        LogType::Error => egui::Color32::LIGHT_RED,
                                    };
                                    let text =
                                        egui::RichText::from(&log.0).color(color).monospace();
                                    ui.label(text);
                                }
                            });
                    });
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(egui_ctx, |ui| match state.tab {
                    Tab::Main => {
                        ui.add_sized(
                            egui::Vec2::new(640., 480.),
                            CanvasWidget::new(runtime.clone(), egui_ctx),
                        );
                    }
                    Tab::Code => {
                        let mut code = params.code.lock().unwrap().clone();
                        ui.horizontal(|ui| {
                            if ui.button("open").clicked() {
                                let runtime = runtime.clone();
                                let state_watcher = state.watcher.clone();
                                let param_code = params.code.clone();
                                let weak_log = Arc::downgrade(&state.log);
                                std::thread::spawn(move || {
                                    let result = rfd::FileDialog::new().pick_file();
                                    if let Some(path) = result {
                                        if let Ok(watcher) = load_script(&path, move |code| {
                                            if let Err(err) = runtime.compile(&*code) {
                                                if let Some(logger) = weak_log.upgrade() {
                                                    logger
                                                        .lock()
                                                        .unwrap()
                                                        .push((err.to_string(), LogType::Error));
                                                }
                                            }
                                            if let Ok(mut param_code) = param_code.lock() {
                                                *param_code = code;
                                            }
                                        }) {
                                            let mut state_watcher = state_watcher.lock().unwrap();
                                            *state_watcher = Some(watcher);
                                        }
                                    }
                                });
                            }
                            let mut check = true; // TODO
                            ui.checkbox(&mut check, "hot reload");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                if ui.button("copy").clicked() {
                                    ui.ctx().copy_text(code.clone());
                                }
                                if ui.button("paste").clicked() {
                                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                        if let Ok(code) = clipboard.get_text() {
                                            if let Err(err) = runtime.compile(&*code) {
                                                state
                                                    .log
                                                    .lock()
                                                    .unwrap()
                                                    .push((err.to_string(), LogType::Error));
                                            }
                                            if let Ok(mut param_code) = params.code.lock() {
                                                *param_code = code;
                                            }
                                        }
                                    }
                                }
                            });
                        });
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut code)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                    }
                });
        },
    )
}

fn load_script<F: Fn(String) + Sync + Send + 'static>(
    path: &std::path::Path,
    callback: F,
) -> Result<Box<dyn Watcher + Send + Sync>, ()> {
    let Ok(mut file) = std::fs::File::open(&path) else {
        return Err(());
    };
    let mut code = String::new();
    file.read_to_string(&mut code).unwrap();
    callback(code);

    let mut watcher: Box<dyn Watcher + Send + Sync> = Box::new(WatcherImpl::new());
    let Ok(rx) = watcher.watch(path) else {
        return Err(());
    };
    let rx = relay_latest(rx, std::time::Duration::from_millis(100));
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
