//! ハイスコアの保存。ブラウザでは localStorage に保存し、再読み込みしても残す。
//! 読み書きは `web/storage.js` が JS 側で用意する関数で行う。
//! ネイティブ実行（開発用）では保存せず、起動している間だけ覚える。

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn high_score_load() -> f64;
    fn high_score_save(score: f64);
}

pub fn load_high_score() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { high_score_load() as u32 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

pub fn save_high_score(score: u32) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        high_score_save(score as f64)
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = score;
}
