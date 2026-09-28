# これは何

本プログラムのライセンスに関する資料です。

# Credits の生成方法

Credits (ライセンス一覧) は以下のコマンドで生成できます。

```sh
cargo install --locked cargo-about
cargo about generate -m ../../crates/ps88/Cargo.toml -o licenses.html about.hbs
```

成功すると `licenses.html` が生成されます。
(cargo-about 0.9.1 で動作確認済み)

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
| `OFL-1.1` | **GPLv3 互換ではない**[^1] ものの、 **OFL-1.1 ライセンスではないソフトウェア** に OFL ライセンスのフォントを埋め込むことは可能[^4][^5] であるため問題ない。 |
| `Ubuntu-font-1.0` | **GPLv3 互換ではない** ものの、 **UFL-1.0 ライセンスではないソフトウェア** に UFL-1.0 ライセンスのフォントを埋め込むことは可能[^2][^3] であるため問題ない。 |
| `Unicode-3.0` | 両立可能[^1] |
| `Zlib` | 両立可能[^1] |

[^1]: Various Licenses and Comments about Them: https://www.gnu.org/licenses/license-list.html
[^2]: Ubuntu font licence: https://ubuntu.com/legal/font-licence
[^3]: Ubuntu Font Family Licensing FAQ: https://ubuntu.com/legal/font-licence/faq
[^4]: The SIL Open Font License FAQ: https://openfontlicense.org/ofl-faq/
[^5]: SIL Open Font License and GPL: https://github.com/FortAwesome/Font-Awesome/issues/1124
