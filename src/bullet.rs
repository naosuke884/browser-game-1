use macroquad::prelude::*;

use crate::arena::Arena;
use crate::ship::{Ship, SHIP_RADIUS};

pub const BULLET_RADIUS: f32 = 8.0;
/// 機体の最高速より速くし、撃った弾に追いつくには回り込みが要るようにする。
const BULLET_SPEED: f32 = 900.0;
/// 回収の判定は見た目の接触より少し広げ、かすっただけでも取れるようにする。
const RECOVER_RADIUS: f32 = SHIP_RADIUS + BULLET_RADIUS + 8.0;

/// 1 発きりの弾。機体が持っているか、場を飛んでいるかのどちらか。
pub enum Bullet {
    Held,
    Flying(FlyingBullet),
}

pub struct FlyingBullet {
    pub position: Vec2,
    pub velocity: Vec2,
    pub bounces: u32,
    /// 撃った直後は機体と重なっているので、一度回収範囲の外へ出るまで回収できない。
    armed: bool,
}

/// 弾を回収したときの記録。得点倍率の計算に使う。
pub struct Recovery {
    pub bounces: u32,
    pub ship_speed: f32,
}

impl Bullet {
    pub fn update(
        &mut self,
        fire: bool,
        ship: &Ship,
        arena: &Arena,
        dt: f32,
    ) -> Option<Recovery> {
        match self {
            Bullet::Held => {
                if fire {
                    let forward = ship.heading_dir();
                    *self = Bullet::Flying(FlyingBullet {
                        position: ship.position + forward * SHIP_RADIUS,
                        velocity: forward * BULLET_SPEED,
                        bounces: 0,
                        armed: false,
                    });
                }
                None
            }
            Bullet::Flying(bullet) => {
                bullet.position += bullet.velocity * dt;

                // 減速せず、壁で反射し続ける。
                if let Some(hit) = arena.push_inside(bullet.position, BULLET_RADIUS) {
                    bullet.position = hit.position;
                    let into_wall = bullet.velocity.dot(hit.normal);
                    if into_wall < 0.0 {
                        bullet.velocity -= hit.normal * into_wall * 2.0;
                        bullet.bounces += 1;
                    }
                }

                let near_ship = bullet.position.distance(ship.position) < RECOVER_RADIUS;
                if !bullet.armed {
                    bullet.armed = !near_ship;
                    return None;
                }
                if near_ship {
                    let recovery = Recovery {
                        bounces: bullet.bounces,
                        ship_speed: ship.velocity.length(),
                    };
                    *self = Bullet::Held;
                    return Some(recovery);
                }
                None
            }
        }
    }

    pub fn draw(&self, ship: &Ship) {
        let color = Color::from_rgba(255, 200, 80, 255);
        match self {
            Bullet::Held => {
                let nose = ship.position + ship.heading_dir() * SHIP_RADIUS * 1.4;
                draw_circle(nose.x, nose.y, BULLET_RADIUS * 0.6, color);
            }
            Bullet::Flying(bullet) => {
                draw_circle(bullet.position.x, bullet.position.y, BULLET_RADIUS, color);
            }
        }
    }

    /// 飛んでいる弾が画面外にあるとき、画面の縁にその方向を示す印を出す。
    /// `view` は画面に映っているワールドの範囲。
    pub fn draw_offscreen_marker(&self, view: Rect) {
        let Bullet::Flying(bullet) = self else {
            return;
        };
        if view.contains(bullet.position) {
            return;
        }
        let center = view.center();
        let to_bullet = bullet.position - center;
        // 縁から少し内側の長方形に、中心から弾への線分が交わる点に置く。
        let half = vec2(view.w, view.h) / 2.0 - Vec2::splat(24.0);
        let scale = (half.x / to_bullet.x.abs()).min(half.y / to_bullet.y.abs());
        let marker = center + to_bullet * scale;
        let dir = to_bullet.normalize();
        let side = dir.perp();
        draw_triangle(
            marker + dir * 14.0,
            marker - dir * 8.0 + side * 9.0,
            marker - dir * 8.0 - side * 9.0,
            Color::from_rgba(255, 200, 80, 200),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    fn ship_at(position: Vec2) -> Ship {
        let mut ship = Ship::new(position);
        ship.heading = 0.0;
        ship
    }

    fn flying(bullet: &Bullet) -> &FlyingBullet {
        match bullet {
            Bullet::Flying(b) => b,
            Bullet::Held => panic!("弾が飛んでいない"),
        }
    }

    #[test]
    fn fires_forward_only_when_held() {
        let arena = Arena::new(2000.0, 2000.0);
        let ship = ship_at(Vec2::ZERO);
        let mut bullet = Bullet::Held;
        bullet.update(true, &ship, &arena, DT);
        let velocity = flying(&bullet).velocity;
        assert_eq!(velocity, vec2(BULLET_SPEED, 0.0));

        // 飛んでいる間に撃っても、2 発目は出ず向きも変わらない。
        let ship = Ship::new(Vec2::ZERO);
        bullet.update(true, &ship, &arena, DT);
        assert_eq!(flying(&bullet).velocity, velocity);
    }

    #[test]
    fn not_recovered_right_after_firing() {
        let arena = Arena::new(2000.0, 2000.0);
        let ship = ship_at(Vec2::ZERO);
        let mut bullet = Bullet::Held;
        bullet.update(true, &ship, &arena, DT);
        for _ in 0..3 {
            assert!(bullet.update(false, &ship, &arena, DT).is_none());
        }
        assert!(matches!(bullet, Bullet::Flying(_)));
    }

    #[test]
    fn bounces_off_wall_without_slowing() {
        let arena = Arena::new(400.0, 2000.0);
        let ship = ship_at(Vec2::ZERO);
        let mut bullet = Bullet::Held;
        bullet.update(true, &ship, &arena, DT);
        // 右の壁まで 200 弱。0.4 秒あれば跳ね返って左へ向かう。
        for _ in 0..48 {
            bullet.update(false, &ship_at(vec2(0.0, 900.0)), &arena, DT);
        }
        let b = flying(&bullet);
        assert_eq!(b.bounces, 1);
        assert_eq!(b.velocity, vec2(-BULLET_SPEED, 0.0));
    }

    #[test]
    fn recovered_when_ship_touches_it() {
        let arena = Arena::new(2000.0, 2000.0);
        let mut bullet = Bullet::Held;
        bullet.update(true, &ship_at(Vec2::ZERO), &arena, DT);
        // 機体から離れた所で回収範囲の外に出す。
        let far = ship_at(vec2(0.0, 900.0));
        bullet.update(false, &far, &arena, DT);
        let at = flying(&bullet).position;
        let recovery = bullet.update(false, &ship_at(at), &arena, DT);
        assert!(recovery.is_some());
        assert!(matches!(bullet, Bullet::Held));
    }
}
