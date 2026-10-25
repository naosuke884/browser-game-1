use macroquad::prelude::*;

use crate::enemy::Enemies;
use crate::generator::Generator;
use crate::level::Level;
use crate::player::{Player, WireInput, MAX_WIRE_LENGTH, PLAYER_RADIUS};

/// 自機がこの高さより下へ落ちたら終わり。天井の下 (y = 120 前後) からかなり下まで落ちられる。
const FALL_LIMIT_Y: f32 = 1800.0;
/// 画面に映すワールドの高さ。縦はカメラを動かさず、y = 0 から終わる高さの少し下までを一度に映す。
pub const VIEW_HEIGHT: f32 = FALL_LIMIT_Y + 60.0;
/// カメラは自機がカメラの左端からこの位置に来るよう左右どちらへも追いかける。
/// 16:9 の画面（横幅は約 3300）で中央より左に置き、進む先を広く見せる。
const FOLLOW_OFFSET: f32 = 1200.0;
/// 最も右まで進んだときのカメラの左端から、これだけ右まで地形を作っておく。
/// 横長の画面でも右端の先まで足りる幅にする。
const GENERATE_AHEAD: f32 = 6000.0;
/// 最も右まで進んだときのカメラの左端より、これだけ左の地形は捨てる。
/// 少し戻るのは自由だが、ずっと戻ると地形がなくなり、刺す先がなくて落ちる。
const KEEP_BEHIND: f32 = 3000.0;
/// 開始時の自機の位置。
const START_OFFSET: Vec2 = vec2(300.0, 450.0);
/// 開始位置より左と、開始位置から右へこの幅までは平らな天井にし、毎回同じ形で始まるようにする。
const START_FLAT_WIDTH: f32 = 1100.0;
/// 敵との当たり判定は見た目より小さくし、かすっただけでは終わらないようにする。
const PLAYER_HURT_RADIUS: f32 = PLAYER_RADIUS * 0.7;
/// 敵 1 体の基本の得点。連鎖の倍率を掛けて入る。
const KILL_POINTS: u32 = 100;
/// 左から迫ってくる、触れたら終わる領域。開始時は自機のこれだけ左から始まる。
const HAZARD_START_GAP: f32 = 900.0;
/// 迫ってくる速さ。最初はゆっくりで、時間とともに上限まで速くなる。
const HAZARD_START_SPEED: f32 = 120.0;
const HAZARD_ACCELERATION: f32 = 3.0;
const HAZARD_MAX_SPEED: f32 = 450.0;
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
    /// 左から迫ってくる領域の右端のワールド x 座標。これより左に入ったら終わり。
    pub hazard_x: f32,
    /// 迫ってくる領域の今の速さ。
    hazard_speed: f32,
    pub over: bool,
}

impl Game {
    pub fn new() -> Self {
        // 毎回違う敵の出方にするため、起動時刻から種を作る。
        Self::with_seed((macroquad::miniquad::date::now() * 1000.0) as u64)
    }

    pub fn with_seed(seed: u64) -> Self {
        let mut level = Level::default();
        let mut generator = Generator::new(-KEEP_BEHIND, START_OFFSET.x + START_FLAT_WIDTH, seed);
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
            hazard_x: start.x - HAZARD_START_GAP,
            hazard_speed: HAZARD_START_SPEED,
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
        self.enemies.update(self.camera_x, distance, dt);

        self.hazard_speed = (self.hazard_speed + HAZARD_ACCELERATION * dt).min(HAZARD_MAX_SPEED);
        self.hazard_x += self.hazard_speed * dt;

        let fell = self.player.position.y - PLAYER_RADIUS > FALL_LIMIT_Y;
        let caught = self.player.position.x - PLAYER_RADIUS < self.hazard_x;
        let hit = self.enemies.any_touching(player, PLAYER_HURT_RADIUS);
        if fell || hit || caught {
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
        // 左から迫ってくる領域。右端に明るい線を引いて、境目を分かりやすくする。
        let hazard_color = Color::from_rgba(120, 40, 50, 160);
        let hazard_left = self.hazard_x - 10_000.0;
        draw_rectangle(hazard_left, -1000.0, 10_000.0, 4000.0, hazard_color);
        draw_line(
            self.hazard_x,
            -1000.0,
            self.hazard_x,
            3000.0,
            4.0,
            Color::from_rgba(240, 90, 110, 255),
        );
        // 落ちたら終わる高さに、横いっぱいの帯を引いて見せる。
        draw_rectangle(
            self.camera_x - 4000.0,
            FALL_LIMIT_Y,
            12_000.0,
            200.0,
            Color::from_rgba(120, 40, 50, 160),
        );
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
    use crate::enemy::Enemy;

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
        game.enemies.list.push(Enemy::new(p + vec2(dx, 0.0), 0.0));
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
    fn starts_hanging_from_the_same_flat_ceiling_every_time() {
        for seed in 0..20 {
            let game = Game::with_seed(seed);
            let anchor = game.player.anchor.unwrap();
            assert_eq!(anchor.point, vec2(START_OFFSET.x, 120.0));
            assert_eq!(game.player.position, START_OFFSET);
        }
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
        place_enemy(&mut game, 0.0);
        game.step(WireInput::default(), DT);
        assert!(game.over);
    }

    #[test]
    fn hazard_creeps_in_from_the_left_and_speeds_up() {
        let mut game = Game::with_seed(1);
        let start = game.hazard_x;
        game.step(WireInput::default(), 1.0);
        let first = game.hazard_x - start;
        assert!(first >= HAZARD_START_SPEED);
        let before = game.hazard_x;
        game.step(WireInput::default(), 1.0);
        assert!(game.hazard_x - before > first);
    }

    #[test]
    fn touching_the_hazard_ends_the_game() {
        let mut game = Game::with_seed(1);
        game.hazard_x = game.player.position.x;
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
        assert!(game.player.position.y > FALL_LIMIT_Y);
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

    /// 単純な操作でも、落ちるまでに 300 m 以上進めることを確かめる。
    /// 刺さっていなければ少し先の刺し先へ撃ち、支点より十分右で上がっている途中にワイヤーを戻す。
    /// 右上へ上がっている途中は、待ち時間が明けるたびにブーストする。
    /// 移動だけを確かめるので、敵は毎回消す。
    /// ボットは粗く、いずれ低い所へ沈んで落ちるので、落ちずに続けることまでは求めない。
    #[test]
    fn simple_swinging_makes_progress() {
        for seed in 1..=10 {
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
                // 真下でほとんど止まっている（開始時など）ときも、一度戻して落ちながら先へ撃ち直す。
                Some(a) => {
                    let rising_ahead = p.x > a.point.x + a.length * 0.3
                        && game.player.velocity.x > 0.0
                        && game.player.velocity.y < 0.0;
                    rising_ahead || game.player.velocity.length() < 30.0
                }
            };
            let input = WireInput {
                aim: best_aim(&game).unwrap_or(game.player.position),
                pressed,
                boost: game.player.velocity.y < 0.0 && game.player.velocity.x > 0.0,
            };
            game.enemies.list.clear();
            game.step(input, DT);
            if game.over {
                break;
            }
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
