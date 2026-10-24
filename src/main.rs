mod arena;
mod bullet;
mod ship;

use macroquad::prelude::*;

use arena::Arena;
use bullet::Bullet;
use ship::Ship;

/// 更新は固定の刻みで回し、画面のリフレッシュレートで手触りが変わらないようにする。
const FIXED_DT: f32 = 1.0 / 120.0;
/// 処理落ちしたときに 1 フレームで追いつく上限。これを超えた分は捨ててゲームを遅らせる。
const MAX_FRAME_TIME: f32 = 0.1;
/// 画面に映すワールドの高さ。ウィンドウの大きさで見える範囲が変わりすぎないよう固定し、幅は縦横比で決める。
const VIEW_HEIGHT: f32 = 900.0;

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
    let arena = Arena::new(2400.0, 1600.0);
    let mut ship = Ship::new(Vec2::ZERO);
    let mut bullet = Bullet::Held;
    let mut last_recovery = None;
    let mut accumulator = 0.0;
    // クリックは描画フレームで受け、次に回る固定刻みの更新で 1 回だけ使う。
    let mut fire_requested = false;

    loop {
        // カメラは機体を常に画面中央に置く。操縦の目標は前のフレームの位置で作ったカメラで変換する。
        let camera = view_camera(ship.position);
        let aim = camera.screen_to_world(mouse_position().into());
        fire_requested |= is_mouse_button_pressed(MouseButton::Left);

        accumulator += get_frame_time().min(MAX_FRAME_TIME);
        while accumulator >= FIXED_DT {
            ship.update(aim, &arena, FIXED_DT);
            if let Some(recovery) = bullet.update(fire_requested, &ship, &arena, FIXED_DT) {
                last_recovery = Some(recovery);
            }
            fire_requested = false;
            accumulator -= FIXED_DT;
        }

        clear_background(Color::from_rgba(16, 18, 24, 255));

        set_camera(&view_camera(ship.position));
        arena.draw();
        ship.draw_dead_zone();
        ship.draw();
        bullet.draw(&ship);
        bullet.draw_offscreen_marker(view_rect(ship.position));

        set_default_camera();
        draw_text(
            format!("speed {:.0}", ship.velocity.length()),
            16.0,
            32.0,
            24.0,
            Color::from_rgba(180, 190, 205, 255),
        );
        let bounces = match &bullet {
            Bullet::Flying(b) => format!("bounces {}", b.bounces),
            Bullet::Held => match &last_recovery {
                Some(r) => format!("recovered: bounces {} / speed {:.0}", r.bounces, r.ship_speed),
                None => "click to fire".to_owned(),
            },
        };
        draw_text(bounces, 16.0, 60.0, 24.0, Color::from_rgba(180, 190, 205, 255));

        next_frame().await
    }
}

fn view_camera(center: Vec2) -> Camera2D {
    Camera2D::from_display_rect(view_rect(center))
}

/// 画面に映るワールドの範囲。
fn view_rect(center: Vec2) -> Rect {
    let aspect = screen_width() / screen_height();
    let size = vec2(VIEW_HEIGHT * aspect, VIEW_HEIGHT);
    Rect::new(
        center.x - size.x / 2.0,
        center.y - size.y / 2.0,
        size.x,
        size.y,
    )
}
