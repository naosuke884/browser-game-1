// 影の部屋との通信。WebSocket で部屋に繋ぎ、wasm とバイト列をやり取りする関数を渡す。
// mq_js_bundle.js の後、load() の前に読み込む。
// 繋がらないときは何もしない。ゲームは影なしで続く。
(function () {
    var API_ORIGIN = "https://api.roommate.hayashi-naoki.com";
    // API が受け付けるのは配信用ドメインからだけ。ほかで開いたとき（手元の確認など）は繋がない。
    var PLAY_ORIGIN = "https://play.roommate.hayashi-naoki.com";
    // 届いたメッセージを wasm が取り出すまで溜める。タブが裏にある間に溜まりすぎないよう上限を置く。
    var MAX_QUEUE = 64;
    // 繋がらないときは待ち時間を倍にしていく。繋がったら最初に戻す。
    var RECONNECT_MS = 5000;
    var MAX_RECONNECT_MS = 60000;
    var reconnectDelay = RECONNECT_MS;

    var socket = null;
    var roomKey = null;
    var queue = [];
    var reconnectTimer = null;

    // 匿名 ID のトークンは brand-sdk が受け持つ。brand-sdk.js をこれより前に読み込む。
    function getToken() {
        var client = window.roommate && window.roommate.client;
        if (!client) return Promise.reject(new Error("brand-sdk が無い"));
        return client.token().then(function (token) {
            if (!token) throw new Error("トークンが取れない");
            return token;
        });
    }

    function push(message) {
        queue.push(message);
        if (queue.length > MAX_QUEUE) queue.shift();
    }

    function scheduleReconnect(key) {
        if (reconnectTimer !== null) return;
        reconnectTimer = setTimeout(function () {
            reconnectTimer = null;
            if (roomKey === key) connect(key);
        }, reconnectDelay);
        reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_MS);
    }

    function connect(key) {
        getToken()
            .then(function (token) {
                if (roomKey !== key) return;
                var url =
                    API_ORIGIN.replace(/^http/, "ws") +
                    "/rooms/" +
                    encodeURIComponent(key) +
                    "?token=" +
                    encodeURIComponent(token);
                var ws = new WebSocket(url);
                ws.binaryType = "arraybuffer";
                ws.onopen = function () {
                    reconnectDelay = RECONNECT_MS;
                };
                ws.onmessage = function (event) {
                    if (socket !== ws) return;
                    if (typeof event.data === "string") {
                        // 部屋を出た人だけ伝える。来た人は状態が届いたときに分かる。
                        var notice;
                        try {
                            notice = JSON.parse(event.data);
                        } catch (e) {
                            return;
                        }
                        if (notice && notice.type === "leave" && Number.isInteger(notice.id)) {
                            push(new Uint8Array([1, (notice.id >> 8) & 255, notice.id & 255]));
                        }
                        return;
                    }
                    var data = new Uint8Array(event.data);
                    var message = new Uint8Array(data.length + 1);
                    message[0] = 0;
                    message.set(data, 1);
                    push(message);
                };
                ws.onclose = function () {
                    if (socket !== ws) return;
                    socket = null;
                    scheduleReconnect(key);
                };
                socket = ws;
            })
            .catch(function () {
                scheduleReconnect(key);
            });
    }

    function join(key) {
        if (window.location.origin !== PLAY_ORIGIN) return;
        if (key === roomKey) return;
        roomKey = key;
        queue = [];
        if (socket) {
            var old = socket;
            socket = null;
            old.close();
        }
        connect(key);
    }

    function register_plugin(importObject) {
        importObject.env.realtime_join = function (ptr, len) {
            join(UTF8ToString(ptr, len));
        };
        importObject.env.realtime_send = function (ptr, len) {
            if (!socket || socket.readyState !== WebSocket.OPEN) return;
            socket.send(new Uint8Array(wasm_memory.buffer, ptr, len).slice());
        };
        importObject.env.realtime_next_len = function () {
            return queue.length > 0 ? queue[0].length : -1;
        };
        importObject.env.realtime_take = function (ptr, len) {
            var message = queue.shift();
            if (!message) return;
            new Uint8Array(wasm_memory.buffer, ptr, len).set(message.subarray(0, len));
        };
    }

    miniquad_add_plugin({ register_plugin: register_plugin, name: "browser_game_realtime", version: 1 });
})();
