# browser-game-1

Rust + [macroquad](https://github.com/not-fl3/macroquad) で作る、ブラウザ向け 2D アクションの土台です。

開発中はネイティブで実行します。WASM ビルドは公開するときだけ行います。macroquad の WASM は `wasm-bindgen` ではなく、同梱している専用 JS ローダー（`web/mq_js_bundle.js`）で動かします。

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
- `browser-game-1.wasm` — `./scripts/build-wasm.sh` が生成する（gitignore 済み）

公開するときは、ビルド後の `web/` ディレクトリをそのまま置きます。itch.io などに上げる場合もこの 3 ファイルです。キャンバスは表示領域の 100% なので、ページや埋め込み枠のサイズ変更に追従します。

## 方針

- 反復は `cargo run`。デバッガが使え、WASM のビルド待ちを挟まない
- `cargo build --release --target wasm32-unknown-unknown` は公開用。release プロファイルはサイズ優先（LTO、`opt-level = "z"`）
- `web-sys` や `wasm-bindgen` 前提のクレートは、このローダーとはそのまま組み合わせられない
