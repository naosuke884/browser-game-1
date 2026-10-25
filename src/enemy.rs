use macroquad::prelude::*;
use macroquad::rand::RandGenerator;

pub const ENEMY_RADIUS: f32 = 18.0;
/// 敵が左へ流れる速さ。
const DRIFT_SPEED: f32 = 120.0;
/// 上下に揺れる幅。
const SWAY: f32 = 60.0;
/// 揺れの速さ（ラジアン毎秒）。
const SWAY_RATE: f32 = 1.5;
/// 敵を出す位置の、カメラ左端からの距離。16:9 の画面の右端より少し外。
const SPAWN_OFFSET: f32 = 3500.0;
/// カメラ左端よりこれだけ左へ流れた敵は消す。
const DISCARD_BEHIND: f32 = 200.0;
/// この距離（m）を過ぎるまで敵を出さない。
const START_DISTANCE: f32 = 100.0;

/// 上下にゆっくり揺れながら、右から左へ流れてくる敵。
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
    until_spawn: f32,
    rng: RandGenerator,
}

impl Enemies {
    pub fn new(seed: u64) -> Self {
        let rng = RandGenerator::new();
        rng.srand(seed);
        Self {
            list: Vec::new(),
            until_spawn: 0.0,
            rng,
        }
    }

    /// `distance` は進んだ距離（m）。
    pub fn update(&mut self, camera_x: f32, distance: f32, dt: f32) {
        if distance >= START_DISTANCE {
            self.until_spawn -= dt;
            if self.until_spawn <= 0.0 {
                self.until_spawn = spawn_interval(distance);
                self.spawn(camera_x);
            }
        }
        for enemy in &mut self.list {
            enemy.update(dt);
        }
        self.list
            .retain(|e| e.position.x > camera_x - DISCARD_BEHIND);
    }

    fn spawn(&mut self, camera_x: f32) {
        let y = self.rng.gen_range(250.0, 900.0);
        let phase = self.rng.gen_range(0.0, std::f32::consts::TAU);
        self.list
            .push(Enemy::new(vec2(camera_x + SPAWN_OFFSET, y), phase));
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

/// 出現の間隔（秒）。100 m で 2.5 秒、1600 m で 0.8 秒まで縮める。
fn spawn_interval(distance: f32) -> f32 {
    let t = ((distance - START_DISTANCE) / 1500.0).clamp(0.0, 1.0);
    2.5 + (0.8 - 2.5) * t
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
    fn no_enemies_before_start_distance() {
        let mut enemies = Enemies::new(1);
        for _ in 0..1200 {
            enemies.update(0.0, START_DISTANCE - 1.0, DT);
        }
        assert!(enemies.list.is_empty());
    }

    #[test]
    fn spawns_ahead_after_start_distance() {
        let mut enemies = Enemies::new(1);
        enemies.update(1000.0, START_DISTANCE, DT);
        assert_eq!(enemies.list.len(), 1);
        assert!(enemies.list[0].position.x > 1000.0 + 3300.0);
    }

    #[test]
    fn spawning_gets_faster() {
        assert!(spawn_interval(1000.0) < spawn_interval(START_DISTANCE));
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
        let mut enemies = Enemies::new(1);
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
        let mut enemies = Enemies::new(1);
        enemies.list.push(floater_at(vec2(-500.0, 400.0)));
        enemies.update(0.0, 0.0, DT);
        assert!(enemies.list.is_empty());
    }
}
