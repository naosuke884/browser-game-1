use macroquad::prelude::*;

use crate::generator::Generator;
use crate::level::Level;
use crate::player::{Player, WireInput, PLAYER_RADIUS};

/// 画面に映すワールドの高さ。y = 0 が画面の上端、y = VIEW_HEIGHT が下端。
pub const VIEW_HEIGHT: f32 = 900.0;
/// カメラが右へ進む速さ。
const SCROLL_SPEED: f32 = 200.0;
/// 自機がカメラの左端からこれより右へ出たら、カメラが追いかけて先へ進む。
/// 画面の右端の外へ飛び出して見えなくならないようにするため。16:9 の画面で中央より少し右。
const FOLLOW_OFFSET: f32 = 900.0;
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
    /// 直前の更新で受けたカーソルの位置。照準の表示に使う。
    aim: Vec2,
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
            aim: start,
            over: false,
        }
    }

    pub fn step(&mut self, input: WireInput, dt: f32) {
        if self.over {
            return;
        }
        self.camera_x += SCROLL_SPEED * dt;
        self.camera_x = self.camera_x.max(self.player.position.x - FOLLOW_OFFSET);
        self.generator
            .fill(&mut self.level, self.camera_x + GENERATE_AHEAD);
        self.level.remove_before(self.camera_x - DISCARD_BEHIND);

        self.aim = input.aim;
        self.player.update(input, &self.level, dt);
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
        // ワイヤーを持っていないときは、今撃てば刺さる点に照準を出す。
        if self.player.anchor.is_none() {
            if let Some(target) = self.player.wire_target(self.aim, &self.level) {
                draw_circle_lines(
                    target.x,
                    target.y,
                    8.0,
                    2.0,
                    Color::from_rgba(255, 200, 80, 180),
                );
            }
        }
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
        game.step(WireInput::default(), 1.0);
        assert_eq!(game.camera_x, SCROLL_SPEED);
    }

    #[test]
    fn camera_follows_player_running_ahead() {
        let mut game = Game::new();
        game.player.position.x = 5000.0;
        game.step(WireInput::default(), DT);
        assert_eq!(game.camera_x, 5000.0 - FOLLOW_OFFSET);
    }

    #[test]
    fn falling_off_the_bottom_ends_the_game() {
        let mut game = Game::new();
        for _ in 0..600 {
            game.step(WireInput::default(), DT);
        }
        assert!(game.over);
        assert!(game.player.position.y > VIEW_HEIGHT);
    }

    #[test]
    fn being_left_behind_ends_the_game() {
        let mut game = Game::new();
        // 自機をカメラの左端すぐ右に置き、次の更新でカメラに追い越させる。
        game.player.position = vec2(game.camera_x + 0.5, 300.0);
        game.step(WireInput::default(), DT);
        assert!(game.over);
    }

    /// 単純な操作でもスクロールに置いていかれずに進み続けられることを確かめる。
    /// 刺さっていなければ少し先の天井へ撃ち、支点を越えて上がり始めたら離す、を繰り返す。
    #[test]
    fn simple_swinging_keeps_up_with_scroll() {
        let mut game = Game::new();
        let mut held = false;
        for _ in 0..(60.0 / DT) as usize {
            let p = game.player.position;
            let mut input = WireInput {
                aim: vec2(p.x + 150.0, 0.0),
                pressed: false,
                held,
            };
            match game.player.anchor {
                // 外れたら一度離し、落ち始めてから撃ち直す。
                None if held => input.held = false,
                None if game.player.velocity.y > 0.0 => {
                    input.pressed = true;
                    input.held = true;
                }
                Some(a)
                    if p.x > a.point.x
                        && game.player.velocity.x > 0.0
                        && game.player.velocity.y < 0.0 =>
                {
                    input.held = false;
                }
                _ => {}
            }
            held = input.held;
            game.step(input, DT);
            assert!(!game.over, "{:.0} m で終わった", game.distance());
        }
        assert!(game.distance() > 60.0 * SCROLL_SPEED / UNITS_PER_METER * 0.9);
    }

    #[test]
    fn terrain_is_generated_ahead_and_discarded_behind() {
        let mut game = Game::new();
        game.camera_x = 10_000.0;
        game.step(WireInput::default(), DT);
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
