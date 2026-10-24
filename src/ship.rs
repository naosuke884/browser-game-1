use macroquad::prelude::*;

use crate::arena::Arena;

pub const SHIP_RADIUS: f32 = 14.0;

/// カーソルがこの距離より近いときは操縦に反応しない（機首も速度目標も変えない）。
const DEAD_ZONE: f32 = 40.0;
/// カーソルがこの距離以上離れると最高速を目指す。
const FULL_SPEED_DISTANCE: f32 = 320.0;
const MAX_SPEED: f32 = 520.0;
/// 機首が向きを変えられる速さ（ラジアン毎秒）。
const TURN_RATE: f32 = 5.0;
/// 速度が目標に近づく速さ。小さいほど慣性が強い。
const RESPONSIVENESS: f32 = 6.0;

pub struct Ship {
    pub position: Vec2,
    pub velocity: Vec2,
    /// 機首の向き（ラジアン、+x が 0、y 下向きなので時計回りが正）。
    pub heading: f32,
}

impl Ship {
    pub fn new(position: Vec2) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            heading: -std::f32::consts::FRAC_PI_2,
        }
    }

    pub fn heading_dir(&self) -> Vec2 {
        Vec2::from_angle(self.heading)
    }

    /// `aim` はカーソルのワールド座標。
    pub fn update(&mut self, aim: Vec2, arena: &Arena, dt: f32) {
        let to_aim = aim - self.position;
        let distance = to_aim.length();

        let target_speed = if distance > DEAD_ZONE {
            let target_heading = to_aim.y.atan2(to_aim.x);
            let diff = wrap_angle(target_heading - self.heading);
            let max_step = TURN_RATE * dt;
            self.heading = wrap_angle(self.heading + diff.clamp(-max_step, max_step));

            let t = ((distance - DEAD_ZONE) / (FULL_SPEED_DISTANCE - DEAD_ZONE)).clamp(0.0, 1.0);
            MAX_SPEED * t
        } else {
            0.0
        };

        let target_velocity = self.heading_dir() * target_speed;
        let blend = 1.0 - (-RESPONSIVENESS * dt).exp();
        self.velocity += (target_velocity - self.velocity) * blend;
        self.position += self.velocity * dt;

        // 壁では止まらず、壁に沿った成分だけ残して滑らせる。
        if let Some(hit) = arena.push_inside(self.position, SHIP_RADIUS) {
            self.position = hit.position;
            let into_wall = self.velocity.dot(hit.normal);
            if into_wall < 0.0 {
                self.velocity -= hit.normal * into_wall;
            }
        }
    }

    pub fn draw(&self) {
        let forward = self.heading_dir();
        let side = forward.perp();
        let nose = self.position + forward * SHIP_RADIUS * 1.4;
        let left = self.position - forward * SHIP_RADIUS + side * SHIP_RADIUS;
        let right = self.position - forward * SHIP_RADIUS - side * SHIP_RADIUS;
        draw_triangle(nose, left, right, Color::from_rgba(240, 240, 250, 255));
    }

    /// 操縦に反応しない範囲を薄く表示する。
    pub fn draw_dead_zone(&self) {
        draw_circle_lines(
            self.position.x,
            self.position.y,
            DEAD_ZONE,
            1.0,
            Color::from_rgba(255, 255, 255, 40),
        );
    }
}

fn wrap_angle(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (angle + PI).rem_euclid(TAU) - PI
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    fn run(ship: &mut Ship, aim: Vec2, seconds: f32) {
        let arena = Arena::new(4000.0, 4000.0);
        for _ in 0..(seconds / DT) as usize {
            ship.update(aim, &arena, DT);
        }
    }

    #[test]
    fn far_cursor_reaches_max_speed() {
        let mut ship = Ship::new(Vec2::ZERO);
        run(&mut ship, vec2(0.0, -10_000.0), 2.0);
        assert!((ship.velocity.length() - MAX_SPEED).abs() < 1.0);
    }

    #[test]
    fn cursor_in_dead_zone_stops_without_turning() {
        let mut ship = Ship::new(Vec2::ZERO);
        let heading = ship.heading;
        run(&mut ship, vec2(10.0, 0.0), 1.0);
        assert_eq!(ship.heading, heading);
        assert!(ship.velocity.length() < 1.0);
    }

    #[test]
    fn turning_is_rate_limited() {
        let mut ship = Ship::new(Vec2::ZERO);
        // 真後ろを向かせる。1 フレームで回れるのは TURN_RATE * DT まで。
        let before = ship.heading;
        ship.update(vec2(0.0, 1000.0), &Arena::new(4000.0, 4000.0), DT);
        assert!((wrap_angle(ship.heading - before)).abs() <= TURN_RATE * DT + 1e-6);
    }

    #[test]
    fn slides_along_wall() {
        let arena = Arena::new(400.0, 400.0);
        let mut ship = Ship::new(vec2(0.0, -150.0));
        // 右上の遠くを狙い、上の壁に押し付けながら右へ進ませる。
        for _ in 0..60 {
            ship.update(vec2(10_000.0, -10_000.0), &arena, DT);
        }
        assert_eq!(ship.position.y, -200.0 + SHIP_RADIUS);
        assert_eq!(ship.velocity.y, 0.0);
        assert!(ship.velocity.x > 0.0);
    }
}
