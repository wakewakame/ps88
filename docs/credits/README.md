# これは何

本プログラムのライセンスに関する資料です。

# Credits の生成方法

Credits (ライセンス一覧) は以下のコマンドで生成できます。

```sh
cargo install --locked cargo-about@0.9.1
cargo about generate -m ../../crates/ps88/Cargo.toml -o ../../crates/ps88/assets/credits.txt credits.txt.hbs
```

`credits.txt` は `crates/ps88/src/gui/editor.rs` から `include_str!` で埋め込まれ、プラグインの licenses 画面に表示されます。
`Cargo.lock` を更新したら再生成してコミットしてください。

## エラーが出る場合

### `error: failed to satisfy license requirements`

エラーが出たライセンスを `about.toml` の `accepted` に追加することで解消できます。
ただし、 `accepted` に追加しようとするライセンスが問題なく使えるものかどうかを確認する必要があります。
ライセンス追加時には [#ライセンス一覧](#ライセンス一覧) も併せて修正ください。

### `unable to parse license expression for 'vst3-sys 0.1.0': GPLv3`

依存ライブラリ側の `Cargo.toml` に記載されたライセンス表記が [SPDX](https://spdx.github.io/license-list-data/) フォーマットに従っていない可能性があります。
依存ライブラリ側を修正するのは大変なので、 `about.toml` でライセンスを上書き指定して対応します。

例:

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
`GPL-3.0-or-later` は PS88 自身のもので、それ以外は依存ライブラリのものです。

| license | GPLv3 と両立可能か |
| ---- | ---- |
| `Apache-2.0` | 両立可能[^1] |
| `BSD-1-Clause` | FSF 未分類。 `BSD-3-Clause` より条項が少ないため、おそらく両立可能。 |
| `BSD-3-Clause` | 両立可能[^1] |
| `BSL-1.0` | 両立可能[^1] |
| `CC0-1.0` | 両立可能[^1]。ただし特許ライセンスを付与しないため、 FSF はソフトウェアへの使用を非推奨としている。 |
| `GPL-3.0-or-later` | PS88 自身のライセンス |
| `ISC` | 両立可能[^1] |
| `MIT` | 両立可能[^1] |
| `MPL-2.0` | §3.3 により両立可能[^1]。ただし Exhibit B の `"Incompatible With Secondary Licenses"` 通知が適用されたクレートは両立不可。 |
| `Unicode-3.0` | FSF 未分類。 OSI 承認済みの permissive ライセンス。 |
| `Zlib` | 両立可能[^1] |

egui のデフォルトフォントは `OFL-1.1` と `Ubuntu-font-1.0` を含み GPLv3 互換ではないため、 `crates/ps88/Cargo.toml` で `egui` の `default-features` を無効化して依存から外しています。
描画には Roboto Mono と Material Symbols しか使っていないため影響はありません。

PS88 のライセンスは `LICENSE` の記載どおり GPLv3 またはそれ以降のバージョンなので、 SPDX 識別子は `GPL-3.0-or-later` を使います。
非推奨の `GPL-3.0` は `GPL-3.0-only` と解釈されてしまいます。

[^1]: Various Licenses and Comments about Them: https://www.gnu.org/licenses/license-list.html
