# browser-game-1

[![CI](https://github.com/naosuke884/browser-game-1/actions/workflows/ci.yml/badge.svg)](https://github.com/naosuke884/browser-game-1/actions/workflows/ci.yml)

push と PR ごとに、書式・lint・テスト・WASM ビルドを GitHub Actions で確かめています（`.github/workflows/ci.yml`）。

Rust + [macroquad](https://github.com/not-fl3/macroquad) で作る、ブラウザ向け 2D アクションの土台です。

開発中はネイティブで実行します。WASM ビルドは公開するときだけ行います。macroquad の WASM は `wasm-bindgen` ではなく、同梱している専用 JS ローダー（`web/mq_js_bundle.js`）で動かします。

## 遊び方

ワイヤーを天井や浮いたブロックに刺し、振り子で右へ進み続けるエンドレスゲームです。最初は真上の天井にワイヤーが刺さった状態で始まります。

- 左クリック: カーソルの方向へワイヤーを撃つ。刺さっている間にクリックすると手元へ戻し、戻り切ったらまた撃てる
- 右クリック: 進んでいる向きへブーストする。使った後は少し待つ
- ワイヤーを敵に当てると倒せて、その場所へ引き寄せられながら上へ大きく跳ねる
- 左から赤い領域がだんだん速く迫ってくる。追いつかれるか、下へ落ちるか、敵に触れると終わり

## 必要なもの

- [rustup](https://rustup.rs/) 経由の Rust stable（この雛形は rustc 1.99.1 でビルドした）
- ネイティブ実行に必要なシステムライブラリ（Linux）

```sh
sudo apt install pkg-config libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
```

WASM ターゲットはビルドスクリプトが追加します。先に入れておく場合:

```sh
rustup target add wasm32-unknown-unknown
```

## ネイティブで実行する

```sh
cargo run
```

リサイズできるウィンドウが開きます。枠と中央の解像度表示は、ウィンドウサイズに合わせて毎フレーム配置し直します。

## WASM をビルドしてブラウザで確認する

```sh
./scripts/build-wasm.sh
python3 -m http.server 8080 --directory web
```

ブラウザで <http://127.0.0.1:8080> を開きます。`file://` では WASM を読めないので、必ず HTTP で配信します。

`web/` が配信用の一式です。

- `index.html` — キャンバスをウィンドウいっぱいに広げ、右クリックメニューを出さない
- `mq_js_bundle.js` — macroquad 0.4.16 同梱の専用ローダー（`wasm-bindgen` ではない）
- `roommate-sdk.js` — roommate-sdk。匿名 ID・スコアの送信・自己ベストを受け持つ。`Cargo.toml` の `roommate-sdk` と同じタグの `packages/roommate-sdk/dist/roommate-sdk.js` を置く
- `realtime.js` — 同じコースを同時に走っている人の影を出すため、roommate の API の部屋に WebSocket で繋ぐ
- `browser-game-1.wasm` — `./scripts/build-wasm.sh` が生成する（gitignore 済み）

main に push すると、CI が WASM をビルドし、R2 のバケット `roommate-games` の `grapnl/` に上書きで置きます。<https://play.roommate.hayashi-naoki.com/grapnl/> で遊べます。置く前に `scripts/stage-web.sh` が JS と wasm の参照に `?v=<commit>` を付け、更新後に古いファイルと新しいファイルが組み合わさらないようにします。戻すときは前のコミットに戻して push し直します。アップロードには GitHub の Environment `production` の Secret `R2_ACCOUNT_ID` / `R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` を使います。

ほかの場所に置くときは、ビルド後の `web/` ディレクトリをそのまま置きます。itch.io などに上げる場合もこの 5 ファイルです（影は roommate の配信用ドメインからしか繋がりません）。キャンバスは表示領域の 100% なので、ページや埋め込み枠のサイズ変更に追従します。

## 方針

- 反復は `cargo run`。デバッガが使え、WASM のビルド待ちを挟まない
- `cargo build --release --target wasm32-unknown-unknown` は公開用。release プロファイルはサイズ優先（LTO、`opt-level = "z"`）
- `web-sys` や `wasm-bindgen` 前提のクレートは、このローダーとはそのまま組み合わせられない
