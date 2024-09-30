# PS88

<p align="center">
<img src="./docs/logo/logo.svg" width="240" alt="PS88 logo">
</p>

Programmable Synthesizer 88

JavaScript で波形を生成できるシンセサイザーです。  
CLAP / VST3 として動作するため、多くの作曲ソフトから利用できます。  

TODO: ここにデモの gif を載せる

```js
const audio = (ctx) => {
  // TODO: 鍵盤を押すと正弦波が鳴る、のような簡単なサンプルを書く
}
```

# インストール

TODO: 書く

# 使い方

TODO: 書く

# ビルド

## Windows / MacOS / Linux

[Rust](https://www.rust-lang.org/tools/install) をインストール後、以下のコマンドを実行します。

```
git clone https://github.com/wakewakame/ps88.git
cd ps88
cargo install --git https://github.com/robbert-vdh/nih-plug --rev dfafe90349aa3d8e40922ec031b6d673803d6432 xtask
xtask bundle ps88 --release
```

実行すると `target/bundled/` に以下が生成されます。

- `ps88.clap`
    - clap ファイル
- `ps88.vst3`
    - vst3 ファイル
- `ps88` or `ps88.exe`
    - 単独実行可能な実行ファイル
    - `ps88 -h` で使い方を表示できます
