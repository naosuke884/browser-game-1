// ハイスコアを localStorage に読み書きする関数を、wasm から呼べるように渡す。
// mq_js_bundle.js の後、load() の前に読み込む。
(function () {
    var KEY = "browser-game-1.high-score";

    function register_plugin(importObject) {
        importObject.env.high_score_load = function () {
            try {
                var value = Number(window.localStorage.getItem(KEY));
                return Number.isFinite(value) ? value : 0;
            } catch (e) {
                // プライベートブラウズなどで localStorage が使えないときは 0 として扱う。
                return 0;
            }
        };
        importObject.env.high_score_save = function (score) {
            try {
                window.localStorage.setItem(KEY, String(score));
            } catch (e) {
                // 保存できなくてもゲームは続ける。
            }
        };
    }

    miniquad_add_plugin({ register_plugin: register_plugin, name: "browser_game_storage", version: 1 });
})();
