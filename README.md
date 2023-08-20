# vst-js

DAW 上で JavaScript を実行できるようにするプラグイン。
JavaScript を使って楽器やエフェクタなどをサクサク実装できるようにする。

これまで楽器やエフェクタの開発には VST が広く使われてきたが、環境構築やらビルドなど色々と面倒臭い。
もっと 1 分で開発環境が用意できて、コードを変更したら即座に反映されるような開発体験を得たい、というのが本開発のモチベーション。

# 技術選定

VST 開発も JavaScript の実行エンジンの用意も C++ では環境構築が結構大変な印象がある。
しかし最近はどちらも Rust で実現できるらしく、 Cargo という素晴らしいパッケージ管理ツールのおかげで環境構築も楽にできてしまうらしい。
ということで今回は Rust をベースに開発する。使用するライブラリ、フレームワークは以下の通り。

- `rusty_v8` (JavaScript 実行エンジン)
- `nih-plug` (VST3 開発フレームワーク)
- `iced` (GUI フレームワーク)

# ビルド 

```
cargo xtask bundle vst-js --release
```

実行すると `target/bundled/vst-js.vst3` が生成される。

# 開発中に気になった疑問

- `nih-plug` のパラメータ等は `Send trait` 必須だが、 `rusty_v8` は `Arc` でラップ [できないみたい](https://github.com/denoland/rusty_v8/issues/643)
    - 仕方ないので VST 処理と JS 実行は別スレッドにして channel で波形など送受信する
- VST3 って panic したときどうやって stack trace 見るの？
    - Rust には [`panic::set_hook`](https://doc.rust-lang.org/std/panic/struct.PanicInfo.html#method.location) という仕組みがあり、 panic 時に任意の処理を実行できる
    - `nih-plug` には [`nih_log`](https://github.com/robbert-vdh/nih-plug/issues/25) というマクロがあり、 stderr やファイルにログ出力できる
    - これらの仕組みで stack trace を適当なファイルに書き出したりして解決できそう
