use super::file_watcher::Watcher;
use crate::js;
use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::*;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui};
use std::io::Read;
use std::ops::Add;
use std::sync::{Arc, Mutex};

#[derive(PartialEq)]
enum Tab {
    Main,
    Code,
    Market,
}

#[derive(PartialEq)]
enum LogType {
    Info,
    Error,
}

struct UserState {
    log: Arc<Mutex<Vec<(String, LogType)>>>,
    watcher: Arc<Mutex<Option<Box<dyn super::file_watcher::Watcher + Sync + Send>>>>,
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
    // TODO: 以下のコメントアウトを外す
    //runtime
    //    .add_logger(Box::new(move |log| {
    //        let Some(logger) = weak_log.upgrade() else {
    //            return false;
    //        };
    //        logger.lock().unwrap().push((log, LogType::Info));
    //        return true;
    //    }))
    //    .unwrap();
    create_egui_editor(
        params.editor_state.clone(),
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
            egui::TopBottomPanel::top("tab").show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("\u{e037}main").clicked() {
                        state.tab = Tab::Main;
                    }
                    if ui.button("\u{e86f}code").clicked() {
                        state.tab = Tab::Code;
                    }
                    if ui.button("\u{e8b6}market").clicked() {
                        state.tab = Tab::Market
                    }
                });
            });
            if state.tab == Tab::Main || state.tab == Tab::Code {
                egui::SidePanel::right("right_panel")
                    .resizable(true)
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
            }
            egui::CentralPanel::default().show(egui_ctx, |ui| match state.tab {
                Tab::Main => {
                    ui.add(CanvasWidget(runtime.clone(), egui_ctx.clone()));
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
                                                logger.lock().unwrap().push((err.to_string(), LogType::Error));
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
                        let mut check = true;  // TODO
                        ui.checkbox(&mut check, "hot reload");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            if ui.button("copy").clicked() {
                                ui.ctx().copy_text(code.clone());
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
                Tab::Market => {
                    ui.label("This feature is under development and is currently unavailable.");
                    ui.label("In the future, it will be possible to post, search, and display rankings of works.");
                }
            });
        },
    )
}

struct CanvasWidget(Arc<js::ps88js::RuntimeActor>, egui::Context);
impl egui::Widget for CanvasWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        let offset = ui.min_rect().min.to_vec2();
        let size = ui.min_rect().size();
        let mouse = ui.input(|s| {
            let mouse = s.pointer.interact_pos().unwrap_or_default();
            (
                mouse.x - offset.x,         // x
                mouse.y - offset.y,         // y
                s.pointer.primary_down(),   // left pressed
                s.pointer.secondary_down(), // right pressed
            )
        });
        let gui_args = js::ps88js::GuiArgs {
            w: size.x as f64,
            h: size.y as f64,
            mouse_x: mouse.0 as f64,
            mouse_y: mouse.1 as f64,
            pressed_l: mouse.2,
            pressed_r: mouse.3,
        };
        let shapes = self.0.gui(gui_args);
        let shapes = match shapes {
            Ok(shapes) => shapes,
            Err(err) => {
                log::error!("failed to call gui: {}", err);
                return response;
            }
        };

        let mut fill_tessellator = FillTessellator::new();
        let mut stroke_tessellator = StrokeTessellator::new();
        for shape in shapes.iter() {
            match shape {
                js::ps88js::Shape::Polygon {
                    path,
                    fill,
                    stroke,
                    stroke_width,
                    stroke_closed,
                } => {
                    let mut builder = Path::builder();
                    for (i, p) in path.iter().enumerate() {
                        let p = (p.0 as f32, p.1 as f32);
                        if i == 0 {
                            builder.begin(point(p.0 + offset.x, p.1 + offset.y));
                        } else {
                            builder.line_to(point(p.0 + offset.x, p.1 + offset.y));
                        }
                    }
                    builder.end(stroke_closed.unwrap_or(false));
                    let path = builder.build();
                    if let Some(fill) = fill {
                        let mut geometry: VertexBuffers<egui::epaint::Vertex, u32> =
                            VertexBuffers::new();
                        fill_tessellator
                            .tessellate_path(
                                &path,
                                &FillOptions::default(),
                                &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                                    egui::epaint::Vertex {
                                        pos: egui::pos2(vertex.position().x, vertex.position().y),
                                        uv: egui::epaint::WHITE_UV,
                                        color: egui::Color32::from_rgba_premultiplied(
                                            (fill >> 24) as u8,
                                            ((fill >> 16) & 0xff) as u8,
                                            ((fill >> 8) & 0xff) as u8,
                                            (fill & 0xff) as u8,
                                        ),
                                    }
                                }),
                            )
                            .unwrap();
                        let mesh = egui::Shape::Mesh(
                            egui::Mesh {
                                indices: geometry.indices,
                                vertices: geometry.vertices,
                                // デフォルトのテクスチャは egui::epaint::WHITE_UV の座標が白色であることが保証されている。
                                // そしてテクスチャは Vertex.color と乗算されるため、この場合は Vertex.color がそのまま反映される。
                                texture_id: egui::TextureId::default(),
                            }
                            .into(),
                        );
                        painter.add(mesh);
                    }
                    if let Some(stroke) = stroke {
                        let mut geometry: VertexBuffers<egui::epaint::Vertex, u32> =
                            VertexBuffers::new();
                        stroke_tessellator
                            .tessellate_path(
                                &path,
                                &StrokeOptions::default()
                                    .with_line_width(stroke_width.unwrap_or(1.0) as f32)
                                    .with_line_join(LineJoin::Bevel),
                                &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                                    egui::epaint::Vertex {
                                        pos: egui::pos2(vertex.position().x, vertex.position().y),
                                        uv: egui::epaint::WHITE_UV,
                                        color: egui::Color32::from_rgba_premultiplied(
                                            (stroke >> 24) as u8,
                                            ((stroke >> 16) & 0xff) as u8,
                                            ((stroke >> 8) & 0xff) as u8,
                                            (stroke & 0xff) as u8,
                                        ),
                                    }
                                }),
                            )
                            .unwrap();
                        let mesh = egui::Shape::Mesh(
                            egui::Mesh {
                                indices: geometry.indices,
                                vertices: geometry.vertices,
                                // デフォルトのテクスチャは egui::epaint::WHITE_UV の座標が白色であることが保証されている。
                                // そしてテクスチャは Vertex.color と乗算されるため、この場合は Vertex.color がそのまま反映される。
                                texture_id: egui::TextureId::default(),
                            }
                            .into(),
                        );
                        painter.add(mesh);
                    }
                }
                js::ps88js::Shape::Text {
                    text,
                    x,
                    y,
                    size,
                    color,
                } => {
                    let color = color.unwrap_or(0x000000FF);
                    let color = egui::Color32::from_rgba_premultiplied(
                        (color >> 24) as u8,
                        ((color >> 16) & 0xff) as u8,
                        ((color >> 8) & 0xff) as u8,
                        (color & 0xff) as u8,
                    );
                    let text = egui::Shape::Text(egui::epaint::TextShape::new(
                        egui::pos2(*x as f32, *y as f32).add(offset),
                        self.1.fonts(|fonts| {
                            fonts.layout_job(egui::text::LayoutJob::simple_singleline(
                                text.clone(),
                                egui::FontId::monospace(size.unwrap_or(12.0) as f32),
                                color,
                            ))
                        }),
                        egui::Color32::TRANSPARENT,
                    ));
                    painter.add(text);
                }
            }
        }
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
