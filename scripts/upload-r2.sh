#!/usr/bin/env bash
# scripts/stage-web.sh でそろえた一式を、R2 の roommate-games/browser-game-1/ に上書きで置く。
# 要る環境変数: R2_ACCOUNT_ID と、AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY（R2 の API トークン）。
# 使い方: scripts/upload-r2.sh <一式の場所>
set -euo pipefail

dir="$1"
endpoint="https://${R2_ACCOUNT_ID:?}.r2.cloudflarestorage.com"
dest="s3://roommate-games/browser-game-1"

put() {
    aws s3 cp --endpoint-url "$endpoint" --no-progress "$dir/$1" "$dest/$1" --content-type "$2"
}

# 新しい index.html が、まだ置いていないファイルを読みに行かないよう、index.html は最後に置く。
put mq_js_bundle.js "text/javascript; charset=utf-8"
put brand-sdk.js "text/javascript; charset=utf-8"
put realtime.js "text/javascript; charset=utf-8"
put browser-game-1.wasm "application/wasm"
put index.html "text/html; charset=utf-8"
