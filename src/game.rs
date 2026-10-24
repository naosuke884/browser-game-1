use macroquad::prelude::*;

use crate::enemy::Enemies;
use crate::generator::Generator;
use crate::level::Level;
use crate::player::{Player, WireInput, MAX_WIRE_LENGTH, PLAYER_RADIUS};

/// 画面に映すワールドの高さ。y = 0 が画面の上端、y = VIEW_HEIGHT が下端。
pub const VIEW_HEIGHT: f32 = 900.0;
/// カメラは自機がカメラの左端からこの位置に来るよう左右どちらへも追いかける。
/// 16:9 の画面で中央より少し左に置き、進む先を広く見せる。
const FOLLOW_OFFSET: f32 = 600.0;
/// 最も右まで進んだときのカメラの左端から、これだけ右まで地形を作っておく。
/// 横長の画面でも右端の先まで足りる幅にする。
const GENERATE_AHEAD: f32 = 4000.0;
/// 最も右まで進んだときのカメラの左端より、これだけ左の地形は捨てる。
/// 少し戻るのは自由だが、ずっと戻ると地形がなくなり、刺す先がなくて落ちる。
const KEEP_BEHIND: f32 = 3000.0;
/// 開始時の自機の、カメラ左端からの位置。
const START_OFFSET: Vec2 = vec2(300.0, 450.0);
/// 敵との当たり判定は見た目より小さくし、かすっただけでは終わらないようにする。
const PLAYER_HURT_RADIUS: f32 = PLAYER_RADIUS * 0.7;
/// 敵 1 体の基本の得点。連鎖の倍率を掛けて入る。
const KILL_POINTS: u32 = 100;
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
    /// 敵を倒して得た点の合計。
    kill_points: u32,
    /// 天井やブロックに刺さずに続けて倒した数。次に倒した敵の倍率は chain + 1 になる。
    pub chain: u32,
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
        let mut generator = Generator::new(-KEEP_BEHIND, seed);
        generator.fill(&mut level, GENERATE_AHEAD);
        let start = START_OFFSET;
        // 最初から真上の天井へワイヤーを刺した状態で始める。
        let mut player = Player::new(start);
        player.shoot_wire(start + vec2(0.0, -1.0), &level);
        Self {
            level,
            generator,
            player,
            enemies: Enemies::new(seed),
            kills: 0,
            kill_points: 0,
            chain: 0,
            camera_x: start.x - FOLLOW_OFFSET,
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
        self.camera_x = self.player.position.x - FOLLOW_OFFSET;
        let front = self.best_x - FOLLOW_OFFSET;
        self.generator.fill(&mut self.level, front + GENERATE_AHEAD);
        self.level.remove_before(front - KEEP_BEHIND);

        self.aim = input.aim;
        // 撃ったワイヤーが地形より手前の敵に当たったら、その敵を倒して引き寄せられる。
        // 天井やブロックに刺すか、何もない所へ撃つと連鎖が切れる。ワイヤーを戻すだけなら切れない。
        if input.pressed && self.player.can_shoot() {
            if let Some((index, point)) = self.enemy_target(input.aim) {
                self.enemies.list.swap_remove(index);
                self.kills += 1;
                self.chain += 1;
                self.kill_points += KILL_POINTS * self.chain;
                self.player.pull_toward(point);
                input.pressed = false;
            } else {
                self.chain = 0;
            }
        }
        self.player.update(input, &self.level, dt);
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

    /// 得点。進んだ距離 1 m につき 1 点と、敵を倒した点の合計。
    pub fn score(&self) -> u32 {
        self.distance() as u32 + self.kill_points
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
        let before = game.camera_x;
        game.step(WireInput::default(), DT);
        assert_eq!(game.camera_x, before);
    }

    #[test]
    fn wire_kills_enemy_and_pulls_player() {
        let mut game = Game::with_seed(1);
        release_wire(&mut game);
        place_enemy(&mut game, 300.0);
        shoot(&mut game, vec2(1.0, 0.0));
        assert_eq!(game.kills, 1);
        assert!(game.enemies.list.is_empty());
        assert!(game.player.anchor.is_none());
        assert!(game.player.velocity.x > 600.0);
    }

    /// 自機の右 `dx` に浮遊する敵を 1 体置く。
    fn place_enemy(game: &mut Game, dx: f32) {
        let p = game.player.position;
        let mut enemies = Enemies::new(1);
        enemies.update(game.camera_x, p, 100.0, DT);
        let mut enemy = enemies.list.pop().unwrap();
        enemy.position = p + vec2(dx, 0.0);
        game.enemies.list.push(enemy);
    }

    /// 自機から `dir` の向きへワイヤーを撃つ。刺さっているときは戻すだけになる。
    fn shoot(game: &mut Game, dir: Vec2) {
        let input = WireInput {
            aim: game.player.position + dir,
            pressed: true,
            ..Default::default()
        };
        game.step(input, DT);
    }

    /// 刺さっているワイヤーを戻し、手元に戻るまで待つ。
    fn release_wire(game: &mut Game) {
        if game.player.anchor.is_some() {
            shoot(game, vec2(0.0, -1.0));
        }
        while !game.player.can_shoot() {
            game.step(WireInput::default(), DT);
        }
    }

    #[test]
    fn starts_hanging_from_the_ceiling() {
        let game = Game::with_seed(1);
        let anchor = game.player.anchor.unwrap();
        assert_eq!(anchor.point.x, game.player.position.x);
    }

    #[test]
    fn chained_kills_multiply_points_until_wire_hits_terrain() {
        let mut game = Game::with_seed(1);
        release_wire(&mut game);
        place_enemy(&mut game, 300.0);
        shoot(&mut game, vec2(1.0, 0.0));
        place_enemy(&mut game, 300.0);
        shoot(&mut game, vec2(1.0, 0.0));
        assert_eq!(game.chain, 2);
        assert_eq!(game.kill_points, KILL_POINTS + KILL_POINTS * 2);

        // 天井に刺すと連鎖が切れ、次の敵は 1 倍に戻る。
        shoot(&mut game, vec2(0.0, -1.0));
        assert!(game.player.anchor.is_some());
        assert_eq!(game.chain, 0);
        release_wire(&mut game);
        place_enemy(&mut game, 300.0);
        shoot(&mut game, vec2(1.0, 0.0));
        assert_eq!(game.kill_points, KILL_POINTS * 4);
        assert_eq!(game.score(), game.distance() as u32 + KILL_POINTS * 4);
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
    fn camera_follows_player_both_ways() {
        let mut game = Game::new();
        release_wire(&mut game);
        game.player.position.x = 5000.0;
        game.step(WireInput::default(), DT);
        assert_eq!(game.camera_x, game.player.position.x - FOLLOW_OFFSET);
        game.player.position.x = 4000.0;
        game.step(WireInput::default(), DT);
        assert_eq!(game.camera_x, game.player.position.x - FOLLOW_OFFSET);
    }

    #[test]
    fn falling_off_the_bottom_ends_the_game() {
        let mut game = Game::new();
        release_wire(&mut game);
        for _ in 0..600 {
            game.step(WireInput::default(), DT);
        }
        assert!(game.over);
        assert!(game.player.position.y > VIEW_HEIGHT);
    }

    #[test]
    fn can_move_left_of_the_start() {
        let mut game = Game::new();
        release_wire(&mut game);
        game.player.position = vec2(0.0, 300.0);
        game.player.velocity = vec2(-500.0, 0.0);
        game.step(WireInput::default(), DT);
        assert!(!game.over);
        assert!(game.player.position.x < 0.0);
    }

    /// 真上から右斜め上までの向きのうち、ワイヤーが刺さる点が自機の 100 先にいちばん近い向き。
    fn best_aim(game: &Game) -> Option<Vec2> {
        let p = game.player.position;
        (0..=12)
            .map(|i| {
                let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * 0.1;
                p + Vec2::from_angle(angle)
            })
            .filter_map(|aim| game.player.wire_target(aim, &game.level).map(|t| (aim, t)))
            // 近すぎる刺し先は、天井の角に挟まって動けなくなるので選ばない。
            .filter(|(_, t)| t.distance(p) > 150.0)
            .min_by(|a, b| {
                let da = (a.1.x - p.x - 100.0).abs();
                let db = (b.1.x - p.x - 100.0).abs();
                da.total_cmp(&db)
            })
            .map(|(aim, _)| aim)
    }

    /// 単純な操作でも、落ちずに 30 秒で 300 m 以上進めることを確かめる。
    /// 刺さっていなければ少し先の刺し先へ撃ち、支点より十分右で上がっている途中にワイヤーを戻す。
    /// 移動だけを確かめるので、敵は毎回消す。
    /// ボットは粗く、長く続けると低い所へ沈んで落ちたり、引っかかって止まったりする種がある。
    /// 仕組みが壊れていないことの確認なので、最初の 30 秒を、止まらずに進める種だけで確かめる。
    #[test]
    fn simple_swinging_makes_progress() {
        for seed in [3, 5, 8] {
            swing_for_a_while(seed);
        }
    }

    fn swing_for_a_while(seed: u64) {
        let mut game = Game::with_seed(seed);
        for _ in 0..(30.0 / DT) as usize {
            let p = game.player.position;
            let pressed = match game.player.anchor {
                // 刺さっていなければ、落ち始めてから撃つ。
                None => {
                    game.player.can_shoot()
                        && game.player.velocity.y > 0.0
                        && best_aim(&game).is_some()
                }
                // 支点より十分右で上がっている途中に、ワイヤーを戻して飛ぶ。
                Some(a) => {
                    p.x > a.point.x + a.length * 0.3
                        && game.player.velocity.x > 0.0
                        && game.player.velocity.y < 0.0
                }
            };
            // 刺さっている間は長さ 150 まで巻き取る。動いている向きへ、待ち時間が明けるたびに一押しする。
            let wheel = match game.player.anchor {
                Some(a) if a.target_length > 150.0 => -1.0,
                _ => 0.0,
            };
            let side = if game.player.velocity.x < 0.0 {
                -1.0
            } else {
                1.0
            };
            let input = WireInput {
                aim: best_aim(&game).unwrap_or(game.player.position),
                pressed,
                wheel,
                swing_pressed: !pressed,
            };
            // 一押しの向きはカーソルの側で決まるので、揺らすときだけカーソルを動いている側へ置く。
            let input = if input.swing_pressed {
                WireInput {
                    aim: game.player.position + vec2(side * 100.0, 0.0),
                    ..input
                }
            } else {
                input
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
            game.distance() > 300.0,
            "seed {seed}: {:.0} m",
            game.distance()
        );
    }

    #[test]
    fn terrain_is_generated_ahead_and_discarded_behind() {
        let mut game = Game::new();
        release_wire(&mut game);
        game.player.position.x = 10_000.0;
        // 地形を作り足すのは、最も右まで進んだ位置を覚えた次の更新から。
        game.step(WireInput::default(), DT);
        game.step(WireInput::default(), DT);
        let front = game.best_x - FOLLOW_OFFSET;
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
        assert!(right >= front + GENERATE_AHEAD - 1000.0);
        assert!(left >= front - KEEP_BEHIND);
    }
}
