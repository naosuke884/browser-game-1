#!/usr/bin/env bash
# 配信用の一式を <出力先> にそろえる。CI が R2 に置く前に使う。
# index.html から読む JS と wasm に `?v=<版>` を付け、更新後に古いファイルと新しいファイルが組み合わさらないようにする。
# 配信側は `?v=` が付いたものだけ長くキャッシュし、index.html はキャッシュさせない。
# 使い方: scripts/stage-web.sh <出力先> <版>   (wasm は先に scripts/build-wasm.sh などで web/ に置いておく)
set -euo pipefail

out="$1"
version="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"

if [[ ! "$version" =~ ^[A-Za-z0-9]+$ ]]; then
    echo "版は英数字だけにする: $version" >&2
    exit 1
fi

mkdir -p "$out"
for file in index.html mq_js_bundle.js roommate-sdk.js realtime.js grapnl.wasm; do
    install -m 644 "$root/web/$file" "$out/$file"
done

# 参照を書き換える。見つからなければ止める（index.html を変えたのに追従し忘れたとき）。
for ref in 'src="mq_js_bundle.js"' 'src="roommate-sdk.js"' 'src="realtime.js"' 'load("grapnl.wasm")'; do
    if ! grep -qF "$ref" "$out/index.html"; then
        echo "index.html に $ref が見つからない" >&2
        exit 1
    fi
done
sed -i \
    -e "s|src=\"mq_js_bundle.js\"|src=\"mq_js_bundle.js?v=$version\"|" \
    -e "s|src=\"roommate-sdk.js\"|src=\"roommate-sdk.js?v=$version\"|" \
    -e "s|src=\"realtime.js\"|src=\"realtime.js?v=$version\"|" \
    -e "s|load(\"grapnl.wasm\")|load(\"grapnl.wasm?v=$version\")|" \
    "$out/index.html"
