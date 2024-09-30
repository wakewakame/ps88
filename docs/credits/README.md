# これは何

本プログラムのライセンスに関する資料です。

# Credits の生成方法

Credits (依存ライブラリのライセンス一覧) は以下のコマンドで生成できます。

```sh
cargo install --locked cargo-about
cargo about generate -m ../../Cargo.toml -o licenses.html about.hbs
```

成功すると `licenses.html` が生成されます。

## エラーが出る場合

### `error: failed to satisfy license requirements`

エラーが出たライセンスを `about.toml` の `accepted` に追加することで解消できます。
ただし、 `accepted` に追加しようとするライセンスが問題なく使えるものかどうかを確認する必要があります。
ライセンス追加時には [#依存ライブラリのライセンス一覧](#依存ライブラリのライセンス一覧) も併せて修正ください。

### `unable to parse license expression for 'vst3-sys 0.1.0': GPLv3`

依存ライブラリ側の `Cargo.toml` に記載されたライセンス表記が [SPDX](https://spdx.github.io/license-list-data/) フォーマットに従っていない可能性があります。
依存ライブラリ側を修正するのは大変なので、 `about.toml` でライセンスを上書き指定して対応します。

例:

```
[vst3-sysa.clarify]
license = "GPL-3.0"
[[vst3-sysa.clarify.files]]
path = "license.md"
checksum = "1be76dd654024ee690864bea328622e912847461671cee0533ddf9a2cab4a31d"
```

### その他のエラー

[cargo-about Config](https://embarkstudios.github.io/cargo-about/cli/generate/config.html) を参照

# 依存ライブラリのライセンス一覧

本プログラムのライセンスは GPLv3 ですが、依存ライブラリに GPLv3 と両立しないライセンスが含まれていないかを確認します。

以下は依存ライブラリのライセンス一覧です。

| license | GPLv3 と両立可能か |
| ---- | ---- |
| `Apache-2.0` | 両立可能[^1] |
| `BSD-1-Clause` | `BSD-3-Clause` とほとんど同じ内容のため、おそらく両立可能。 |
| `BSD-3-Clause` | 両立可能[^1] |
| `BSL-1.0` | 両立可能[^1] |
| `CC0-1.0` | 両立可能[^1] |
| `GPL-3.0` | 両立可能[^1] |
| `ISC` | 両立可能[^1] |
| `LicenseRef-UFL-1.0` | **GPLv3 互換ではない** ものの、 **UFL-1.0 ライセンスではないソフトウェア** に UFL-1.0 ライセンスのフォントを埋め込むことは可能[^2][^3] であるため問題ない。 |
| `MIT` | 両立可能[^1] |
| `OFL-1.1` | **GPLv3 互換ではない**[^1] ものの、 **OFL-1.1 ライセンスではないソフトウェア** に OFL ライセンスのフォントを埋め込むことは可能[^4][^5] であるため問題ない。 |
| `Unicode-DFS-2016` | 両立可能[^1] |
| `Zlib` | 両立可能[^1] |

[^1]: Various Licenses and Comments about Them: https://www.gnu.org/licenses/license-list.html
[^2]: Ubuntu font licence: https://ubuntu.com/legal/font-licence
[^3]: Ubuntu Font Family Licensing FAQ: https://ubuntu.com/legal/font-licence/faq
[^4]: The SIL Open Font License FAQ: https://openfontlicense.org/ofl-faq/
[^5]: SIL Open Font License and GPL: https://github.com/FortAwesome/Font-Awesome/issues/1124
