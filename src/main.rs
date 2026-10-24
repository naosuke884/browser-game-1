use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "browser-game-1".to_owned(),
        window_width: 1280,
        window_height: 720,
        // ネイティブウィンドウのドラッグリサイズを受け付ける。
        // 毎フレーム screen_width / screen_height で配置するので、WASM 側は
        // キャンバスの CSS サイズ（ブラウザのウィンドウ）にも同じ描画で追従する。
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        let width = screen_width();
        let height = screen_height();

        clear_background(Color::from_rgba(16, 18, 24, 255));

        let margin = 24.0;
        if width > margin * 2.0 && height > margin * 2.0 {
            draw_rectangle_lines(
                margin,
                margin,
                width - margin * 2.0,
                height - margin * 2.0,
                2.0,
                Color::from_rgba(120, 180, 255, 255),
            );
        }

        let size_label = format!("{width:.0} x {height:.0}");
        draw_centered_text(&size_label, width / 2.0, height / 2.0, 36.0, WHITE);
        draw_centered_text(
            "browser-game-1",
            width / 2.0,
            height / 2.0 - 48.0,
            28.0,
            Color::from_rgba(180, 190, 205, 255),
        );

        next_frame().await
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
