//! スタート画面の背景に描くジェネラティブアート。
//!
//! 3D simplex ノイズを domain warp した高さ場を作り、
//! marching squares で等高線を取り出して描いている。
//! ノイズの 3 次元目を時間軸として使うことで地形がゆっくり変化する。

use egui::{pos2, Color32, Painter, Pos2, Rect};

// -----------------------------------------------------------------------------
// 設定 (ここをいじると見た目が変わる)
// -----------------------------------------------------------------------------

// ノイズの並びを決める種
const SEED: u32 = 20260824;

// サンプリング格子の間隔[px]。小さいほど精細かつ重い
const CELL_SIZE: f32 = 30.0;

// 地形 (fBm ノイズ)
// 1px あたりのノイズ座標。小さいほど地形が大きい
const NOISE_SCALE: f32 = 0.0004;
// fBm のオクターブ数
const OCTAVES: usize = 1;
// domain warp 用ノイズの周波数
const WARP_SCALE: f32 = 0.0012;
const WARP_OCTAVES: usize = 2;
// 座標をどれだけ歪めるか[px]。0 でワープ無効
const WARP_STRENGTH: f32 = 90.0;
// ノイズの z 軸 (時間) の進む速さ
const TIME_SCALE: f32 = 0.05;

// 等高線
// 等高線の本数
const LEVELS: usize = 20;
// 高さの下端・上端 (fBm の値域は概ね ±0.5)
const ISO_MIN: f32 = -1.0;
const ISO_MAX: f32 = 1.0;

// 描画
// 低地・高地の色相
const HUE_START: f32 = 168.0;
const HUE_END: f32 = 310.0;
// 色相が 1 秒あたり回る量
const HUE_DRIFT: f32 = 5.0;
// 背景に沈ませるため彩度と明度は低めにしている
const SATURATION: f32 = 1.0;
const VALUE: f32 = 0.30;
const ALPHA: f32 = 0.8;
const LINE_WIDTH: f32 = 1.0;

// 等高線を滑らかにするときの、頂点 1 つあたりの分割数
const SMOOTH_DIV: usize = 3;

// -----------------------------------------------------------------------------
// 3D simplex ノイズ
// -----------------------------------------------------------------------------

#[rustfmt::skip]
const GRAD3: [f32; 36] = [
    1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, -1.0, 0.0, -1.0, -1.0, 0.0,
    1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, -1.0,
    0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, -1.0, 0.0, -1.0, -1.0,
];

const F3: f32 = 1.0 / 3.0;
const G3: f32 = 1.0 / 6.0;

struct Noise3 {
    perm: [usize; 512],
    perm_mod12: [usize; 512],
}

impl Noise3 {
    // シード付きの置換表を作る
    fn new(seed: u32) -> Self {
        // xorshift32: シードから再現可能な並びを作るためだけの簡易 PRNG
        let mut s = if seed == 0 { 1 } else { seed };
        let mut rand = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s as f32 / 4294967296.0
        };

        let mut p = [0u8; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        for i in (1..256).rev() {
            let j = (rand() * (i + 1) as f32) as usize;
            p.swap(i, j);
        }

        let mut perm = [0usize; 512];
        let mut perm_mod12 = [0usize; 512];
        for i in 0..512 {
            perm[i] = p[i & 255] as usize;
            perm_mod12[i] = perm[i] % 12;
        }
        Self { perm, perm_mod12 }
    }

    fn noise(&self, x: f32, y: f32, z: f32) -> f32 {
        // 立方格子を歪めて単体 (四面体) 格子に落とす
        let s = (x + y + z) * F3;
        let i = (x + s).floor();
        let j = (y + s).floor();
        let k = (z + s).floor();
        let t = (i + j + k) * G3;
        let x0 = x - (i - t);
        let y0 = y - (j - t);
        let z0 = z - (k - t);

        // 四面体のどの頂点順で進むかを決める
        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
            } else if x0 >= z0 {
                (1.0, 0.0, 0.0, 1.0, 0.0, 1.0)
            } else {
                (0.0, 0.0, 1.0, 1.0, 0.0, 1.0)
            }
        } else if y0 < z0 {
            (0.0, 0.0, 1.0, 0.0, 1.0, 1.0)
        } else if x0 < z0 {
            (0.0, 1.0, 0.0, 0.0, 1.0, 1.0)
        } else {
            (0.0, 1.0, 0.0, 1.0, 1.0, 0.0)
        };

        let (x1, y1, z1) = (x0 - i1 + G3, y0 - j1 + G3, z0 - k1 + G3);
        let (x2, y2, z2) = (x0 - i2 + 2.0 * G3, y0 - j2 + 2.0 * G3, z0 - k2 + 2.0 * G3);
        let (x3, y3, z3) = (
            x0 - 1.0 + 3.0 * G3,
            y0 - 1.0 + 3.0 * G3,
            z0 - 1.0 + 3.0 * G3,
        );

        let ii = i as i32 as usize & 255;
        let jj = j as i32 as usize & 255;
        let kk = k as i32 as usize & 255;

        // 4 頂点それぞれからの寄与を足し合わせる
        let mut n = 0.0;
        for (dx, dy, dz, gi) in [
            (
                x0,
                y0,
                z0,
                self.perm_mod12[ii + self.perm[jj + self.perm[kk]]],
            ),
            (
                x1,
                y1,
                z1,
                self.perm_mod12
                    [ii + i1 as usize + self.perm[jj + j1 as usize + self.perm[kk + k1 as usize]]],
            ),
            (
                x2,
                y2,
                z2,
                self.perm_mod12
                    [ii + i2 as usize + self.perm[jj + j2 as usize + self.perm[kk + k2 as usize]]],
            ),
            (
                x3,
                y3,
                z3,
                self.perm_mod12[ii + 1 + self.perm[jj + 1 + self.perm[kk + 1]]],
            ),
        ] {
            let mut t = 0.6 - dx * dx - dy * dy - dz * dz;
            if t > 0.0 {
                let g = gi * 3;
                t *= t;
                n += t * t * (GRAD3[g] * dx + GRAD3[g + 1] * dy + GRAD3[g + 2] * dz);
            }
        }

        32.0 * n // 概ね -1..1
    }

    /// fractional Brownian motion: 周波数 2 倍・振幅 1/2 でノイズを重ねる
    fn fbm(&self, x: f32, y: f32, z: f32, octaves: usize) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves {
            sum += amp * self.noise(x * freq, y * freq, z * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

// -----------------------------------------------------------------------------
// 等値線の交点
// -----------------------------------------------------------------------------

// 格子の辺 1 本ぶんの状態。
// 辺 ID = 水平辺 (cols-1)*rows 本 + 垂直辺 cols*(rows-1) 本 の通し番号。
#[derive(Clone, Copy)]
struct Edge {
    // 辺上の交点座標
    p: Pos2,
    // その交点が繋がる相手の辺 ID (最大 2、未使用は None)
    link: [Option<usize>; 2],
    // 何番目の等値線でこの辺を使ったか
    stamp: u32,
}

impl Default for Edge {
    fn default() -> Self {
        Self {
            p: Pos2::ZERO,
            link: [None; 2],
            stamp: 0,
        }
    }
}

/// 背景の描画に必要なバッファ類。毎フレーム作り直さずに使い回す。
pub(super) struct Background {
    noise: Noise3,
    // 格子を確保したときの描画領域 (変化したら作り直す)
    area: Rect,
    // 格子点の数
    cols: usize,
    rows: usize,
    // 格子間隔[px]
    cell_w: f32,
    cell_h: f32,
    // 高さ場
    field: Vec<f32>,
    edges: Vec<Edge>,
    // 今回使った辺の一覧
    touched: Vec<usize>,
    generation: u32,
    // 追跡中のポリライン (使い回して確保を減らす)
    poly: Vec<Pos2>,
}

impl Background {
    pub(super) fn new() -> Self {
        Self {
            noise: Noise3::new(SEED),
            area: Rect::NOTHING,
            cols: 0,
            rows: 0,
            cell_w: 0.0,
            cell_h: 0.0,
            field: vec![],
            edges: vec![],
            touched: vec![],
            generation: 0,
            poly: vec![],
        }
    }

    fn alloc_grid(&mut self, area: Rect) {
        self.area = area;
        self.cols = ((area.width() / CELL_SIZE) as usize + 1).max(2);
        self.rows = ((area.height() / CELL_SIZE) as usize + 1).max(2);
        self.cell_w = area.width() / (self.cols - 1) as f32;
        self.cell_h = area.height() / (self.rows - 1) as f32;

        self.field = vec![0.0; self.cols * self.rows];
        let edge_count = (self.cols - 1) * self.rows + self.cols * (self.rows - 1);
        self.edges = vec![Edge::default(); edge_count];
        self.touched = Vec::with_capacity(edge_count);
        self.generation = 0;
    }

    /// 高さ場を再計算する。z は時間軸 (ノイズの 3 次元目)
    fn compute_field(&mut self, z: f32) {
        for gy in 0..self.rows {
            let y = gy as f32 * self.cell_h;
            for gx in 0..self.cols {
                let x = gx as f32 * self.cell_w;
                // domain warp: 座標そのものを別のノイズでずらす
                let qx = self
                    .noise
                    .fbm(x * WARP_SCALE, y * WARP_SCALE, z, WARP_OCTAVES);
                let qy = self.noise.fbm(
                    x * WARP_SCALE + 41.7,
                    y * WARP_SCALE + 17.3,
                    z + 9.1,
                    WARP_OCTAVES,
                );
                let wx = x + WARP_STRENGTH * qx;
                let wy = y + WARP_STRENGTH * qy;
                let h = self
                    .noise
                    .fbm(wx * NOISE_SCALE, wy * NOISE_SCALE, z, OCTAVES);
                self.field[gy * self.cols + gx] = h;
            }
        }
    }

    // 辺 e の交点を登録する (同じ辺は隣のセルと共有するので一度だけ)
    fn register(&mut self, e: usize, p: Pos2) {
        if self.edges[e].stamp == self.generation {
            return;
        }
        self.edges[e] = Edge {
            p,
            link: [None; 2],
            stamp: self.generation,
        };
        self.touched.push(e);
    }

    // 2 つの交点を線分で繋ぐ
    fn link(&mut self, e1: usize, e2: usize) {
        for (e, other) in [(e1, e2), (e2, e1)] {
            let slot = if self.edges[e].link[0].is_none() {
                0
            } else {
                1
            };
            self.edges[e].link[slot] = Some(other);
        }
    }

    // e に残っている繋がり先をひとつ取り出して消費する
    fn take_link(&mut self, e: usize) -> Option<usize> {
        for slot in 0..2 {
            if let Some(next) = self.edges[e].link[slot].take() {
                return Some(next);
            }
        }
        None
    }

    fn drop_link(&mut self, e: usize, other: usize) {
        for slot in 0..2 {
            if self.edges[e].link[slot] == Some(other) {
                self.edges[e].link[slot] = None;
            }
        }
    }

    // start から線分を辿って self.poly にポリラインを組み立てる。
    // 戻り値は閉じた輪かどうか。
    fn walk(&mut self, start: usize) -> bool {
        self.poly.clear();
        self.poly.push(self.edges[start].p);
        let mut cur = start;
        loop {
            let Some(next) = self.take_link(cur) else {
                return false;
            };
            self.drop_link(next, cur);
            self.poly.push(self.edges[next].p);
            if next == start {
                return true;
            }
            cur = next;
        }
    }

    /// 高さ場から値 iso の等高線を取り出し、繋がったパス単位で emit(poly, closed) を呼ぶ。
    /// poly は呼び出しごとに使い回されるので溜め込まないこと。
    fn trace_contours(&mut self, iso: f32, mut emit: impl FnMut(&[Pos2], bool)) {
        self.generation += 1;
        self.touched.clear();

        let origin = self.area.min.to_vec2();
        let cells_x = self.cols - 1;
        let cells_y = self.rows - 1;
        // 水平辺の本数 (垂直辺 ID のオフセット)
        let h_count = cells_x * self.rows;

        for cy in 0..cells_y {
            for cx in 0..cells_x {
                let i = cy * self.cols + cx;
                let a = self.field[i]; // 左上
                let b = self.field[i + 1]; // 右上
                let c = self.field[i + self.cols + 1]; // 右下
                let d = self.field[i + self.cols]; // 左下

                let (a_in, b_in, c_in, d_in) = (a >= iso, b >= iso, c >= iso, d >= iso);
                let code = u8::from(a_in)
                    | (u8::from(b_in) << 1)
                    | (u8::from(c_in) << 2)
                    | (u8::from(d_in) << 3);
                if code == 0 || code == 15 {
                    continue;
                }

                let e_top = cy * cells_x + cx;
                let e_bottom = (cy + 1) * cells_x + cx;
                let e_left = h_count + cy * self.cols + cx;
                let e_right = h_count + cy * self.cols + cx + 1;

                let (x, y) = (cx as f32 * self.cell_w, cy as f32 * self.cell_h);
                let (w, h) = (self.cell_w, self.cell_h);

                // 符号が変わる辺だけ、線形補間で交点を求めて登録する
                if a_in != b_in {
                    let p = pos2(x + w * (iso - a) / (b - a), y) + origin;
                    self.register(e_top, p);
                }
                if b_in != c_in {
                    let p = pos2(x + w, y + h * (iso - b) / (c - b)) + origin;
                    self.register(e_right, p);
                }
                if d_in != c_in {
                    let p = pos2(x + w * (iso - d) / (c - d), y + h) + origin;
                    self.register(e_bottom, p);
                }
                if a_in != d_in {
                    let p = pos2(x, y + h * (iso - a) / (d - a)) + origin;
                    self.register(e_left, p);
                }

                match code {
                    1 | 14 => self.link(e_left, e_top),
                    2 | 13 => self.link(e_top, e_right),
                    3 | 12 => self.link(e_left, e_right),
                    4 | 11 => self.link(e_right, e_bottom),
                    6 | 9 => self.link(e_top, e_bottom),
                    7 | 8 => self.link(e_bottom, e_left),
                    // 対角のみが高いサドル。セル中心の値でどちらの繋ぎ方かを決める
                    5 | 10 => {
                        let high = (a + b + c + d) * 0.25 >= iso;
                        if high == (code == 5) {
                            self.link(e_top, e_right);
                            self.link(e_bottom, e_left);
                        } else {
                            self.link(e_left, e_top);
                            self.link(e_right, e_bottom);
                        }
                    }
                    _ => {}
                }
            }
        }

        // 画面端で途切れる開いた等高線を先に辿る (端点 = 繋がり先が 1 つだけの交点)
        for k in 0..self.touched.len() {
            let e = self.touched[k];
            if self.edges[e].link[0].is_some() && self.edges[e].link[1].is_none() {
                let closed = self.walk(e);
                if self.poly.len() >= 2 {
                    emit(&self.poly, closed);
                }
            }
        }
        // 残っているのは閉じたループ
        for k in 0..self.touched.len() {
            let e = self.touched[k];
            if self.edges[e].link.iter().any(Option::is_some) {
                let closed = self.walk(e);
                if self.poly.len() >= 2 {
                    emit(&self.poly, closed);
                }
            }
        }
    }

    /// 背景を 1 フレーム描画する。time は経過秒数。
    pub(super) fn draw(&mut self, painter: &Painter, area: Rect, time: f32) {
        if self.area != area {
            self.alloc_grid(area);
        }
        self.compute_field(time * TIME_SCALE);

        let drift = time * HUE_DRIFT;
        for li in 0..LEVELS {
            let t = if LEVELS > 1 {
                li as f32 / (LEVELS - 1) as f32
            } else {
                0.0
            };
            let iso = ISO_MIN + (ISO_MAX - ISO_MIN) * t;
            let hue = (HUE_START + (HUE_END - HUE_START) * t + drift).rem_euclid(360.0);
            let stroke = egui::Stroke::new(LINE_WIDTH, hsva(hue, SATURATION, VALUE, ALPHA));

            self.trace_contours(iso, |poly, closed| {
                let points = smooth(poly, closed);
                painter.add(if closed {
                    egui::Shape::closed_line(points, stroke)
                } else {
                    egui::Shape::line(points, stroke)
                });
            });
        }
    }
}

// -----------------------------------------------------------------------------
// 補助関数
// -----------------------------------------------------------------------------

// 色相[度]・彩度・明度・不透明度から色を作る
fn hsva(hue: f32, s: f32, v: f32, a: f32) -> Color32 {
    egui::ecolor::Hsva::new(hue / 360.0, s, v, a).into()
}

// ポリラインを、各点の中点を通る二次ベジェで滑らかにした点列に変換する
fn smooth(pts: &[Pos2], closed: bool) -> Vec<Pos2> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(n * SMOOTH_DIV + 2);

    // 二次ベジェを SMOOTH_DIV 分割して点を並べる (始点は既に out の末尾にある前提)
    let quad = |c: Pos2, p1: Pos2, out: &mut Vec<Pos2>| {
        let p0 = *out.last().unwrap();
        for i in 1..=SMOOTH_DIV {
            let t = i as f32 / SMOOTH_DIV as f32;
            let u = 1.0 - t;
            let p = p0.to_vec2() * (u * u) + c.to_vec2() * (2.0 * u * t) + p1.to_vec2() * (t * t);
            out.push(p.to_pos2());
        }
    };
    let mid = |a: Pos2, b: Pos2| a + (b - a) * 0.5;

    if closed {
        // 末尾は先頭と同じ点なので除く
        let m = n - 1;
        out.push(mid(pts[m - 1], pts[0]));
        for i in 0..m {
            quad(pts[i], mid(pts[i], pts[(i + 1) % m]), &mut out);
        }
        return out;
    }

    out.push(pts[0]);
    for i in 1..n - 1 {
        quad(pts[i], mid(pts[i], pts[i + 1]), &mut out);
    }
    out.push(pts[n - 1]);
    out
}
