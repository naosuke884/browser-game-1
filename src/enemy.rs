use macroquad::prelude::*;
use macroquad::rand::RandGenerator;

pub const ENEMY_RADIUS: f32 = 18.0;
/// 浮遊する敵が左へ流れる速さ。
const FLOATER_SPEED: f32 = 120.0;
const FLOATER_SWAY: f32 = 60.0;
/// 浮遊する敵の揺れの速さ（ラジアン毎秒）。
const FLOATER_SWAY_RATE: f32 = 1.5;
/// 突進する敵が狙いを付けている時間。
const CHARGER_AIM_TIME: f32 = 0.8;
const CHARGER_SPEED: f32 = 700.0;
/// 敵を出す位置の、カメラ左端からの距離。16:9 の画面の右端より少し外。
const SPAWN_OFFSET: f32 = 3500.0;
/// カメラ左端よりこれだけ左へ流れた敵は消す。
const DISCARD_BEHIND: f32 = 200.0;
/// この距離（m）を過ぎるまで敵を出さない。
const START_DISTANCE: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// 上下に揺れながら左へ流れる。
    Floater { base_y: f32, phase: f32 },
    /// 画面の右端で自機の高さを狙い、残り時間が 0 になったら左へ突っ込む。
    Charger { aim_left: f32 },
}

pub struct Enemy {
    pub position: Vec2,
    kind: Kind,
}

impl Enemy {
    fn update(&mut self, camera_x: f32, player: Vec2, dt: f32) {
        match &mut self.kind {
            Kind::Floater { base_y, phase } => {
                *phase += FLOATER_SWAY_RATE * dt;
                self.position.x -= FLOATER_SPEED * dt;
                self.position.y = *base_y + phase.sin() * FLOATER_SWAY;
            }
            Kind::Charger { aim_left } => {
                if *aim_left > 0.0 {
                    // 狙っている間は画面の右端に留まり、自機の高さへ寄っていく。
                    *aim_left -= dt;
                    self.position.x = camera_x + SPAWN_OFFSET - 200.0;
                    let blend = 1.0 - (-4.0 * dt).exp();
                    self.position.y += (player.y - self.position.y) * blend;
                } else {
                    self.position.x -= CHARGER_SPEED * dt;
                }
            }
        }
    }

    fn draw(&self) {
        let (x, y) = (self.position.x, self.position.y);
        match self.kind {
            Kind::Floater { .. } => {
                draw_circle(x, y, ENEMY_RADIUS, Color::from_rgba(120, 220, 140, 255));
            }
            Kind::Charger { aim_left } => {
                let color = Color::from_rgba(240, 90, 110, 255);
                draw_circle(x, y, ENEMY_RADIUS, color);
                if aim_left > 0.0 {
                    // 突っ込む向きを予告する。
                    draw_line(x, y, x - 400.0, y, 2.0, Color { a: 0.4, ..color });
                }
            }
        }
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
    pub fn update(&mut self, camera_x: f32, player: Vec2, distance: f32, dt: f32) {
        if distance >= START_DISTANCE {
            self.until_spawn -= dt;
            if self.until_spawn <= 0.0 {
                self.until_spawn = spawn_interval(distance);
                self.spawn(camera_x, distance);
            }
        }
        for enemy in &mut self.list {
            enemy.update(camera_x, player, dt);
        }
        self.list
            .retain(|e| e.position.x > camera_x - DISCARD_BEHIND);
    }

    fn spawn(&mut self, camera_x: f32, distance: f32) {
        let y = self.rng.gen_range(250.0, 750.0);
        let kind = if self.rng.gen_range(0.0, 1.0) < charger_chance(distance) {
            Kind::Charger {
                aim_left: CHARGER_AIM_TIME,
            }
        } else {
            Kind::Floater {
                base_y: y,
                phase: self.rng.gen_range(0.0, std::f32::consts::TAU),
            }
        };
        self.list.push(Enemy {
            position: vec2(camera_x + SPAWN_OFFSET, y),
            kind,
        });
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

/// 突進する敵が出る確率。100 m で 0、600 m 以降は半分。
fn charger_chance(distance: f32) -> f32 {
    ((distance - START_DISTANCE) / 500.0).clamp(0.0, 1.0) * 0.5
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
        Enemy {
            position,
            kind: Kind::Floater {
                base_y: position.y,
                phase: 0.0,
            },
        }
    }

    #[test]
    fn no_enemies_before_start_distance() {
        let mut enemies = Enemies::new(1);
        for _ in 0..1200 {
            enemies.update(0.0, Vec2::ZERO, START_DISTANCE - 1.0, DT);
        }
        assert!(enemies.list.is_empty());
    }

    #[test]
    fn spawns_ahead_after_start_distance() {
        let mut enemies = Enemies::new(1);
        enemies.update(1000.0, Vec2::ZERO, START_DISTANCE, DT);
        assert_eq!(enemies.list.len(), 1);
        assert!(enemies.list[0].position.x > 1000.0 + 3300.0);
    }

    #[test]
    fn spawning_gets_faster_and_chargers_appear_later() {
        assert!(spawn_interval(1000.0) < spawn_interval(START_DISTANCE));
        assert_eq!(charger_chance(START_DISTANCE), 0.0);
        assert_eq!(charger_chance(2000.0), 0.5);
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
    fn charger_waits_then_rushes_left() {
        let mut enemy = Enemy {
            position: vec2(1500.0, 300.0),
            kind: Kind::Charger {
                aim_left: CHARGER_AIM_TIME,
            },
        };
        for _ in 0..(CHARGER_AIM_TIME / DT) as usize + 1 {
            enemy.update(0.0, vec2(0.0, 600.0), DT);
        }
        let x = enemy.position.x;
        assert!(enemy.position.y > 500.0);
        enemy.update(0.0, vec2(0.0, 600.0), DT);
        assert!(enemy.position.x < x);
    }

    #[test]
    fn passed_enemies_are_discarded() {
        let mut enemies = Enemies::new(1);
        enemies.list.push(floater_at(vec2(-500.0, 400.0)));
        enemies.update(0.0, Vec2::ZERO, 0.0, DT);
        assert!(enemies.list.is_empty());
    }
}
