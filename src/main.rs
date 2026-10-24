mod game;
mod generator;
mod level;
mod player;

use macroquad::prelude::*;

use game::{Game, VIEW_HEIGHT};
use player::WireInput;

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
    let mut accumulator = 0.0;
    // ゲームオーバーになってからの経過時間。
    let mut since_over = 0.0;
    // クリックは描画フレームで受け、次に回る固定刻みの更新で 1 回だけ使う。
    let mut wire_pressed = false;

    loop {
        let frame_time = get_frame_time().min(MAX_FRAME_TIME);
        let camera = Camera2D::from_display_rect(view_rect(game.camera_x));
        let aim = camera.screen_to_world(mouse_position().into());
        let held = is_mouse_button_down(MouseButton::Left);
        wire_pressed |= is_mouse_button_pressed(MouseButton::Left);
        if game.over {
            since_over += frame_time;
            if since_over >= RETRY_DELAY && is_mouse_button_pressed(MouseButton::Left) {
                game = Game::new();
                accumulator = 0.0;
                since_over = 0.0;
            }
            wire_pressed = false;
        } else {
            accumulator += frame_time;
            while accumulator >= FIXED_DT && !game.over {
                let input = WireInput {
                    aim,
                    pressed: wire_pressed,
                    held,
                };
                game.step(input, FIXED_DT);
                wire_pressed = false;
                accumulator -= FIXED_DT;
            }
        }

        clear_background(Color::from_rgba(16, 18, 24, 255));

        set_camera(&Camera2D::from_display_rect(view_rect(game.camera_x)));
        game.draw();

        set_default_camera();
        draw_text(
            format!("{:.0} m", game.distance()),
            16.0,
            32.0,
            28.0,
            Color::from_rgba(180, 190, 205, 255),
        );
        draw_text(
            "hold left click: wire",
            16.0,
            screen_height() - 16.0,
            22.0,
            Color::from_rgba(150, 160, 175, 200),
        );
        if game.over {
            draw_game_over(&game, since_over);
        }

        next_frame().await
    }
}

/// 画面に映るワールドの範囲。縦は固定で、横は縦横比で決める。
fn view_rect(camera_x: f32) -> Rect {
    let aspect = screen_width() / screen_height();
    Rect::new(camera_x, 0.0, VIEW_HEIGHT * aspect, VIEW_HEIGHT)
}

fn draw_game_over(game: &Game, since_over: f32) {
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
        &format!("{:.0} m", game.distance()),
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
