use macroquad::prelude::*;
use macroquad::rand::RandGenerator;

/// 敵の大きさ。ワイヤーを当てる的と、触れたら終わる判定の両方に使う。
/// 画面を引いて広く映しているので、狙いやすいよう大きめにする。
pub const ENEMY_RADIUS: f32 = 32.0;
/// 敵が左へ流れる速さ。
const DRIFT_SPEED: f32 = 120.0;
/// 上下に揺れる幅。
const SWAY: f32 = 60.0;
/// 揺れの速さ（ラジアン毎秒）。
const SWAY_RATE: f32 = 1.5;
/// 敵は、最も右まで進んだときのカメラ左端からこれだけ先に出す。16:9 の画面の右端より少し外。
const SPAWN_OFFSET: f32 = 3500.0;
/// カメラ左端よりこれだけ左へ流れた敵は消す。
const DISCARD_BEHIND: f32 = 200.0;
/// 開始時のカメラ左端からこれだけ（100 m）進むまで敵を出さない。
const START_GAP: f32 = 1000.0;
/// 出す位置の間隔の範囲。最初は広く、進むほど狭める。
/// 出る間隔は時間ではなく位置で決め、同じ種なら誰が遊んでも同じ所に同じ敵が出るようにする。
const SPAWN_SPACING: (f32, f32) = (750.0, 250.0);
/// 最初の敵からこれだけ先で、出す位置の間隔が最も狭くなる。
const SPACING_RAMP: f32 = 15_000.0;

/// 上下にゆっくり揺れながら、右から左へ流れてくる敵。
#[derive(Debug, PartialEq)]
pub struct Enemy {
    pub position: Vec2,
    /// 揺れの中心の高さ。
    base_y: f32,
    /// 揺れの位相（ラジアン）。
    phase: f32,
}

impl Enemy {
    pub fn new(position: Vec2, phase: f32) -> Self {
        Self {
            position,
            base_y: position.y,
            phase,
        }
    }

    fn update(&mut self, dt: f32) {
        self.phase += SWAY_RATE * dt;
        self.position.x -= DRIFT_SPEED * dt;
        self.position.y = self.base_y + self.phase.sin() * SWAY;
    }

    fn draw(&self) {
        draw_circle(
            self.position.x,
            self.position.y,
            ENEMY_RADIUS,
            Color::from_rgba(120, 220, 140, 255),
        );
    }
}

/// 場の敵と、進んだ距離に応じて出現を増やす仕組み。
pub struct Enemies {
    pub list: Vec<Enemy>,
    /// 最初の敵を出す位置のワールド x 座標。
    first_x: f32,
    /// 次の敵を出す位置のワールド x 座標。
    next_x: f32,
    rng: RandGenerator,
}

impl Enemies {
    /// `start_front` は開始時のカメラ左端のワールド x 座標。
    pub fn new(seed: u64, start_front: f32) -> Self {
        let rng = RandGenerator::new();
        rng.srand(seed);
        let first_x = start_front + START_GAP + SPAWN_OFFSET;
        Self {
            list: Vec::new(),
            first_x,
            next_x: first_x,
            rng,
        }
    }

    /// `front` は最も右まで進んだときのカメラ左端、`camera_x` は今のカメラ左端のワールド x 座標。
    pub fn update(&mut self, front: f32, camera_x: f32, dt: f32) {
        while front + SPAWN_OFFSET >= self.next_x {
            self.spawn();
        }
        for enemy in &mut self.list {
            enemy.update(dt);
        }
        self.list
            .retain(|e| e.position.x > camera_x - DISCARD_BEHIND);
    }

    fn spawn(&mut self) {
        let y = self.rng.gen_range(250.0, 900.0);
        let phase = self.rng.gen_range(0.0, std::f32::consts::TAU);
        self.list.push(Enemy::new(vec2(self.next_x, y), phase));
        self.next_x += spawn_spacing(self.next_x - self.first_x);
    }

    /// `origin` から `dir` へ伸ばした線が最初に当たる敵の番号と、当たる点までの距離。
    pub fn raycast(&self, origin: Vec2, dir: Vec2, max_len: f32) -> Option<(usize, f32)> {
        let dir = dir.normalize_or_zero();
        self.list
            .iter()
            .enumerate()
            .filter_map(|(i, e)| ray_circle(origin, dir, e.position, ENEMY_RADIUS).map(|t| (i, t)))
            .filter(|&(_, t)| t <= max_len)
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// 円 (`center`, `radius`) に触れている敵がいるか。
    pub fn any_touching(&self, center: Vec2, radius: f32) -> bool {
        self.list
            .iter()
            .any(|e| e.position.distance(center) < ENEMY_RADIUS + radius)
    }

    pub fn draw(&self) {
        for enemy in &self.list {
            enemy.draw();
        }
    }
}

/// 最初の敵から `past` だけ先で、次の敵までに空ける間隔。
fn spawn_spacing(past: f32) -> f32 {
    let t = (past / SPACING_RAMP).clamp(0.0, 1.0);
    SPAWN_SPACING.0 + (SPAWN_SPACING.1 - SPAWN_SPACING.0) * t
}

/// 線と円が最初に交わる距離。始点が円の中なら 0。
fn ray_circle(origin: Vec2, dir: Vec2, center: Vec2, radius: f32) -> Option<f32> {
    let to_center = center - origin;
    if to_center.length_squared() <= radius * radius {
        return Some(0.0);
    }
    let along = to_center.dot(dir);
    if along < 0.0 {
        return None;
    }
    let off_sq = to_center.length_squared() - along * along;
    let r_sq = radius * radius;
    if off_sq > r_sq {
        return None;
    }
    Some(along - (r_sq - off_sq).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    fn floater_at(position: Vec2) -> Enemy {
        Enemy::new(position, 0.0)
    }

    #[test]
    fn no_enemies_before_start_gap() {
        let mut enemies = Enemies::new(1, 0.0);
        for _ in 0..1200 {
            enemies.update(START_GAP - 1.0, 0.0, DT);
        }
        assert!(enemies.list.is_empty());
    }

    #[test]
    fn spawns_ahead_after_start_gap() {
        let mut enemies = Enemies::new(1, 0.0);
        enemies.update(START_GAP, START_GAP, DT);
        assert_eq!(enemies.list.len(), 1);
        assert!(enemies.list[0].position.x > START_GAP + 3300.0);
    }

    #[test]
    fn spawning_gets_denser() {
        assert!(spawn_spacing(10_000.0) < spawn_spacing(0.0));
    }

    /// 出る位置は進んだ位置だけで決まり、進み方や更新の刻みには依らない。
    #[test]
    fn same_seed_spawns_same_enemies_regardless_of_pace() {
        let mut slow = Enemies::new(7, 0.0);
        let mut x = 0.0;
        while x < 20_000.0 {
            x += 7.0;
            slow.update(x, 0.0, 0.0);
        }
        let mut fast = Enemies::new(7, 0.0);
        for x in [3000.0, 3001.0, 12_345.0, 20_000.0] {
            fast.update(x, 0.0, 0.0);
        }
        slow.update(20_000.0, 0.0, 0.0);
        assert!(slow.list.len() > 20);
        assert_eq!(slow.list, fast.list);

        let mut other = Enemies::new(8, 0.0);
        other.update(20_000.0, 0.0, 0.0);
        assert_ne!(slow.list, other.list);
    }

    #[test]
    fn drifts_left_while_swaying() {
        let mut enemy = floater_at(vec2(1000.0, 500.0));
        let mut highest = f32::MAX;
        let mut lowest = f32::MIN;
        for _ in 0..600 {
            enemy.update(DT);
            highest = highest.min(enemy.position.y);
            lowest = lowest.max(enemy.position.y);
        }
        // 600 刻み = 5 秒。
        assert!((enemy.position.x - (1000.0 - DRIFT_SPEED * 5.0)).abs() < 1.0);
        assert!(lowest - highest > SWAY);
    }

    #[test]
    fn raycast_hits_nearest_enemy() {
        let mut enemies = Enemies::new(1, 0.0);
        enemies.list.push(floater_at(vec2(300.0, 0.0)));
        enemies.list.push(floater_at(vec2(200.0, 0.0)));
        let (index, t) = enemies.raycast(Vec2::ZERO, vec2(1.0, 0.0), 600.0).unwrap();
        assert_eq!(index, 1);
        assert_eq!(t, 200.0 - ENEMY_RADIUS);
        assert!(enemies
            .raycast(Vec2::ZERO, vec2(-1.0, 0.0), 600.0)
            .is_none());
        assert!(enemies.raycast(Vec2::ZERO, vec2(1.0, 0.0), 100.0).is_none());
    }

    #[test]
    fn passed_enemies_are_discarded() {
        let mut enemies = Enemies::new(1, 0.0);
        enemies.list.push(floater_at(vec2(-500.0, 400.0)));
        enemies.update(0.0, 0.0, DT);
        assert!(enemies.list.is_empty());
    }
}
