# PS88 コードレビュー

対象: `review` ブランチ (7a4f882) / crates/ps88 全ソース + リポジトリ構成
検証: `cargo check` (警告2件) / `cargo clippy` (警告28件) / `cargo test` (12件全て成功)

全体として、コードは読みやすく整理されており、特に JS ランタイム層 (`js/core`, `js/ps88js`) は
テストが充実していて品質が高いです。unsafe 箇所に安全性の根拠コメントが付いているのも良い習慣です。
一方で、**オーディオスレッドのリアルタイム安全性**と**ユーザー入力起因のパニック(=ホストDAWのクラッシュ)**
に関わる問題がいくつかあり、これらは優先的に対処する価値があります。

凡例: 🔴 重大 / 🟡 要検討 / 🔵 軽微・スタイル

---

## 1. アーキテクチャ・設計レベル

### 🔴 1-1. ユーザー JS の無限ループでオーディオスレッドが永久ブロックする
`RuntimeActor::audio` (`js/ps88js/runtime_actor.rs:137`) は `rx.recv()` で JS の完了を**無期限に**待ちます。
ユーザーが `ps88.audio(() => { while(true){} })` と書くと、オーディオスレッドが返ってこなくなり
ホスト DAW ごとフリーズします。「任意のコードをユーザーに書かせる」プラグインなので、これは
通常のプラグインより起きやすい事故です。

対策案:
- `recv_timeout` + タイムアウト時は無音を返す
- ウォッチドッグスレッドから `v8::IsolateHandle::terminate_execution()` で JS を強制中断
  (rusty_v8 でサポートされており、`TryCatch` 側は `has_terminated` で区別できます)

### 🔴 1-2. `assert_process_allocs` が有効なのに `process()` 内でアロケーションしている
`Cargo.toml:18` で `assert_process_allocs` を有効にしていますが、`process()` の経路には
アロケーションが多数あります:

- `lib.rs:106` — `Vec::<NoteEvent>::new()` + `push` (MIDI イベント受信時に確保)
- `runtime_actor.rs:128` — 呼び出しごとの `channel()` 生成
- `runtime_actor.rs:117` — バッファ長変化時の `to_vec()`

この feature はデバッグビルドで process 中のアロケーションを検出して panic させるものなので、
デバッグビルドでプラグインとして動かすと最初の `process()` で落ちるはずです。
「リアルタイム安全を目指す」なら以下を、そうでないなら feature を外して方針を明示するのが良いです。

- MIDI バッファは固定長 (`ArrayVec` など) を事前確保して再利用
- 応答チャンネルは毎回作らず、事前生成したものを使い回す

### 🔴 1-3. アクタースレッドの panic が連鎖的にホストをクラッシュさせる
`runtime_actor.rs:27` — アクタースレッド内の `Runtime::new(userdata).unwrap()` が失敗すると
スレッドが panic で死にます。すると以降のすべての `self.sender.send(...).unwrap()`
(同 74, 84, 91, 130, 153 行) が呼び出し側スレッド (= オーディオ / GUI スレッド) で panic し、
ホストがクラッシュします。

- `RuntimeActor::new` は `core::Result<Self>` を返す設計なのに、実際には `Err` を返す経路がなく、
  失敗はスレッド内 panic に化けています。初期化はスレッド起動後に結果をチャンネルで受け取り、
  `new` から `Err` として返すのが素直です。
- 各メソッドの `send/recv` の `unwrap()` も、アクター死亡時は `Err(JsRuntimeError::...)` に
  変換して返す方が安全です。

### 🟡 1-4. GUI とオーディオが単一アクタースレッドを奪い合う
`gui/canvas.rs:37` は毎フレーム `runtime.gui()` を同期呼び出しし、オーディオと同じスレッドで
JS を実行します。重い `gui` コールバックはそのまま音切れになります。単一 Isolate 上で
audio/gui の状態を共有する設計自体は合理的なので、すぐ変えるべきとは言いませんが:

- 少なくとも「gui が重いと音が途切れる」制約として README / docs に明記する
- 将来的には gui 呼び出しを低優先度化する (audio メッセージを優先処理するキュー) 余地があります

なお `docs/figure/architecture.md` の構成図は input_queue / output_queue モデルで描かれており、
現在の「呼び出しごとの応答チャンネル + 共有 args」という実装と乖離しています (TODO 済みですが)。

### 🟡 1-5. `JsRuntime::run` が drop 済み HandleScope の `v8::Local` を返している
`js/core/runtime.rs:138-148` — `run()` は関数内で `HandleScope` / `TryCatch` を作り、
そこで得た `v8::Local<v8::Value>` を返しています。ライフタイムは `&mut self` に推論されて
コンパイルは通りますが、HandleScope が pop された後のハンドルを外で使うのは V8 的には
無効ハンドルであり、健全性が怪しいパターンです (現状テストが通るのは偶然の域)。
戻り値を `v8::Global<v8::Value>` にするか、値をスコープ内で Rust 型に変換して返すのが安全です。
`cargo check` の lifetime 警告 2 件もこの関数と `scope()` に対するものです。

### 🔴 1-6. `src/runtime.rs` はデッドファイル
`lib.rs` は `file_watcher / gui / js / params` しか宣言しておらず、`src/runtime.rs`
(存在しない `js_sync` モジュールを参照) はどこからも使われていません。削除しましょう。

### 🟡 1-7. VST3 サブカテゴリが「エフェクト」になっている
`lib.rs:214-215` — `Vst3SubCategory::Fx, Tools` ですが、これはシンセなので
`Vst3SubCategory::Instrument` (+ `Synth`) が正しいはずです。CLAP 側 (`ClapFeature::Instrument,
Synthesizer`) と矛盾しており、DAW によってはインストゥルメント一覧に出てきません。
また `CLAP_ID: "ps88"` は逆ドメイン形式 (`com.github.wakewakame.ps88` 等) が慣例です。

### 🟡 1-8. MIDI の扱い: CC を黙って捨てる / 入力をそのままエコーする
- `lib.rs:139-140` — `MidiConfig::MidiCCs` を宣言しているのに NoteOn/NoteOff 以外
  (CC, PolyPressure 等) は黙って破棄されます。JS に渡さないなら `MidiConfig::Basic` に
  落とすか、TODO の通り対応するかを揃えたほうが誠実です。
- `lib.rs:160-193` — JS が midi 配列を変更しなかった場合、入力イベントがそのまま出力に
  再送されます。ホストのルーティングによってはノートの二重発音になり得ます。
  意図的な仕様なら OK ですが、明示が欲しいところです。

---

## 2. バグ・正確性

### 🔴 2-1. ファイルオープンで非 UTF-8 ファイルを選ぶと panic
`gui/editor.rs:212` と `gui/editor.rs:228` — `file.read_to_string(&mut code).unwrap()`。
ファイルダイアログでユーザーは任意のファイルを選べるため、バイナリファイルを選ぶだけで
スレッドが panic します(GUI 上は「何も起きない」ように見えて監視スレッドが静かに死ぬ)。
`Err` はログパネルに出すべきです。同関数の戻り値 `Result<_, ()>` もエラー情報を捨てているので、
`file_watcher::Error` などを流用して理由を返すと UI に出せます。

### 🟡 2-2. runtime_actor のテストが RuntimeActor をテストしていない
`js/ps88js/runtime_actor.rs:349` — `test_save_load` が `Runtime::new(...)` を使っており
(コピペ由来と思われます)、アクター経由の save/load は未検証です。`RuntimeActor::new` に
変えるとスレッド境界越しの `UserData` 共有がテストできます。

### 🟡 2-3. テッセレーション失敗で JS ランタイムを reset するのは過剰
`gui/canvas.rs:89, 129` — lyon の tessellate が失敗すると `runtime.reset()` を呼び、
ユーザースクリプトの全状態 (audio コールバック含む) を破壊します。描画側の失敗で
音まで止めるのは影響が大きすぎるので、その Shape をスキップしてログを出すだけで十分です。
また `canvas.rs:128` のエラーメッセージが stroke 側なのに `"failed to tessellate fill"` に
なっています(コピペミス)。

### 🟡 2-4. 「hot reload」チェックボックスがダミー
`gui/editor.rs:165-166` — `let mut check = true; // TODO` で、見た目は操作できるのに
何も制御していません。未実装なら `ui.add_enabled(false, ...)` で無効表示にするか、
消しておかないとユーザーを混乱させます。

### 🟡 2-5. ログが無制限に溜まる
`gui/editor.rs:22` の `log: Vec<(String, LogType)>` は clear ボタン以外で減りません。
`console.log` を audio コールバック内で毎ブロック呼ぶスクリプトだと、数時間で
数百 MB 級に育ちます。リングバッファ (`VecDeque` + 上限) にするのが安全です。

### 🔵 2-6. マウス座標: ポインタ非存在時に変な値が JS に渡る
`gui/canvas.rs:21` — `interact_pos().unwrap_or_default()` は (0,0) を返すため、offset を引くと
負の座標が `ctx.mouse` に渡ります。JS 側で「ポインタなし」を区別できるよう `null` を渡すか、
最後の有効座標を保持するのが親切です。

### 🔵 2-7. テキストのデフォルト色が黒
`gui/canvas.rs:154` — `color.unwrap_or(0x000000FF)`。egui のダークテーマ背景では
ほぼ見えません。テーマ前景色か白をデフォルトにする方が実用的です。

### 🔵 2-8. `ctx.bpm` が 0 になり得ることが未文書
`lib.rs:152` — `transport.tempo.unwrap_or(0.0)`。テンポ未提供時に 0 が JS に渡るので、
JS API ドキュメントに明記するか `null` を渡す方が扱いやすいです。

---

## 3. コード品質・簡素化

### 🟡 3-1. NoteEvent の変換が往復とも手書きで重複 (`lib.rs:105-193`)
nih_plug の `NoteEvent` ↔ 自前 `NoteEvent` の変換 match が受信側と送信側でほぼ同じ形で
2 回書かれています。`impl TryFrom<nih_plug::NoteEvent<()>> for js::ps88js::NoteEvent` と
逆方向の `From` を用意すると、`process()` が大幅に短くなり、イベント種別を増やすときの
変更箇所も 1 箇所になります。

### 🟡 3-2. `gui_api.rs` の add_polygon / add_text はほぼ同じ構造の重複
両者とも「serde_v8 で引数をデコード → data() の Array に push」で、80% 同じコードです。
`#[derive(Deserialize)]` した引数タプル + 汎用ヘルパー
(`fn add_shape_fn<Args: Deserialize>(scope, shapes, build: fn(Args) -> Shape)`) に
まとめられます。`type Arg0 = String;` のような別名も、素直に変数名で表現した方が読みやすいです。

### 🟡 3-3. `canvas.rs` の fill / stroke 描画と色変換の重複
`u32 (0xRRGGBBAA) → Color32` の変換が 3 回コピペされています (`canvas.rs:78, 118, 155`)。
`fn color32(c: u32) -> egui::Color32` を切り出し、mesh 構築部分も
`fn tessellate(path, tessellator, color) -> Option<egui::Shape>` にまとめると
ループ本体が半分以下になります。

### 🟡 3-4. logger の「二重 replace」は意図がコメントなしでは読めない
`js/ps88js/runtime.rs:37-45`:
```rust
logger2.replace(logger2.replace(vec![]).into_iter().filter(...).collect())
```
おそらく「ロガー実行中に再入して RefCell の borrow が衝突するのを避けるため、一旦取り出して
実行後に書き戻す」意図だと思いますが、初見では読めません。意図をコメントで残すか、
`Vec` を `borrow_mut().retain(...)` できない理由を書いてください。なお実行中に
`add_logger` されたロガーは外側の `replace` で失われる edge case があります。

### 🟡 3-5. エラー時 reset のエラーが元エラーを隠す
`js/ps88js/runtime.rs:115-119, 163-167` — `if result.is_err() { self.reset()?; }` は、
reset 自体が失敗すると**元の JS エラーを捨てて** reset のエラーを返します。
`self.reset().and(result)` ではなく、reset 失敗はログに落として `result` を返す方が
デバッグしやすいです。

### 🔵 3-6. `RuntimeActor::drop` のダミー swap
`runtime_actor.rs:159-164` — drop のためだけにダミーの channel と thread を生成しています。
`handle: Option<JoinHandle<()>>` にして `take()` すれば、無駄なスレッド生成なしで書けます。
`handle.join().unwrap()` はアクターが panic 済みだと drop 中の二重 panic → abort になるので
`let _ = handle.join();` が安全です。

### 🔵 3-7. `convert.rs` の細かい点
- `convert.rs:104` — `float32arrays.clone()` は不要です (`iter().map(|f| (*f).into())` か、
  そもそも最初から `Vec<v8::Local<Value>>` で集める)。
- `convert.rs:81, 127` — src/dst は重ならないので `std::ptr::copy_nonoverlapping` が明確です。
- `convert.rs:38` — `midi_to_obj` の `&mut Vec<NoteEvent>` は `&[NoteEvent]` で足ります。
- `convert.rs:67` — アラインメントへの不安がコメントされていますが、`v8::ArrayBuffer` の
  backing store は最低でも 8 byte アラインなので f32 (align 4) は安全です。確認済みの事実として
  コメントを書き換えられます。

### 🔵 3-8. 命名・小物
- `js/core/runtime.rs:56` — `PUPPY_INIT` は別プロジェクト由来と思われる名前です (`V8_INIT` へ)。
- `js/core/runtime.rs:103` — `pub fn reset<'b>(&'b mut self)` の `'b` は不要です。
- `JsRuntimeBuilder::new` / `WatcherImpl::new` — `Default` も実装しておくと clippy が黙ります。
- `error.rs:15` の `wrap_err` は `map_err(|e| JsRuntimeError::UnexpectedError(e.to_string()))`
  の別名でしかないので、使用箇所で直接書くか `From` 実装の方が Rust らしいです。
- `CanvasWidget(Arc<...>, egui::Context)` (`canvas.rs:9`) — タプル構造体の `.0` `.1` より
  名前付きフィールドが読みやすいです。また `egui::Context` は `Widget::ui` 内で
  `ui.ctx()` から取れるので、保持する必要がありません。
- `params.rs:5` — `&'static str` の `'static` は const では冗長です (clippy 指摘)。
- clippy 警告 28 件のうち 24 件は `cargo clippy --fix` で自動修正できます
  (`unneeded return` 8 件、自動 deref、`format!` の不要使用など)。一度流す価値があります。

### 🔵 3-9. `Api::add` の ZST クロージャ + `mem::zeroed` トリック
`js/core/api.rs:43-92` — コンパイル時 `size_of::<F>() == 0` assert で捕捉不能クロージャを
弾いてから `mem::zeroed::<F>()` で再構成する手法は正しく、参照コメントもあって良いです。
ただ「なぜ引数 `_: F` の値を捨てるのか」「キャプチャありだとどんなコンパイルエラーが出るのか」
は利用者視点で分かりにくいので、`add` の doc コメントに一言あると親切です。

### 🔵 3-10. `UserData::Bytes` の永続化形式
nih-plug の `#[persist]` は JSON でシリアライズされるため、`Vec<u8>` は
`[1,2,3,...]` の数値配列になります。ユーザーが大きなデータ (サンプル波形等) を `save()`
すると保存サイズと速度が大きく劣化します。`serde_bytes` や base64 文字列化を検討してください。

---

## 4. リポジトリ運用・ビルド

### 🟡 4-1. `Cargo.lock` を ignore している
`.gitignore` で `/Cargo.lock` を除外していますが、これはライブラリの慣習です。
ps88 は配布バイナリ (プラグイン) なので、再現可能なビルドのために **コミットするのが推奨**
(現在の cargo 公式ガイダンスも、バイナリはロックファイルをコミットする方針) です。

### 🟡 4-2. CI がない
GitHub Actions で `cargo fmt --check` / `clippy -D warnings` / `cargo test` を回すだけでも、
今回 clippy が拾った 28 件のような退行を防げます。V8 のビルドが重い場合も、
rusty_v8 はプリビルドバイナリを落とすのでキャッシュありなら現実的な時間で回ります。

### 🔵 4-3. README のビルド手順の順序
`README.md` / `README.ja.md`:
```sh
git clone https://github.com/wakewakame/ps88.git
git submodule update --init --recursive   # ← cd ps88 の前に実行している
cd ps88
```
`git submodule update` はリポジトリ内で実行する必要があるので `cd ps88` の後です。
そもそも `.gitmodules` が見当たらないので、サブモジュールが本当に必要なければこの行ごと
削除できます。

### 🔵 4-4. その他
- `.DS_Store` が多数あります (グローバル gitignore で無視されている状態)。他の macOS
  コントリビュータのために、リポジトリの `.gitignore` にも `.DS_Store` を足すのが無難です。
- `crates/ps88/Cargo.toml:6` — `license = "GPL-3.0"` 直書きは `license.workspace = true` に
  統一できます (version / edition は workspace 継承済み)。
- `TODO.md` と `docs/todo/README.md` の二重管理になっています。片方 (GitHub Issues でも可) に
  寄せると迷子になりません。
- `mockall` が dev-dependencies にあり `file_watcher.rs:1` で `automock` していますが、
  `MockWatcher` を使うテストが存在しません。使う予定がなければ属性ごと外せます
  (mockall はコンパイル時間もそれなりに食います)。
- 依存の `deno_core` は v8 + serde_v8 のためだけに使っているように見えます。
  `v8` / `serde_v8` クレートへの直接依存に置き換えられれば、依存ツリーとビルド時間を
  削れる可能性があります (serde_v8 の単体公開状況は要確認)。

---

## 5. テスト

- `js/core` / `js/ps88js` のテストは入出力・エラー・リセット・ライフサイクル (drop 検証まで!)
  を押さえていて模範的です。
- 一方 `lib.rs::process` (イベント変換・書き戻し)、`file_watcher` の `WatcherImpl`、
  `gui/canvas` の色変換などはテストがありません。特に 3-1 の `From` 実装を導入すれば
  イベント変換は純粋関数になり、そのままテスト可能になります。
- 2-2 の通り、`runtime_actor.rs` の `test_save_load` は修正が必要です。

---

## 推奨アクション優先順

1. JS 無限ループ対策 (`recv_timeout` + `terminate_execution`) — 1-1
2. アクター初期化失敗と send/recv unwrap の panic 連鎖解消 — 1-3
3. `read_to_string().unwrap()` の除去 — 2-1
4. process() のアロケーション排除 or `assert_process_allocs` の方針決定 — 1-2
5. VST3 サブカテゴリ修正 — 1-7
6. デッドファイル削除 (`src/runtime.rs`)、clippy --fix、テスト修正 (2-2) — 小粒でも即効
7. Cargo.lock コミット + CI 導入 — 4-1, 4-2
