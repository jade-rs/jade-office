#!/usr/bin/env bash
# Builds the browser prototype into crates/jade-web/www:
#   pkg/      the wasm module and its JS glue (wasm-bindgen --target web)
#   fonts/    the bundled Noto Sans JP faces the page hands to the painter
#   samples/  a few fixtures to open without picking a file
# Serve that directory over HTTP (wasm cannot be fetched from file://), e.g.
#   python3 -m http.server 8765 --directory crates/jade-web/www
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
web="$root/crates/jade-web"
www="$web/www"

cargo build --manifest-path "$web/Cargo.toml" --lib --release --target wasm32-unknown-unknown
wasm-bindgen "$web/target/wasm32-unknown-unknown/release/jade_web.wasm" \
  --out-dir "$www/pkg" --target web --no-typescript
if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    "$www/pkg/jade_web_bg.wasm" -o "$www/pkg/jade_web_bg.wasm"
fi

mkdir -p "$www/fonts" "$www/samples"
cp "$root/jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf" \
   "$root/jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Bold.ttf" "$www/fonts/"
for sample in sample-deck.pptx themed-deck.pptx charts-workbook.xlsx grid-operations-workbook.xlsx; do
  cp "$root/jade-core/public/fixtures/$sample" "$www/samples/"
done
ls -la "$www/pkg"

# The gpui shell (gpui-pre's web platform) on the same engine: www/gpui.html.
# Its crate pins a newer stable toolchain of its own (crates/jade-gpui-web).
gpui_web="$root/crates/jade-gpui-web"
(cd "$gpui_web" && cargo build --lib --release --target wasm32-unknown-unknown)
wasm-bindgen "$gpui_web/target/wasm32-unknown-unknown/release/jade_gpui_web.wasm" \
  --out-dir "$www/gpui-pkg" --target web --no-typescript
if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    "$www/gpui-pkg/jade_gpui_web_bg.wasm" -o "$www/gpui-pkg/jade_gpui_web_bg.wasm"
fi
ls -la "$www/gpui-pkg"
