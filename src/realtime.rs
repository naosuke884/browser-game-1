//! 影の部屋との通信。ブラウザでは `web/realtime.js` が WebSocket で部屋に繋ぎ、バイト列をやり取りする。
//! ネイティブ実行（開発用）では繋がず、送っても何も起きず、何も届かない。
//! 繋がらないときも同じで、ゲームは影なしで続けられる。

/// 1 件のメッセージの大きさの上限。部屋は 20 人 × 100ms ぶんをまとめて送るので、それより十分大きくする。
#[cfg(target_arch = "wasm32")]
const MAX_MESSAGE: usize = 64 * 1024;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn realtime_join(key: *const u8, len: u32);
    fn realtime_send(data: *const u8, len: u32);
    fn realtime_next_len() -> i32;
    fn realtime_take(data: *mut u8, len: u32);
}

/// 部屋 `key` に入る。別の部屋にいれば出てから入る。
pub fn join(key: &str) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        realtime_join(key.as_ptr(), key.len() as u32)
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = key;
}

/// 部屋の他の人へ送る。繋がっていなければ捨てる。
pub fn send(data: &[u8]) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        realtime_send(data.as_ptr(), data.len() as u32)
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = data;
}

/// 届いたメッセージを 1 件取り出す。先頭の 1 バイトが種類（`shadow::read_incoming` を参照）。
pub fn receive() -> Option<Vec<u8>> {
    #[cfg(target_arch = "wasm32")]
    {
        let len = unsafe { realtime_next_len() };
        if len < 0 {
            return None;
        }
        let len = len as usize;
        // 大きすぎるものも取り出して捨てる。取り出さないと次が読めない。
        let mut buffer = vec![0; len.min(MAX_MESSAGE)];
        unsafe { realtime_take(buffer.as_mut_ptr(), buffer.len() as u32) };
        (len <= MAX_MESSAGE).then_some(buffer)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}
