use macroquad::prelude::*;

use crate::generator::Generator;
use crate::level::Level;
use crate::player::{Player, PLAYER_RADIUS};

/// 画面に映すワールドの高さ。y = 0 が画面の上端、y = VIEW_HEIGHT が下端。
pub const VIEW_HEIGHT: f32 = 900.0;
/// カメラが右へ進む速さ。
const SCROLL_SPEED: f32 = 200.0;
/// カメラの左端からこれだけ右まで地形を作っておく。横長の画面でも右端の先まで足りる幅にする。
const GENERATE_AHEAD: f32 = 4000.0;
/// カメラの左端よりこれだけ左へ流れた地形は捨てる。
const DISCARD_BEHIND: f32 = 200.0;
/// 開始時の自機の、カメラ左端からの位置。
const START_OFFSET: Vec2 = vec2(300.0, 450.0);
/// 画面の距離表示の単位。ワールドの 10 を 1 m と数える。
const UNITS_PER_METER: f32 = 10.0;

/// 1 回のプレイの状態。
pub struct Game {
    pub level: Level,
    generator: Generator,
    pub player: Player,
    /// カメラの左端のワールド x 座標。
    pub camera_x: f32,
    start_x: f32,
    best_x: f32,
    pub over: bool,
}

impl Game {
    pub fn new() -> Self {
        let mut level = Level::default();
        let mut generator = Generator::new(-DISCARD_BEHIND);
        generator.fill(&mut level, GENERATE_AHEAD);
        let start = START_OFFSET;
        Self {
            level,
            generator,
            player: Player::new(start),
            camera_x: 0.0,
            start_x: start.x,
            best_x: start.x,
            over: false,
        }
    }

    pub fn step(&mut self, dt: f32) {
        if self.over {
            return;
        }
        self.camera_x += SCROLL_SPEED * dt;
        self.generator
            .fill(&mut self.level, self.camera_x + GENERATE_AHEAD);
        self.level.remove_before(self.camera_x - DISCARD_BEHIND);

        self.player.update(&self.level, dt);
        self.best_x = self.best_x.max(self.player.position.x);

        let pushed_off = self.player.position.x < self.camera_x;
        let fell = self.player.position.y - PLAYER_RADIUS > VIEW_HEIGHT;
        if pushed_off || fell {
            self.over = true;
        }
    }

    /// 開始位置から最も右へ進んだ距離（m）。
    pub fn distance(&self) -> f32 {
        (self.best_x - self.start_x) / UNITS_PER_METER
    }

    pub fn draw(&self) {
        self.level.draw();
        self.player.draw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    #[test]
    fn camera_scrolls_right() {
        let mut game = Game::new();
        game.step(1.0);
        assert_eq!(game.camera_x, SCROLL_SPEED);
    }

    #[test]
    fn falling_off_the_bottom_ends_the_game() {
        let mut game = Game::new();
        for _ in 0..600 {
            game.step(DT);
        }
        assert!(game.over);
        assert!(game.player.position.y > VIEW_HEIGHT);
    }

    #[test]
    fn being_left_behind_ends_the_game() {
        let mut game = Game::new();
        // 自機をカメラの左端すぐ右に置き、次の更新でカメラに追い越させる。
        game.player.position = vec2(game.camera_x + 0.5, 300.0);
        game.step(DT);
        assert!(game.over);
    }

    #[test]
    fn terrain_is_generated_ahead_and_discarded_behind() {
        let mut game = Game::new();
        game.camera_x = 10_000.0;
        game.step(DT);
        let right = game
            .level
            .blocks
            .iter()
            .map(|b| b.right())
            .fold(f32::MIN, f32::max);
        let left = game
            .level
            .blocks
            .iter()
            .map(|b| b.right())
            .fold(f32::MAX, f32::min);
        assert!(right >= game.camera_x + GENERATE_AHEAD);
        assert!(left >= game.camera_x - DISCARD_BEHIND);
    }
}
