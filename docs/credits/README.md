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

PS88 のライセンスは MIT です。
依存ライブラリに、配布を妨げる条件を持つライセンスが含まれていないかを確認します。

以下は生成された Credits に含まれるライセンスの一覧です。
`MIT` には PS88 自身も含まれます。

| license | 備考 |
| ---- | ---- |
| `Apache-2.0` | permissive。特許許諾あり |
| `BSD-1-Clause` | permissive。 OSI 承認済み |
| `BSD-3-Clause` | permissive |
| `BSL-1.0` | permissive |
| `CC0-1.0` | パブリックドメイン宣言。特許ライセンスを付与しないため、 FSF はソフトウェアへの使用を非推奨としている[^1] |
| `ISC` | permissive |
| `MIT` | permissive。 PS88 自身のライセンス |
| `MPL-2.0` | **ファイル単位のコピーレフト。**元々 MPL だったファイルは MPL のままにする必要がある |
| `Unicode-3.0` | permissive。 OSI 承認済み |
| `Zlib` | permissive |

いずれも著作権表記の同梱を求めるだけで、 PS88 を MIT で配布することを妨げません。
表記は `credits.txt` を埋め込むことで満たしています。

egui のデフォルトフォントは `OFL-1.1` と `Ubuntu-font-1.0` を含むため、 `crates/ps88/Cargo.toml` で `egui` の `default-features` を無効化して依存から外しています。
描画には Roboto Mono と Material Symbols しか使っていないため影響はありません。

`crates/ps88/LICENSE` はリポジトリ直下の `LICENSE` のコピーです。
cargo-about はクレートのディレクトリ内からライセンスファイルを探すため、これが無いと MIT の
雛形が使われて `Copyright (c) <year> <copyright holders>` のまま出力されてしまいます。
重複して見えますが削除しないでください。

[^1]: Various Licenses and Comments about Them: https://www.gnu.org/licenses/license-list.html
