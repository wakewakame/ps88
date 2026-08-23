use super::canvas::*;
use super::start::{start_screen, StartAction, StartState};
use crate::file_watcher::*;
use crate::js;
use nice_plug::{editor::dpi::LogicalSize, prelude::*};
use nice_plug_egui::{create_egui_editor, EguiState};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// 画面共通の余白
const MARGIN: i8 = 12;
// 画面上部のバーの高さ
const TOOL_BAR_HEIGHT: f32 = 30.0;
// ログパネルの幅
const LOG_PANEL_WIDTH: f32 = 300.0;
// キャンバスの縦横比
const CANVAS_ASPECT: f32 = 640.0 / 480.0;
// キャンバスが 640x480 になるウィンドウサイズ
const WINDOW_SIZE: (f32, f32) = (
    640.0 + MARGIN as f32 * 2.0 + LOG_PANEL_WIDTH,
    480.0 + MARGIN as f32 * 2.0 + TOOL_BAR_HEIGHT,
);

// 画面全体のモード
#[derive(PartialEq)]
enum Scene {
    // スクリプトの読み込み方法を選ぶだけの画面
    Start,
    // 依存ライブラリのライセンス一覧
    Licenses,
    // 実行中のスクリプトの内容
    Code,
    // スクリプトを実行している画面
    Run,
}

#[derive(PartialEq)]
enum LogType {
    Info,
    Error,
}

type Log = Arc<Mutex<Vec<(String, LogType)>>>;
type WatcherSlot = Arc<Mutex<Option<Box<dyn Watcher + Sync + Send>>>>;

struct UserState {
    log: Log,
    watcher: WatcherSlot,
    scene: Scene,
    start: StartState,
    // ファイルの読み込みが完了したことを別スレッドから通知するためのフラグ
    loaded: Arc<AtomicBool>,
}

impl UserState {
    fn new() -> Self {
        Self {
            log: Arc::new(Mutex::new(vec![])),
            watcher: Arc::new(Mutex::new(None)),
            scene: Scene::Start,
            start: StartState::new(),
            loaded: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn editor(
    params: Arc<crate::params::PS88Params>,
    runtime: Arc<js::ps88js::RuntimeActor>,
) -> Option<Box<dyn Editor>> {
    let user_state = UserState::new();
    let weak_log = Arc::downgrade(&user_state.log);
    runtime.add_logger(Box::new(move |log| {
        let Some(logger) = weak_log.upgrade() else {
            return false;
        };
        logger.lock().unwrap().push((log, LogType::Info));
        true
    }));
    create_egui_editor(
        EguiState::from_size(LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1)),
        user_state,
        Default::default(),
        |egui_ctx, _, _| {
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
        move |ui, _setter, _queue, state| {
            // 別スレッドでのファイル読み込みが完了していたら実行画面へ切り替える
            if state.loaded.swap(false, Ordering::Relaxed) {
                state.scene = Scene::Run;
            }

            // スクリプトの読み込み方法を選ぶ画面
            if state.scene == Scene::Start {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new())
                    .show(ui, |ui| match start_screen(ui, &mut state.start) {
                        Some(StartAction::OpenFile) => {
                            open_file(
                                runtime.clone(),
                                state.watcher.clone(),
                                params.code.clone(),
                                &state.log,
                                state.loaded.clone(),
                            );
                        }
                        Some(StartAction::Paste) => {
                            let pasted =
                                paste_code(&runtime, &params.code, &state.log, &state.watcher);
                            if pasted {
                                state.scene = Scene::Run;
                            }
                        }
                        Some(StartAction::Licenses) => {
                            state.scene = Scene::Licenses;
                        }
                        None => {}
                    });
                return;
            }

            // 依存ライブラリのライセンス一覧
            if state.scene == Scene::Licenses {
                let licenses = licenses();
                tool_bar(ui, |ui| {
                    if ui.button("\u{e5c4}back").clicked() {
                        state.scene = Scene::Start;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        if ui.button("\u{e14d}copy").clicked() {
                            copy_text(&licenses);
                        }
                    });
                });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().inner_margin(egui::Margin::same(MARGIN)))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.label(egui::RichText::new(licenses.as_str()).monospace());
                        });
                    });
                return;
            }

            // 実行中のスクリプトの内容を表示する画面
            if state.scene == Scene::Code {
                let mut code = params.code.lock().unwrap().clone();
                tool_bar(ui, |ui| {
                    if ui.button("\u{e5c4}back").clicked() {
                        state.scene = Scene::Run;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        if ui.button("\u{e14d}copy").clicked() {
                            copy_text(&code);
                        }
                    });
                });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().inner_margin(egui::Margin::same(MARGIN)))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut code)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                    });
                return;
            }

            let visuals = ui.style().visuals.clone();
            tool_bar(ui, |ui| {
                if ui.button("\u{e5c4}back").clicked() {
                    state.scene = Scene::Start;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button("\u{e86f}code").clicked() {
                        state.scene = Scene::Code;
                    }
                    // ログが流れてもエラーに気づけるように件数を表示する
                    let errors = state
                        .log
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|log| log.1 == LogType::Error)
                        .count();
                    if errors > 0 {
                        let text = format!("\u{e000}{errors}");
                        ui.label(
                            egui::RichText::new(text)
                                .color(egui::Color32::LIGHT_RED)
                                .monospace(),
                        );
                    }
                });
            });
            egui::Panel::right("log_panel")
                .frame(
                    egui::Frame::new()
                        .fill(visuals.faint_bg_color)
                        .inner_margin(egui::Margin::same(MARGIN)),
                )
                .exact_size(LOG_PANEL_WIDTH)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // 他の画面と同じく、左に見出し・右に操作を置く
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("log").monospace());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                if ui.button("clear").clicked() {
                                    state.log.lock().unwrap().clear();
                                }
                            });
                        });
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for (i, log) in state.log.lock().unwrap().iter().enumerate() {
                                    // 区切り線はエントリの間にだけ入れる
                                    if i > 0 {
                                        ui.separator();
                                    }
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
                .frame(egui::Frame::new().inner_margin(egui::Margin::same(MARGIN)))
                .show(ui, |ui| {
                    // キャンバスは縦横比を保ったまま中央に配置する
                    let area = ui.available_rect_before_wrap();
                    let size = if area.width() / area.height() > CANVAS_ASPECT {
                        egui::Vec2::new(area.height() * CANVAS_ASPECT, area.height())
                    } else {
                        egui::Vec2::new(area.width(), area.width() / CANVAS_ASPECT)
                    };
                    let rect = egui::Align2::CENTER_CENTER.align_size_within_rect(size, area);
                    ui.painter()
                        .rect_filled(rect, 0.0, visuals.extreme_bg_color);
                    // スクリプトの描画がキャンバスの外へはみ出さないようにする
                    let mut canvas_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                    canvas_ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    let canvas = CanvasWidget::new(runtime.clone(), &canvas_ui);
                    canvas_ui.add(canvas);
                    // 枠線はキャンバスの描画内容に隠れないよう最後に描く
                    ui.painter().rect_stroke(
                        rect,
                        0.0,
                        visuals.widgets.noninteractive.bg_stroke,
                        egui::StrokeKind::Inside,
                    );
                });
        },
    )
}

// 画面上部のバー
fn tool_bar(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Panel::top("tool_bar")
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(MARGIN, 0)))
        .exact_size(TOOL_BAR_HEIGHT)
        .show(ui, |ui| {
            ui.horizontal(|ui| add_contents(ui));
        });
}

// 依存ライブラリのライセンス一覧
// TODO: 実際のライセンス一覧に差し替える
fn licenses() -> String {
    (1..=100)
        .map(|i| {
            // 折り返しや横スクロールの確認のため、行ごとに長さを変えている
            format!(
                "{i:03}: {}",
                "TODO: show dependency licenses here. ".repeat(i % 5 + 1)
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

// クリップボードに文字列をコピーする
// (nice-plug-egui は egui の copy_text を処理しないため自前でコピーしている)
fn copy_text(text: &str) {
    let result = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text));
    if let Err(err) = result {
        log::error!("failed to copy text: {}", err);
    }
}

// ファイル選択ダイアログを開き、選択されたスクリプトを読み込む。
// ダイアログの表示は別スレッドで行うため、読み込みが成功したことは loaded 経由で通知する。
fn open_file(
    runtime: Arc<js::ps88js::RuntimeActor>,
    watcher: WatcherSlot,
    param_code: Arc<Mutex<String>>,
    log: &Log,
    loaded: Arc<AtomicBool>,
) {
    let weak_log = Arc::downgrade(log);
    std::thread::spawn(move || {
        let dialog = rfd::FileDialog::new().add_filter("javascript", &["js", "mjs"]);
        let Some(path) = dialog.pick_file() else {
            return;
        };
        let result = load_script(&path, move |code| {
            if let Err(err) = runtime.compile(&code) {
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
        });
        if let Ok(new_watcher) = result {
            *watcher.lock().unwrap() = Some(new_watcher);
            loaded.store(true, Ordering::Relaxed);
        }
    });
}

// クリップボードの文字列をスクリプトとして読み込む。
// クリップボードから文字列を取得できた場合に true を返す。
fn paste_code(
    runtime: &js::ps88js::RuntimeActor,
    param_code: &Mutex<String>,
    log: &Log,
    watcher: &WatcherSlot,
) -> bool {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return false;
    };
    let Ok(code) = clipboard.get_text() else {
        return false;
    };
    // 以前に開いたファイルの監視を終了する
    // (そのままにするとファイルが更新されたときに貼り付けた内容が上書きされてしまう)
    if let Ok(mut watcher) = watcher.lock() {
        *watcher = None;
    }
    if let Err(err) = runtime.compile(&code) {
        log.lock().unwrap().push((err.to_string(), LogType::Error));
    }
    if let Ok(mut param_code) = param_code.lock() {
        *param_code = code;
    }
    true
}

fn load_script<F: Fn(String) + Sync + Send + 'static>(
    path: &std::path::Path,
    callback: F,
) -> Result<Box<dyn Watcher + Send + Sync>, ()> {
    let Ok(mut file) = std::fs::File::open(path) else {
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
    Ok(watcher)
}
