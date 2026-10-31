mod enemy;
mod game;
mod generator;
mod level;
mod player;
mod realtime;
mod shadow;

use macroquad::prelude::*;

use game::{Game, VIEW_HEIGHT};
use player::WireInput;
use shadow::{Shadows, State};

/// 更新は固定の刻みで回し、画面のリフレッシュレートで手触りが変わらないようにする。
const FIXED_DT: f32 = 1.0 / 120.0;
/// 処理落ちしたときに 1 フレームで追いつく上限。これを超えた分は捨ててゲームを遅らせる。
const MAX_FRAME_TIME: f32 = 0.1;
/// ゲームオーバー直後のクリックでうっかりやり直さないよう、受け付けない時間。
const RETRY_DELAY: f32 = 0.6;

fn window_conf() -> Conf {
    Conf {
        window_title: "browser-game-1".to_owned(),
        window_width: 1280,
        window_height: 720,
        // ネイティブウィンドウのドラッグリサイズを受け付ける。
        // 毎フレーム screen_width / screen_height でカメラを作り直すので、WASM 側は
        // キャンバスの CSS サイズ（ブラウザのウィンドウ）にも同じ描画で追従する。
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new();
    realtime::join(&game::room_key(game.seed()));
    let mut shadows = Shadows::default();
    // 次に自機の状態を影の部屋へ送るまでの時間。
    let mut until_send = 0.0;
    // 自己ベストは roommate-sdk がサーバーと端末の大きいほうを返す。サーバーの分は後から届くので、毎フレーム読む。
    let mut high_score = 0;
    let mut accumulator = 0.0;
    // ゲームオーバーになってからの経過時間。
    let mut since_over = 0.0;
    // クリックは描画フレームで受け、次に回る固定刻みの更新で 1 回だけ使う。
    let mut wire_pressed = false;
    let mut boost_pressed = false;

    loop {
        let frame_time = get_frame_time().min(MAX_FRAME_TIME);
        let camera = view_camera(game.camera_x);
        let aim = camera.screen_to_world(mouse_position().into());
        wire_pressed |= is_mouse_button_pressed(MouseButton::Left);
        boost_pressed |= is_mouse_button_pressed(MouseButton::Right);
        if game.over {
            since_over += frame_time;
            if since_over >= RETRY_DELAY && is_mouse_button_pressed(MouseButton::Left) {
                game = Game::new();
                // 日付が変わっていれば、新しいコースの部屋へ移る。
                realtime::join(&game::room_key(game.seed()));
                accumulator = 0.0;
                since_over = 0.0;
            }
            wire_pressed = false;
            boost_pressed = false;
        } else {
            accumulator += frame_time;
            while accumulator >= FIXED_DT && !game.over {
                let input = WireInput {
                    aim,
                    pressed: wire_pressed,
                    boost: boost_pressed,
                };
                game.step(input, FIXED_DT);
                wire_pressed = false;
                boost_pressed = false;
                accumulator -= FIXED_DT;
            }
            if game.over {
                roommate_sdk::submit_score(game.score(), (game.elapsed * 1000.0) as u32);
            }
        }

        high_score = high_score.max(roommate_sdk::best_score().unwrap_or(0));
        let now = get_time();
        while let Some(message) = realtime::receive() {
            if let Some(incoming) = shadow::read_incoming(&message) {
                shadows.receive(incoming, now);
            }
        }
        shadows.forget_stale(now);
        // 終わった後は送らない。届かなくなった影は、他の人の画面からしばらくして消える。
        if !game.over {
            until_send -= frame_time;
            if until_send <= 0.0 {
                until_send = shadow::SEND_INTERVAL;
                let state = State::of(&game.player, (now * 1000.0) as u32);
                realtime::send(&state.encode());
            }
        }

        clear_background(Color::from_rgba(16, 18, 24, 255));

        set_camera(&view_camera(game.camera_x));
        // 影は判定に関わらないので、地形や自機より奥に描く。
        shadows.draw(now);
        game.draw();

        set_default_camera();
        draw_text(
            format!(
                "score {}   {:.0} m   kills {}",
                game.score(),
                game.distance(),
                game.kills
            ),
            16.0,
            32.0,
            28.0,
            Color::from_rgba(180, 190, 205, 255),
        );
        draw_text(
            "left click: shoot / pull back wire   right click: boost",
            16.0,
            screen_height() - 16.0,
            22.0,
            Color::from_rgba(150, 160, 175, 200),
        );
        draw_text(
            format!("best {high_score}"),
            16.0,
            60.0,
            22.0,
            Color::from_rgba(150, 160, 175, 255),
        );
        // ブーストの待ち時間。使えるときは明るく、待っている間は残り時間を出す。
        let cooldown = game.player.boost_cooldown;
        let (label, color) = if cooldown > 0.0 {
            (
                format!("boost {cooldown:.1}"),
                Color::from_rgba(140, 150, 165, 255),
            )
        } else {
            (
                "boost ready".to_owned(),
                Color::from_rgba(140, 220, 160, 255),
            )
        };
        draw_text(label, 16.0, 116.0, 22.0, color);
        if game.chain > 0 {
            let multiplier = game.next_multiplier();
            let max = if multiplier == game::MAX_MULTIPLIER {
                " max"
            } else {
                ""
            };
            draw_text(
                format!("chain x{multiplier}{max}"),
                16.0,
                88.0,
                28.0,
                Color::from_rgba(255, 200, 80, 255),
            );
        }
        if game.over {
            draw_game_over(&game, since_over, high_score);
        }

        next_frame().await
    }
}

/// 画面に映るワールドの範囲を映すカメラ。縦は固定で、横は縦横比で決める。
/// ワールドは y 下向きで、y = 0 を画面の上端に映す。
/// `Camera2D::from_display_rect` は画面に描くと上下が逆になるので使わない。
fn view_camera(camera_x: f32) -> Camera2D {
    let aspect = screen_width() / screen_height();
    let size = vec2(VIEW_HEIGHT * aspect, VIEW_HEIGHT);
    Camera2D {
        target: vec2(camera_x, 0.0) + size / 2.0,
        zoom: vec2(2.0 / size.x, 2.0 / size.y),
        ..Default::default()
    }
}

fn draw_game_over(game: &Game, since_over: f32, high_score: u32) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(0, 0, 0, 140),
    );
    let center = vec2(screen_width(), screen_height()) / 2.0;
    draw_centered_text("GAME OVER", center.x, center.y - 20.0, 56.0, WHITE);
    draw_centered_text(
        &format!(
            "score {}   ({:.0} m, kills {})   best {high_score}",
            game.score(),
            game.distance(),
            game.kills
        ),
        center.x,
        center.y + 24.0,
        28.0,
        WHITE,
    );
    if since_over >= RETRY_DELAY {
        draw_centered_text(
            "click to retry",
            center.x,
            center.y + 64.0,
            24.0,
            Color::from_rgba(180, 190, 205, 255),
        );
    }
}

fn draw_centered_text(text: &str, center_x: f32, baseline_y: f32, font_size: f32, color: Color) {
    let dimensions = measure_text(text, None, font_size as u16, 1.0);
    draw_text(
        text,
        center_x - dimensions.width / 2.0,
        baseline_y,
        font_size,
        color,
    );
}
