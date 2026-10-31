#!/usr/bin/env bash
# 公開用 WASM をビルドし、配信用ディレクトリ web/ に wasm を置く。
# 開発中は使わない。反復は `cargo run`（ネイティブ）で行う。
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown

install -D -m 644 \
    target/wasm32-unknown-unknown/release/grapnl.wasm \
    web/grapnl.wasm

echo "web/grapnl.wasm を更新しました。"
echo "確認: python3 -m http.server 8080 --directory web"
echo "配布: web/ をそのまま公開する（index.html, mq_js_bundle.js, roommate-sdk.js, realtime.js, grapnl.wasm）。"
