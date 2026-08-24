mod background;

use background::Background;
use egui::{vec2, Color32, Pos2, Rect, Vec2};

/// スタート画面で選択された操作
#[derive(Clone, Copy, PartialEq)]
pub(super) enum StartAction {
    // ローカルファイルからスクリプトを読み込む
    OpenFile,
    // クリップボードからスクリプトを貼り付ける
    Paste,
    // 依存ライブラリのライセンス一覧を開く
    Licenses,
}

// 配色 (参考実装の白黒を反転している)
const BACKGROUND: Color32 = Color32::from_rgb(0x11, 0x11, 0x11);
const FILL: Color32 = Color32::from_rgb(0xcc, 0xcc, 0xcc);
const STROKE: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xaa);
const TEXT: Color32 = Color32::from_rgb(0x11, 0x11, 0x11);
const LINK: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
const LINK_HOVER: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xaa);

// ボタンの下に表示するリンク
const LINK_URL: &str = "https://github.com/wakewakame/ps88";
const LINK_LABEL: &str = "how to use";

// 角の丸い四角形の、角ひとつあたりの分割数
const CORNER_DIV: usize = 5;
// 手書き風の線の重ね描き回数
const SKETCH_REPEAT: usize = 2;

// 物理定数
// 単位
// - 距離 = 0.1 mm (100 px = 1 cm 程度と仮定)
// - 時間 = 1 s
// - 質量 = 1 kg
const MASS: f32 = 0.01; //   10 g
const SPRING: f32 = 3.5 / 10.0; //  3.5 N/mm
                                // 参考実装ではダンパと空気抵抗が別々だが、
                                // ここでは目標の形がほとんど静止しているため 1 つにまとめている
const DAMPER: f32 = 0.06 / 10.0; // 0.06 Ns/mm

struct Button {
    // ボタンの中央に表示するアイコン (Material Symbols)
    icon: &'static str,
    // アイコンの下に表示する文字列
    label: &'static str,
    // クリックされたときに返す操作
    action: StartAction,
}

const BUTTONS: [Button; 2] = [
    Button {
        icon: "\u{e2c8}", // folder_open
        label: "open .js file",
        action: StartAction::OpenFile,
    },
    Button {
        icon: "\u{e14f}", // content_paste
        label: "paste from browser",
        action: StartAction::Paste,
    },
];

/// スタート画面が保持する状態
pub(super) struct StartState {
    // 各ボタンの物理状態
    bodies: Vec<SoftBody>,
    // bodies を作ったときの描画領域 (変化したら作り直す)
    area: Rect,
    // 背景のジェネラティブアート
    background: Background,
}

impl StartState {
    pub(super) fn new() -> Self {
        Self {
            bodies: vec![],
            area: Rect::NOTHING,
            background: Background::new(),
        }
    }
}

/// 起動直後に表示する画面。
pub(super) fn start_screen(ui: &mut egui::Ui, state: &mut StartState) -> Option<StartAction> {
    // 常に形が変化し続けるため毎フレーム再描画する
    ui.ctx().request_repaint();

    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, egui::Sense::click());
    let painter = ui.painter_at(rect);

    // 1 フレームの時間が長すぎるとシミュレーションが発散するため上限を設ける
    let (time, dt) = ui.input(|i| (i.time as f32, i.stable_dt.min(1.0 / 30.0)));

    painter.rect_filled(rect, 0.0, BACKGROUND);
    state.background.draw(&painter, rect, time);

    // 画面の縦横比に応じてボタンを横並び・縦並びに切り替える
    // 隙間はボタンの幅の半分にする (中心間の距離はボタン 1.5 個分)
    let (side, gap) = if rect.width() >= rect.height() {
        let side = (rect.width() * 0.24).min(rect.height() * 0.40);
        (side, vec2(side * 0.75, 0.0))
    } else {
        let side = (rect.height() * 0.24).min(rect.width() * 0.40);
        (side, vec2(0.0, side * 0.75))
    };

    // 描画領域が変わったらボタンを配置し直す
    if state.area != rect {
        state.area = rect;
        state.bodies = vec![
            SoftBody::new(rect.center() - gap, side),
            SoftBody::new(rect.center() + gap, side),
        ];
    }

    let pointer = response.hover_pos();

    let mut action = None;
    for (button, body) in BUTTONS.iter().zip(state.bodies.iter_mut()) {
        // 当たり判定は変形後の図形に対して行う
        let hovered = pointer.is_some_and(|p| contain(&body.shape(), p));
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if response.clicked() {
                action = Some(button.action);
            }
        }
        let pressed = hovered && response.is_pointer_button_down_on();

        // ホバー中は円形に膨らませ、押している間は少しへこませる
        let target = body.target(hovered, 1.0 - 0.08 * f32::from(pressed));
        body.update(&target, dt);

        // 手書き風に少しずつずらした輪郭を重ねて描く
        let shape = body.shape();
        let normals = point_normals(&shape);
        for count in 0..SKETCH_REPEAT {
            let sketch = sketchy(&shape, &normals, count, time, side * 0.01);
            // 塗りつぶしは最後の 1 回だけ行い、その下に隠れた線がはみ出して見えるようにする
            if count + 1 == SKETCH_REPEAT {
                painter.add(fill(&sketch, FILL));
            }
            painter.add(egui::Shape::closed_line(
                sketch,
                egui::Stroke::new(side / 40.0, STROKE),
            ));
        }

        // アイコンとラベルは図形の重心に合わせて動かす
        let center = center_of(&shape);
        painter.text(
            center - vec2(0.0, side * 0.14),
            egui::Align2::CENTER_CENTER,
            button.icon,
            egui::FontId::proportional(side * 0.30),
            TEXT,
        );
        painter.text(
            center + vec2(0.0, side * 0.21),
            egui::Align2::CENTER_CENTER,
            button.label,
            egui::FontId::monospace((side * 0.08).clamp(9.0, 20.0)),
            TEXT,
        );
    }

    let font_size = (side * 0.07).clamp(9.0, 14.0);

    // 一番下のボタンのすぐ下にリンクを表示する (side * 0.6 は円形に膨らんだときの半径)
    let link_clicked = text_button(
        ui,
        &painter,
        &response,
        egui::Align2::CENTER_TOP,
        egui::pos2(rect.center().x, rect.center().y + gap.y + side * 0.68),
        LINK_LABEL,
        font_size,
        true,
    );
    if link_clicked {
        open_url(LINK_URL);
    }

    // 画面の右下にライセンス一覧を開くボタンを表示する
    let licenses_clicked = text_button(
        ui,
        &painter,
        &response,
        egui::Align2::RIGHT_BOTTOM,
        rect.max - vec2(side * 0.12, side * 0.12),
        "licenses",
        font_size,
        false,
    );
    if licenses_clicked {
        action = Some(StartAction::Licenses);
    }

    action
}

// クリックできる文字列を描画する。クリックされた場合は true を返す。
#[allow(clippy::too_many_arguments)]
fn text_button(
    ui: &egui::Ui,
    painter: &egui::Painter,
    response: &egui::Response,
    anchor: egui::Align2,
    pos: Pos2,
    text: &str,
    size: f32,
    underline: bool,
) -> bool {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::FontId::monospace(size),
        // 色は painter.galley に渡すものが使われる
        Color32::PLACEHOLDER,
    );
    let rect = anchor.anchor_size(pos, galley.size());
    let hovered = response
        .hover_pos()
        .is_some_and(|p| rect.expand(6.0).contains(p));
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let color = if hovered { LINK_HOVER } else { LINK };
    painter.galley(rect.min, galley, color);
    if underline {
        painter.hline(rect.x_range(), rect.max.y, egui::Stroke::new(1.0, color));
    }
    hovered && response.clicked()
}

// 既定のブラウザで URL を開く
// (nice-plug-egui は egui の open_url を処理しないため自前で開いている)
fn open_url(url: &str) {
    let url = url.to_owned();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut command = std::process::Command::new("cmd");
            command.args(["/c", "start", ""]);
            command
        };
        #[cfg(all(unix, not(target_os = "macos")))]
        let mut command = std::process::Command::new("xdg-open");
        if let Err(err) = command.arg(url).status() {
            log::error!("failed to open url: {}", err);
        }
    });
}

// 質点
#[derive(Clone, Copy)]
struct MassPoint {
    // 座標
    p: Pos2,
    // 速度
    v: Vec2,
}

// バネで元の形に戻ろうとする図形
struct SoftBody {
    // 変形していない状態の形
    rest: Vec<Pos2>,
    // 現在の各頂点の状態
    points: Vec<MassPoint>,
    // 図形の中心
    center: Pos2,
    // 円に変形するときの半径
    radius: f32,
}

impl SoftBody {
    fn new(center: Pos2, side: f32) -> Self {
        let rest = smooth_rect(center, side, side, side * 0.4, 0.9, CORNER_DIV);
        let points = rest
            .iter()
            .map(|p| MassPoint {
                p: *p,
                v: Vec2::ZERO,
            })
            .collect();
        Self {
            rest,
            points,
            center,
            radius: side * 0.6,
        }
    }

    // 各頂点が引き寄せられる目標の座標を計算する
    fn target(&self, round: bool, scale: f32) -> Vec<Pos2> {
        self.rest
            .iter()
            .map(|p| {
                let v = *p - self.center;
                // 頂点の対応がずれないように、円形にするときも角度は元の形のものを使う
                let v = if round {
                    Vec2::angled(v.angle()) * self.radius
                } else {
                    v
                };
                self.center + v * scale
            })
            .collect()
    }

    // 位置と速度を更新する
    fn update(&mut self, target: &[Pos2], dt: f32) {
        let mass = MASS / self.points.len() as f32;
        for (point, target) in self.points.iter_mut().zip(target) {
            // 自然長 0 のバネで目標の座標に引き寄せつつ、速度に比例した抵抗をかける
            let force = (*target - point.p) * SPRING - point.v * DAMPER;
            point.v += force * (dt / mass);
            point.p += point.v * dt;
        }
    }

    fn shape(&self) -> Vec<Pos2> {
        self.points.iter().map(|point| point.p).collect()
    }
}

// 角の丸い四角形を作る
// w: 幅, h: 高さ, r1: 角の半径, r2: 角の鋭さ (r2=(2**0.5-1)*4/3 で円弧になる), div: 角の分割数
fn smooth_rect(center: Pos2, w: f32, h: f32, r1: f32, r2: f32, div: usize) -> Vec<Pos2> {
    let r = r1 * (1.0 - r2);
    let corner = bezier(
        vec2(0.0, r1),
        vec2(0.0, r),
        vec2(r, 0.0),
        vec2(r1, 0.0),
        div,
    );
    let mut points = Vec::with_capacity(corner.len() * 4);
    for (quarter, sign) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .into_iter()
        .enumerate()
    {
        for p in corner.iter() {
            points.push(center + vec2(sign.0 * w / 2.0, sign.1 * h / 2.0) + rot90(*p, quarter));
        }
    }
    points
}

// ベジェ曲線を作る
// p1: 始点, p2: 始点側の制御点, p3: 終点側の制御点, p4: 終点, div: 分割数
fn bezier(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2, div: usize) -> Vec<Vec2> {
    (0..div + 2)
        .map(|i| {
            let t = i as f32 / (div + 1) as f32;
            let u = 1.0 - t;
            p1 * (u * u * u) + p2 * (3.0 * u * u * t) + p3 * (3.0 * u * t * t) + p4 * (t * t * t)
        })
        .collect()
}

// ベクトルを 90 度単位で回転する
fn rot90(v: Vec2, count: usize) -> Vec2 {
    match count % 4 {
        0 => v,
        1 => vec2(-v.y, v.x),
        2 => -v,
        _ => vec2(v.y, -v.x),
    }
}

// 図形の各頂点に対する法線ベクトルを取得する
fn point_normals(shape: &[Pos2]) -> Vec<Vec2> {
    let edges = shape
        .iter()
        .enumerate()
        .map(|(i, p1)| rot90(shape[(i + 1) % shape.len()] - *p1, 3).normalized())
        .collect::<Vec<Vec2>>();
    edges
        .iter()
        .enumerate()
        .map(|(i, n12)| (edges[(i + edges.len() - 1) % edges.len()] + *n12).normalized())
        .collect()
}

// 手書き風に輪郭を揺らす
fn sketchy(shape: &[Pos2], normals: &[Vec2], count: usize, time: f32, amplitude: f32) -> Vec<Pos2> {
    // 0.1 秒ごとに揺らぎ方を変えることで手書きアニメーションのように見せる
    let tick = (time as f64 * 10.0).floor();
    shape
        .iter()
        .zip(normals)
        .enumerate()
        .map(|(i, (p, normal))| {
            // TODO: 頂点の密度によってギザギザ具合が変わってしまうので、均一なゆらぎになるようにする
            let noise = (count as f64 * 1284.382235
                + i as f64 * 10284.38238
                + tick * 1285329.0482938)
                .sin() as f32;
            *p + *normal * noise * amplitude
        })
        .collect()
}

// 図形を塗りつぶす
fn fill(shape: &[Pos2], color: Color32) -> egui::Shape {
    let count = shape.len() as u32;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center_of(shape), color);
    for point in shape {
        mesh.colored_vertex(*point, color);
    }
    for i in 0..count {
        mesh.add_triangle(0, i + 1, (i + 1) % count + 1);
    }
    egui::Shape::Mesh(mesh.into())
}

// 図形の重心を取得する
fn center_of(shape: &[Pos2]) -> Pos2 {
    let sum = shape.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2());
    (sum / shape.len() as f32).to_pos2()
}

// 点 p が図形 shape の内側にあるかどうか判定する
fn contain(shape: &[Pos2], p: Pos2) -> bool {
    let mut count = 0;
    for (i, p1) in shape.iter().enumerate() {
        let p2 = shape[(i + 1) % shape.len()];
        let t = (p.y - p1.y) / (p2.y - p1.y);
        if (0.0..1.0).contains(&t) && p.x < p1.x * (1.0 - t) + p2.x * t {
            count += 1;
        }
    }
    count % 2 == 1
}
