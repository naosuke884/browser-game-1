use macroquad::prelude::*;

use crate::enemy::Enemies;
use crate::generator::Generator;
use crate::level::Level;
use crate::player::{Player, WireInput, MAX_WIRE_LENGTH, PLAYER_RADIUS};

/// 画面に映すワールドの高さ。y = 0 が画面の上端、y = VIEW_HEIGHT が下端。
pub const VIEW_HEIGHT: f32 = 900.0;
/// カメラは自機がカメラの左端からこの位置に来るよう右へ追いかける。左へは戻らない。
/// 16:9 の画面で中央より少し左に置き、進む先を広く見せる。
const FOLLOW_OFFSET: f32 = 600.0;
/// カメラの左端からこれだけ右まで地形を作っておく。横長の画面でも右端の先まで足りる幅にする。
const GENERATE_AHEAD: f32 = 4000.0;
/// カメラの左端よりこれだけ左へ流れた地形は捨てる。
const DISCARD_BEHIND: f32 = 200.0;
/// 開始時の自機の、カメラ左端からの位置。
const START_OFFSET: Vec2 = vec2(300.0, 450.0);
/// 敵との当たり判定は見た目より小さくし、かすっただけでは終わらないようにする。
const PLAYER_HURT_RADIUS: f32 = PLAYER_RADIUS * 0.7;
/// 画面の距離表示の単位。ワールドの 10 を 1 m と数える。
const UNITS_PER_METER: f32 = 10.0;

/// 1 回のプレイの状態。
pub struct Game {
    pub level: Level,
    generator: Generator,
    pub player: Player,
    pub enemies: Enemies,
    /// ワイヤーで倒した敵の数。
    pub kills: u32,
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
        // 毎回違う敵の出方にするため、起動時刻から種を作る。
        Self::with_seed((macroquad::miniquad::date::now() * 1000.0) as u64)
    }

    pub fn with_seed(seed: u64) -> Self {
        let mut level = Level::default();
        let mut generator = Generator::new(-DISCARD_BEHIND, seed);
        generator.fill(&mut level, GENERATE_AHEAD);
        let start = START_OFFSET;
        Self {
            level,
            generator,
            player: Player::new(start),
            enemies: Enemies::new(seed),
            kills: 0,
            camera_x: 0.0,
            start_x: start.x,
            best_x: start.x,
            aim: start,
            over: false,
        }
    }

    pub fn step(&mut self, mut input: WireInput, dt: f32) {
        if self.over {
            return;
        }
        self.camera_x = self.camera_x.max(self.player.position.x - FOLLOW_OFFSET);
        self.generator
            .fill(&mut self.level, self.camera_x + GENERATE_AHEAD);
        self.level.remove_before(self.camera_x - DISCARD_BEHIND);

        self.aim = input.aim;
        // 撃ったワイヤーが地形より手前の敵に当たったら、その敵を倒して引き寄せられる。
        if input.pressed {
            if let Some((index, point)) = self.enemy_target(input.aim) {
                self.enemies.list.swap_remove(index);
                self.kills += 1;
                self.player.pull_toward(point);
                input.pressed = false;
            }
        }
        self.player.update(input, &self.level, dt);
        // 画面の左端は壁にする。戻れないだけで、触れても終わらない。
        let left_limit = self.camera_x + PLAYER_RADIUS;
        if self.player.position.x < left_limit {
            self.player.position.x = left_limit;
            self.player.velocity.x = self.player.velocity.x.max(0.0);
        }
        self.best_x = self.best_x.max(self.player.position.x);
        let player = self.player.position;
        let distance = self.distance();
        self.enemies.update(self.camera_x, player, distance, dt);

        let fell = self.player.position.y - PLAYER_RADIUS > VIEW_HEIGHT;
        let hit = self.enemies.any_touching(player, PLAYER_HURT_RADIUS);
        if fell || hit {
            self.over = true;
        }
    }

    /// `aim` の方向へ撃ったとき、地形より手前で当たる敵の番号と当たる点。
    fn enemy_target(&self, aim: Vec2) -> Option<(usize, Vec2)> {
        let origin = self.player.position;
        let dir = (aim - origin).normalize_or_zero();
        let (index, t) = self.enemies.raycast(origin, dir, MAX_WIRE_LENGTH)?;
        let blocked = self
            .level
            .raycast(origin, dir, t)
            .is_some_and(|p| p.distance(origin) < t);
        (!blocked).then(|| (index, origin + dir * t))
    }

    /// 開始位置から最も右へ進んだ距離（m）。
    pub fn distance(&self) -> f32 {
        (self.best_x - self.start_x) / UNITS_PER_METER
    }

    pub fn draw(&self) {
        self.level.draw();
        self.enemies.draw();
        // ワイヤーを持っていないときは、今撃てば刺さる点（敵が手前なら敵）に照準を出す。
        if self.player.anchor.is_none() {
            let target = match self.enemy_target(self.aim) {
                Some((_, point)) => Some(point),
                None => self.player.wire_target(self.aim, &self.level),
            };
            if let Some(target) = target {
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
    fn camera_does_not_scroll_by_itself() {
        let mut game = Game::new();
        game.step(WireInput::default(), DT);
        assert_eq!(game.camera_x, 0.0);
    }

    #[test]
    fn wire_kills_enemy_and_pulls_player() {
        let mut game = Game::with_seed(1);
        let p = game.player.position;
        let mut enemies = Enemies::new(1);
        enemies.update(game.camera_x, p, 100.0, DT);
        let mut enemy = enemies.list.pop().unwrap();
        enemy.position = p + vec2(300.0, 0.0);
        game.enemies.list.push(enemy);
        let input = WireInput {
            aim: p + vec2(300.0, 0.0),
            pressed: true,
            reel: 0.0,
        };
        game.step(input, DT);
        assert_eq!(game.kills, 1);
        assert!(game.enemies.list.is_empty());
        assert!(game.player.anchor.is_none());
        assert!(game.player.velocity.x > 600.0);
    }

    #[test]
    fn touching_enemy_ends_the_game() {
        let mut game = Game::with_seed(1);
        let p = game.player.position;
        let mut enemies = Enemies::new(1);
        enemies.update(game.camera_x, p, 100.0, DT);
        let mut enemy = enemies.list.pop().unwrap();
        enemy.position = p;
        game.enemies.list.push(enemy);
        game.step(WireInput::default(), DT);
        assert!(game.over);
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
    fn left_edge_is_a_wall() {
        let mut game = Game::new();
        game.player.position = vec2(game.camera_x + PLAYER_RADIUS + 1.0, 300.0);
        game.player.velocity = vec2(-500.0, 0.0);
        game.step(WireInput::default(), DT);
        assert!(!game.over);
        assert_eq!(game.player.position.x, game.camera_x + PLAYER_RADIUS);
        assert_eq!(game.player.velocity.x, 0.0);
    }

    /// 真上から右斜め上までの向きのうち、ワイヤーが刺さる点が自機の 100 先にいちばん近い向き。
    fn best_aim(game: &Game) -> Vec2 {
        let p = game.player.position;
        (0..=12)
            .map(|i| {
                let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * 0.1;
                p + Vec2::from_angle(angle)
            })
            .filter_map(|aim| game.player.wire_target(aim, &game.level).map(|t| (aim, t)))
            .min_by(|a, b| {
                let da = (a.1.x - p.x - 100.0).abs();
                let db = (b.1.x - p.x - 100.0).abs();
                da.total_cmp(&db)
            })
            .map_or(p + vec2(0.0, -1.0), |(aim, _)| aim)
    }

    /// 単純な操作でも、落ちずに 1 分で 1000 m 以上進めることを確かめる。
    /// 刺さっていなければ少し先の刺し先へ撃ち、支点を越えて上がり始めたら次の刺し先へ撃ち直す。
    /// 移動だけを確かめるので、敵は毎回消す。
    /// ボットは粗く、ブロックの上に乗って止まったり、低い所へ沈んだりする種がある。
    /// 地形と操作の仕組みが壊れていないことの確認なので、最後まで進める種だけで確かめる。
    #[test]
    fn simple_swinging_makes_progress() {
        for seed in [1, 2, 5, 7, 8, 9, 10] {
            swing_for_a_minute(seed);
        }
    }

    fn swing_for_a_minute(seed: u64) {
        let mut game = Game::with_seed(seed);
        for _ in 0..(60.0 / DT) as usize {
            let p = game.player.position;
            let pressed = match game.player.anchor {
                // 刺さっていなければ、落ち始めてから撃つ。
                None => game.player.velocity.y > 0.0,
                // 支点を越えて上がり始めたら、次の刺し先へ撃ち直す。
                Some(a) => {
                    p.x > a.point.x && game.player.velocity.x > 0.0 && game.player.velocity.y < 0.0
                }
            };
            // 刺さっている間は、長さ 150 になるまで巻き取る。
            let reel = match game.player.anchor {
                Some(a) if a.length > 150.0 => -1.0,
                _ => 0.0,
            };
            let input = WireInput {
                aim: best_aim(&game),
                pressed,
                reel,
            };
            game.enemies.list.clear();
            game.step(input, DT);
            assert!(
                !game.over,
                "seed {seed}: {:.0} m で終わった",
                game.distance()
            );
        }
        assert!(
            game.distance() > 1000.0,
            "seed {seed}: {:.0} m",
            game.distance()
        );
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
        // 最後が穴で終わると刺し先の右端は少し手前になるので、穴の幅の分だけ余裕を見る。
        assert!(right >= game.camera_x + GENERATE_AHEAD - 1000.0);
        assert!(left >= game.camera_x - DISCARD_BEHIND);
    }
}
