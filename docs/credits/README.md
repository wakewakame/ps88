# これは何

本プログラムのライセンスに関する資料です。

# Credits の生成方法

Credits (ライセンス一覧) は以下のコマンドで生成できます。

```sh
cargo install --locked cargo-about@0.9.1

# プログラムに埋め込まれるテキスト版 (リポジトリにコミットする)
cargo about generate -m ../../crates/ps88/Cargo.toml -o ../../crates/ps88/assets/credits.txt credits.txt.hbs

# 目視確認用の HTML 版 (コミットしない)
cargo about generate -m ../../crates/ps88/Cargo.toml -o licenses.html about.hbs
```

**`Cargo.lock` を更新したら `credits.txt` を再生成してコミットしてください。**
`credits.txt` は `crates/ps88/src/gui/editor.rs` から `include_str!` で埋め込まれ、
プラグインの licenses 画面に表示されます。
依存が変わったのに再生成し忘れると、配布物のライセンス表記が実際の依存とずれます。

`licenses.html` はリンクや見出しがあり確認しやすいので、
ライセンスの中身を人が読みたいときに使います。こちらは `.gitignore` 済みです。

cargo-about はマイナーバージョンの更新で `about.toml` の設定項目やライセンスの
識別子が変わることがあるため、バージョンを固定しています。
更新する際は `about.toml` の追従も必要になる可能性があります。

## テンプレートを書くときの注意

handlebars の `{{value}}` は HTML エスケープされるため、テキスト版のテンプレートでは
`{{{value}}}` (三重括弧) を使う必要があります。
`{{value}}` のままだと `&lt;https://fsf.org/&gt;` のような出力になります。

## 出力に含まれる範囲

依存ライブラリだけでなく、 **PS88 自身とワークスペース内のクレートも含まれます**。

| クレート | ライセンス |
| ---- | ---- |
| `ps88` | `GPL-3.0` |
| `roboto-mono` | `Apache-2.0` |
| `material-design-icons` | `Apache-2.0` |

これは `about.toml` の `private.ignore` を `false` にしているためです。
cargo-about のデフォルトも `false` ですが、 PS88 自身の GPLv3 全文が出力から
抜け落ちるのを防ぐため、意図を明示する目的で明記しています。

配布物に GPLv3 の全文を同梱する必要があるため、この設定は変更しないでください。

## エラーが出る場合

### `error: expected a table, found boolean`

`about.toml` に cargo-about の新しいバージョンで削除されたオプションが残っている可能性があります。
未知のトップレベルキーはクレート名 (テーブル) として解釈されるため、このエラーになります。
該当のオプションを `about.toml` から削除することで解消できます。

例: `no-clearly-defined` は [cargo-about 0.8.0](https://github.com/EmbarkStudios/cargo-about/blob/main/CHANGELOG.md) で clearlydefined.io 連携ごと削除されました。

### `warning: config found for a crate not present in the graph`

`about.toml` に、依存関係から外れたクレートの設定が残っています。該当の設定を削除してください。

### `error: failed to satisfy license requirements`

エラーが出たライセンスを `about.toml` の `accepted` に追加することで解消できます。
ただし、 `accepted` に追加しようとするライセンスが問題なく使えるものかどうかを確認する必要があります。
ライセンス追加時には [#依存ライブラリのライセンス一覧](#依存ライブラリのライセンス一覧) も併せて修正ください。

### `unable to parse license expression for 'vst3-sys 0.1.0': GPLv3`

依存ライブラリ側の `Cargo.toml` に記載されたライセンス表記が [SPDX](https://spdx.github.io/license-list-data/) フォーマットに従っていない可能性があります。
依存ライブラリ側を修正するのは大変なので、 `about.toml` でライセンスを上書き指定して対応します。

例 (かつて依存していた `vst3-sys` に対する設定):

```
[vst3-sys.clarify]
license = "GPL-3.0"
[[vst3-sys.clarify.files]]
path = "license.md"
checksum = "1be76dd654024ee690864bea328622e912847461671cee0533ddf9a2cab4a31d"
```

### その他のエラー

[cargo-about Config](https://embarkstudios.github.io/cargo-about/cli/generate/config.html) を参照

# ライセンス一覧

本プログラムのライセンスは GPLv3 ですが、依存ライブラリに GPLv3 と両立しないライセンスが含まれていないかを確認します。

以下は生成された Credits に含まれるライセンスの一覧です。
`GPL-3.0` は PS88 自身のライセンスで、それ以外は依存ライブラリのものです。

| license | GPLv3 と両立可能か |
| ---- | ---- |
| `Apache-2.0` | 両立可能[^1] |
| `BSD-1-Clause` | `BSD-3-Clause` とほとんど同じ内容のため、おそらく両立可能。 |
| `BSD-3-Clause` | 両立可能[^1] |
| `BSL-1.0` | 両立可能[^1] |
| `CC0-1.0` | 両立可能[^1] |
| `GPL-3.0` | PS88 自身のライセンス |
| `ISC` | 両立可能[^1] |
| `MIT` | 両立可能[^1] |
| `MPL-2.0` | 両立可能[^1] |
| `Unicode-3.0` | 両立可能[^1] |
| `Zlib` | 両立可能[^1] |

GPLv3 と両立しないライセンスは含まれていません。

なお egui のデフォルトフォントは `OFL-1.1` と `Ubuntu-font-1.0` を含むため、
`egui/default_fonts` を無効化して依存から外しています。
詳細は [#egui のデフォルトフォントについて](#egui-のデフォルトフォントについて) を参照してください。

[^1]: Various Licenses and Comments about Them: https://www.gnu.org/licenses/license-list.html

# egui のデフォルトフォントについて

`crates/ps88/Cargo.toml` では `egui` の `default-features` を無効化しています。

```toml
egui = { version = "0.35.0", default-features = false }
```

egui のデフォルトフォント (`epaint_default_fonts`) は `OFL-1.1` と `Ubuntu-font-1.0`
のフォントを含みます。どちらも **GPLv3 互換ではありません**。
フォントの埋め込み自体はライセンス上可能[^2][^3][^4][^5] ですが、
PS88 は描画に Roboto Mono (`Apache-2.0`) と Material Symbols (`Apache-2.0`) しか
使っておらず、デフォルトフォントは `FontDefinitions` に読み込まれるだけで
一度も使われていませんでした。

そのため、不要なライセンス義務とバイナリサイズ (フォント 4 種で約 1.4 MB) を
避ける目的で依存から外しています。

egui 0.35 の `default` フィーチャは `default_fonts` のみなので、
`default-features = false` で他に失われるものはありません。

`FontDefinitions::default()` は `default_fonts` が無効だと空を返しますが、
`crates/ps88/src/gui/editor.rs` では `families` をクリアした上で自前のフォントを
登録しているため影響はありません。

[^2]: Ubuntu font licence: https://ubuntu.com/legal/font-licence
[^3]: Ubuntu Font Family Licensing FAQ: https://ubuntu.com/legal/font-licence/faq
[^4]: The SIL Open Font License FAQ: https://openfontlicense.org/ofl-faq/
[^5]: SIL Open Font License and GPL: https://github.com/FortAwesome/Font-Awesome/issues/1124
