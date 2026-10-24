use macroquad::prelude::*;
use macroquad::rand::RandGenerator;

use crate::arena::Arena;

/// 出現の予告を出してから当たるようになるまでの時間。
const SPAWN_WARNING: f32 = 0.8;
/// 機体からこれ以上離れた所にだけ出す。
const MIN_SPAWN_DISTANCE: f32 = 450.0;
const MAX_ENEMIES: usize = 60;

const CHASER_SPEED: f32 = 150.0;
const BOUNCER_SPEED: f32 = 260.0;
const DASHER_APPROACH_SPEED: f32 = 80.0;
const DASHER_DASH_SPEED: f32 = 900.0;
const DASHER_WAIT: f32 = 1.2;
const DASHER_AIM: f32 = 0.5;
const DASHER_DASH: f32 = 0.45;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// 機体へゆっくり寄ってくる。
    Chaser,
    /// まっすぐ飛び、壁で跳ね返る。
    Bouncer,
    /// 止まって狙いを付け、その時点の機体の位置へ突っ込む。
    Dasher,
}

enum DashPhase {
    Wait(f32),
    Aim { left: f32, direction: Vec2 },
    Dash(f32),
}

pub struct Enemy {
    pub kind: Kind,
    pub position: Vec2,
    pub radius: f32,
    velocity: Vec2,
    /// 0 より大きい間は予告中で、当たらない。
    warning: f32,
    dash: DashPhase,
}

impl Enemy {
    fn new(kind: Kind, position: Vec2, rng: &RandGenerator) -> Self {
        let radius = match kind {
            Kind::Chaser => 16.0,
            Kind::Bouncer => 14.0,
            Kind::Dasher => 18.0,
        };
        let velocity = match kind {
            Kind::Bouncer => {
                Vec2::from_angle(rng.gen_range(0.0, std::f32::consts::TAU)) * BOUNCER_SPEED
            }
            _ => Vec2::ZERO,
        };
        Self {
            kind,
            position,
            radius,
            velocity,
            warning: SPAWN_WARNING,
            dash: DashPhase::Wait(DASHER_WAIT),
        }
    }

    pub fn is_active(&self) -> bool {
        self.warning <= 0.0
    }

    fn update(&mut self, ship_position: Vec2, arena: &Arena, dt: f32) {
        if self.warning > 0.0 {
            self.warning -= dt;
            return;
        }
        let to_ship = (ship_position - self.position).normalize_or_zero();
        match self.kind {
            Kind::Chaser => self.velocity = to_ship * CHASER_SPEED,
            Kind::Bouncer => {}
            Kind::Dasher => {
                self.dash = match self.dash {
                    DashPhase::Wait(left) => {
                        self.velocity = to_ship * DASHER_APPROACH_SPEED;
                        if left - dt <= 0.0 {
                            DashPhase::Aim {
                                left: DASHER_AIM,
                                direction: to_ship,
                            }
                        } else {
                            DashPhase::Wait(left - dt)
                        }
                    }
                    DashPhase::Aim { left, direction } => {
                        self.velocity = Vec2::ZERO;
                        if left - dt <= 0.0 {
                            self.velocity = direction * DASHER_DASH_SPEED;
                            DashPhase::Dash(DASHER_DASH)
                        } else {
                            DashPhase::Aim {
                                left: left - dt,
                                direction,
                            }
                        }
                    }
                    DashPhase::Dash(left) => {
                        if left - dt <= 0.0 {
                            DashPhase::Wait(DASHER_WAIT)
                        } else {
                            DashPhase::Dash(left - dt)
                        }
                    }
                };
            }
        }

        self.position += self.velocity * dt;
        if let Some(hit) = arena.push_inside(self.position, self.radius) {
            self.position = hit.position;
            let into_wall = self.velocity.dot(hit.normal);
            if into_wall < 0.0 {
                // 跳ね返りの敵は反射、ほかは壁に沿って滑る。
                let factor = if self.kind == Kind::Bouncer { 2.0 } else { 1.0 };
                self.velocity -= hit.normal * into_wall * factor;
            }
        }
    }

    fn draw(&self) {
        let color = match self.kind {
            Kind::Chaser => Color::from_rgba(240, 90, 110, 255),
            Kind::Bouncer => Color::from_rgba(120, 220, 140, 255),
            Kind::Dasher => Color::from_rgba(200, 120, 255, 255),
        };
        let (x, y) = (self.position.x, self.position.y);
        if !self.is_active() {
            draw_circle_lines(x, y, self.radius, 2.0, Color { a: 0.5, ..color });
            return;
        }
        draw_circle(x, y, self.radius, color);
        if let DashPhase::Aim { direction, .. } = self.dash {
            let end = self.position + direction * 260.0;
            draw_line(x, y, end.x, end.y, 2.0, Color { a: 0.5, ..color });
        }
    }
}

/// 場の敵全体と、時間とともに出現を増やす仕組み。
pub struct Enemies {
    pub list: Vec<Enemy>,
    elapsed: f32,
    until_spawn: f32,
    rng: RandGenerator,
}

impl Enemies {
    pub fn new(seed: u64) -> Self {
        let rng = RandGenerator::new();
        rng.srand(seed);
        Self {
            list: Vec::new(),
            elapsed: 0.0,
            until_spawn: 1.0,
            rng,
        }
    }

    pub fn update(&mut self, ship_position: Vec2, arena: &Arena, dt: f32) {
        self.elapsed += dt;
        self.until_spawn -= dt;
        if self.until_spawn <= 0.0 {
            self.until_spawn += spawn_interval(self.elapsed);
            if self.list.len() < MAX_ENEMIES {
                self.spawn(ship_position, arena);
            }
        }
        for enemy in &mut self.list {
            enemy.update(ship_position, arena, dt);
        }
    }

    fn spawn(&mut self, ship_position: Vec2, arena: &Arena) {
        let kind = self.pick_kind();
        let limit = arena.half_size - Vec2::splat(60.0);
        // 遠い点が見つかるまで何度か引き直す。場が狭すぎて見つからなければ出さない。
        for _ in 0..20 {
            let position = vec2(
                self.rng.gen_range(-limit.x, limit.x),
                self.rng.gen_range(-limit.y, limit.y),
            );
            if position.distance(ship_position) >= MIN_SPAWN_DISTANCE {
                self.list.push(Enemy::new(kind, position, &self.rng));
                return;
            }
        }
    }

    fn pick_kind(&self) -> Kind {
        let mut pool = vec![Kind::Chaser];
        if self.elapsed >= 30.0 {
            pool.push(Kind::Bouncer);
        }
        if self.elapsed >= 60.0 {
            pool.push(Kind::Dasher);
        }
        pool[self.rng.gen_range(0, pool.len())]
    }

    /// 円 (`center`, `radius`) に触れている当たる状態の敵を消し、消した数を返す。
    pub fn remove_touching(&mut self, center: Vec2, radius: f32) -> u32 {
        let before = self.list.len();
        self.list
            .retain(|e| !(e.is_active() && e.position.distance(center) < e.radius + radius));
        (before - self.list.len()) as u32
    }

    /// 円 (`center`, `radius`) に触れている当たる状態の敵がいるか。
    pub fn any_touching(&self, center: Vec2, radius: f32) -> bool {
        self.list
            .iter()
            .any(|e| e.is_active() && e.position.distance(center) < e.radius + radius)
    }

    pub fn draw(&self) {
        for enemy in &self.list {
            enemy.draw();
        }
    }
}

/// 出現の間隔。3 分かけて 2 秒から 0.5 秒まで縮める。
fn spawn_interval(elapsed: f32) -> f32 {
    let t = (elapsed / 180.0).min(1.0);
    2.0 + (0.5 - 2.0) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    fn run(enemies: &mut Enemies, ship: Vec2, seconds: f32) {
        let arena = Arena::new(2400.0, 1600.0);
        for _ in 0..(seconds / DT) as usize {
            enemies.update(ship, &arena, DT);
        }
    }

    #[test]
    fn spawns_far_from_ship() {
        let mut enemies = Enemies::new(1);
        run(&mut enemies, Vec2::ZERO, 20.0);
        assert!(!enemies.list.is_empty());
        // 追跡の敵は寄ってくるので、出現直後（予告中）の位置で確かめる。
        let mut enemies = Enemies::new(2);
        run(&mut enemies, Vec2::ZERO, 1.01);
        let first = &enemies.list[0];
        assert!(!first.is_active());
        assert!(first.position.length() >= MIN_SPAWN_DISTANCE);
    }

    #[test]
    fn only_chasers_early_and_all_kinds_later() {
        let mut enemies = Enemies::new(3);
        run(&mut enemies, Vec2::ZERO, 29.0);
        assert!(enemies.list.iter().all(|e| e.kind == Kind::Chaser));

        let mut enemies = Enemies::new(4);
        enemies.elapsed = 60.0;
        let kinds: Vec<Kind> = (0..60).map(|_| enemies.pick_kind()).collect();
        for kind in [Kind::Chaser, Kind::Bouncer, Kind::Dasher] {
            assert!(kinds.contains(&kind));
        }
    }

    #[test]
    fn spawn_interval_shrinks_over_time() {
        assert_eq!(spawn_interval(0.0), 2.0);
        assert!(spawn_interval(90.0) < 2.0);
        assert_eq!(spawn_interval(180.0), 0.5);
        assert_eq!(spawn_interval(1000.0), 0.5);
    }

    #[test]
    fn warning_enemies_are_harmless() {
        let rng = RandGenerator::new();
        let mut enemies = Enemies::new(5);
        enemies
            .list
            .push(Enemy::new(Kind::Chaser, Vec2::ZERO, &rng));
        assert!(!enemies.any_touching(Vec2::ZERO, 10.0));
        assert_eq!(enemies.remove_touching(Vec2::ZERO, 10.0), 0);

        enemies.list[0].warning = 0.0;
        assert!(enemies.any_touching(Vec2::ZERO, 10.0));
        assert_eq!(enemies.remove_touching(Vec2::ZERO, 10.0), 1);
        assert!(enemies.list.is_empty());
    }

    #[test]
    fn chaser_moves_toward_ship() {
        let rng = RandGenerator::new();
        let arena = Arena::new(2400.0, 1600.0);
        let mut enemy = Enemy::new(Kind::Chaser, vec2(500.0, 0.0), &rng);
        enemy.warning = 0.0;
        enemy.update(Vec2::ZERO, &arena, 1.0);
        assert_eq!(enemy.position, vec2(500.0 - CHASER_SPEED, 0.0));
    }
}
