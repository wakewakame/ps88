use crate::js;
use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::*;
use nih_plug_egui::egui;
use std::ops::Add;
use std::sync::Arc;

pub(super) struct CanvasWidget(Arc<js::ps88js::RuntimeActor>, egui::Context);
impl CanvasWidget {
    pub(super) fn new(runtime: Arc<js::ps88js::RuntimeActor>, egui_ctx: &egui::Context) -> Self {
        Self(runtime, egui_ctx.clone())
    }
}
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
                                        color: egui::Color32::from_rgba_unmultiplied(
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
                                        color: egui::Color32::from_rgba_unmultiplied(
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
                    let color = egui::Color32::from_rgba_unmultiplied(
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
