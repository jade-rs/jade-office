# デスクトップ Word の IME 描画計測（2026-11-08）

対象は `NativeDocument` の日本語変換候補の置換、キャレット更新、表示ページと次ページのピクセル取得。GPUI の画面提示と OS の IME は含まない。

## 原因と修正

macOS `sample` では入力中の主な CPU 消費が `replace_range` 以下のページ描画に集中していた。ネイティブ側は描画済みの dirty region を消費しておらず、初回の full-repaint フラグが残っていた。これにより後続の文字編集も差分描画を利用できなかった。

- 成功したネイティブ描画の後で dirty region を消費する。描画前に消去しないため、選択表示などが要求した無効化は維持される。
- 同一段落内の IME 置換でも、通常入力と同じレイアウトキャッシュ再利用を試す。履歴の変更範囲が単一段落と確認できた場合に限る。
- この文字編集の描画中だけ既存の段落帯域描画を有効にする。高さ変更や未対応のレイアウトでは全面描画へフォールバックする。
- ネイティブの retained surface では安全な帯域計画がない限りページ全体を描画する。帯域が隣の段落のインクまで届く場合も、そのページの残りをクリップ下で描き直してから安定編集の走査を終了する。リンク領域を保持する文書では、領域レジストリの重複を避けるためネイティブ帯域描画を有効にしない。ブラウザの既定動作は変更しない。

## 計測

debug ビルド、`jade-core/contract.docx`、段落 3、ウォームアップ 10 回の後に 500 回。候補は `に → にほ → にほん → 日本 → 日本語 → にほんご → 空文字`。CPU 時間には文書の初期ロードも含む。

| 計測 | 修正前 | 修正後 |
| --- | ---: | ---: |
| user CPU 秒（500 回、最終版） | 5.70 | 2.92 |
| 編集・描画 p50 ms | 10.208 | 4.718 |
| 編集・描画 p95 ms | 20.341 | 15.706 |

user CPU 時間は約49%減少した。ウォームアップを含む510回すべてで帯域描画カウンタが増加し、全面描画へのフォールバックは0回だった。並行ビルド・テスト・追加プロファイルの負荷があるため、実時間から体感の改善率は推定しない。これらはネイティブ処理の debug 計測であり、リリース版や実機 IME の入力遅延を表す値ではない。

最終版の4,000回測定でも、ウォームアップを含む4,010回すべてで帯域描画を利用した。編集・描画 p50 は4.871 ms、p95 は20.761 ms。`sample` による5秒間のCPUスタック取得も完了した。

再現用ベンチマーク:

```sh
CARGO_TARGET_DIR="$PWD/crates/jade-gpui-app/target" cargo build \
  --manifest-path jade-core/crates/jade-docx-native-render/Cargo.toml \
  --example ime_profile
/usr/bin/time -p crates/jade-gpui-app/target/debug/examples/ime_profile \
  jade-core/contract.docx 500 3
```

CPU スタックの取得は、長めのベンチマークを起動し、標準エラーに出る `ready pid=...` の PID に対して `sample <PID> 5 1 -file /tmp/ime.sample.txt` を実行する。

## 回帰検証

最終版で以下の33件が成功した（番号付き段落1件、composition3件、native_document10件、selection_and_history8件、typing_differential11件）。ランダム操作モデルは今回の対象外とした。

IME 候補の伸長・変換・短縮・削除、折り返す長い候補、2 ページ目での編集について、差分描画と全面再描画のピクセル一致、および Undo/Redo 後の本文を検証するテストを追加した。さらに full-repaint フラグの消費と、安定した IME 置換で段落帯域描画が実際に使われることもカウンタで検証する。

```sh
CARGO_TARGET_DIR="$PWD/crates/jade-gpui-app/target" cargo test \
  --manifest-path jade-core/crates/jade-docx-native-render/Cargo.toml \
  --test native_document --test composition --test selection_and_history \
  -- --test-threads=1
CARGO_TARGET_DIR="$PWD/crates/jade-gpui-app/target" cargo test \
  --manifest-path jade-core/crates/jade-docx-native-render/Cargo.toml \
  --test typing_differential --test band_redraw_numbered \
  -- --test-threads=1 --skip _model
```

後者は IME・Unicode・選択・ページ・セクション・段組み・番号付き段落の描画差分テストを対象にし、`_model` モジュールのランダム操作テストは含めない。
