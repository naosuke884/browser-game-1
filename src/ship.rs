use macroquad::prelude::*;

use crate::arena::Arena;

pub const SHIP_RADIUS: f32 = 14.0;

/// カーソルがこの距離より近いときは操縦に反応しない（機首も速度目標も変えない）。
const DEAD_ZONE: f32 = 40.0;
/// カーソルがこの距離以上離れると最高速を目指す。
const FULL_SPEED_DISTANCE: f32 = 320.0;
const MAX_SPEED: f32 = 520.0;
/// 機首が向きを変えられる速さ（ラジアン毎秒）。ドリフト中も同じ上限。
const TURN_RATE: f32 = 5.0;
/// 速度が目標に近づく速さ。小さいほど慣性が強い。
const RESPONSIVENESS: f32 = 6.0;

/// ブースト中の目標速度。通常の最高速の 2 倍で、短くても差が分かる。
const BOOST_SPEED: f32 = MAX_SPEED * 2.0;
/// ブーストが速度を上げている時間（秒）。
const BOOST_DURATION: f32 = 0.32;
/// ブーストが終わってから、次に使えるまでの時間（秒）。
const BOOST_COOLDOWN: f32 = 1.15;
/// 短いブーストのあいだに最高速へ届くよう、通常より反応を強くする。
const BOOST_RESPONSIVENESS: f32 = 18.0;
/// これより遅いときは進行方向が定まっていないとみなし、ブーストは機首の向きへ出す。
const MIN_TRAVEL_SPEED: f32 = 1.0;

/// 1 フレーム分の操縦入力。キーの読み取りは呼び出し側が行う。
#[derive(Clone, Copy, Default)]
pub struct ShipInput {
    /// Shift を押しているあいだ真。速度と進行方向を保ち、機首だけマウスへ向ける。
    pub drift: bool,
    /// Space を押したフレームだけ真。クールダウン中は無視する。
    pub boost: bool,
}

pub struct Ship {
    pub position: Vec2,
    pub velocity: Vec2,
    /// 機首の向き（ラジアン、+x が 0、y 下向きなので時計回りが正）。
    pub heading: f32,
    /// 直前の更新でドリフトしていたか。描画用。
    pub drifting: bool,
    /// ブーストの残り時間（秒）。0 なら発動していない。
    pub boost_remaining: f32,
    /// 次のブーストまでの待ち時間（秒）。0 なら使用できる。
    pub boost_cooldown: f32,
}

impl Ship {
    pub fn new(position: Vec2) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            heading: -std::f32::consts::FRAC_PI_2,
            drifting: false,
            boost_remaining: 0.0,
            boost_cooldown: 0.0,
        }
    }

    pub fn heading_dir(&self) -> Vec2 {
        Vec2::from_angle(self.heading)
    }

    pub fn is_boosting(&self) -> bool {
        self.boost_remaining > 0.0
    }

    /// ブースト表示用。発動中は残り割合（1 が開始時）、待ち時間中は回復割合（1 が再使用可）。
    pub fn boost_charge(&self) -> f32 {
        if self.boost_remaining > 0.0 {
            (self.boost_remaining / BOOST_DURATION).clamp(0.0, 1.0)
        } else if self.boost_cooldown > 0.0 {
            1.0 - (self.boost_cooldown / BOOST_COOLDOWN).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// `aim` はカーソルのワールド座標。
    pub fn update(&mut self, aim: Vec2, input: ShipInput, arena: &Arena, dt: f32) {
        self.drifting = input.drift;
        if input.boost && self.boost_remaining <= 0.0 && self.boost_cooldown <= 0.0 {
            self.boost_remaining = BOOST_DURATION;
        }

        let to_aim = aim - self.position;
        let distance = to_aim.length();
        // デッドゾーンでは機首を変えない。ドリフト中も同じで、機体の上でカーソルが暴れて機首が振れないようにする。
        if distance > DEAD_ZONE {
            let target_heading = to_aim.y.atan2(to_aim.x);
            let diff = wrap_angle(target_heading - self.heading);
            let max_step = TURN_RATE * dt;
            self.heading = wrap_angle(self.heading + diff.clamp(-max_step, max_step));
        }

        let boosting = self.is_boosting();
        // ドリフト中は速度も進行方向も変えない。ブーストだけは、その進行方向のまま速くする。
        if !input.drift || boosting {
            let target_velocity = if input.drift {
                travel_dir(self.velocity, self.heading_dir())
                    * self.velocity.length().max(BOOST_SPEED)
            } else {
                let cruise = if distance > DEAD_ZONE {
                    let t = ((distance - DEAD_ZONE) / (FULL_SPEED_DISTANCE - DEAD_ZONE))
                        .clamp(0.0, 1.0);
                    MAX_SPEED * t
                } else {
                    0.0
                };
                let speed = if boosting {
                    cruise.max(BOOST_SPEED)
                } else {
                    cruise
                };
                self.heading_dir() * speed
            };
            let response = if boosting {
                BOOST_RESPONSIVENESS
            } else {
                RESPONSIVENESS
            };
            let blend = 1.0 - (-response * dt).exp();
            self.velocity += (target_velocity - self.velocity) * blend;
        }

        self.position += self.velocity * dt;

        // 壁では止まらず、壁に沿った成分だけ残して滑らせる。ドリフト中も同じ。
        if let Some(hit) = arena.push_inside(self.position, SHIP_RADIUS) {
            self.position = hit.position;
            let into_wall = self.velocity.dot(hit.normal);
            if into_wall < 0.0 {
                self.velocity -= hit.normal * into_wall;
            }
        }

        if self.boost_remaining > 0.0 {
            self.boost_remaining = (self.boost_remaining - dt).max(0.0);
            if self.boost_remaining == 0.0 {
                self.boost_cooldown = BOOST_COOLDOWN;
            }
        } else if self.boost_cooldown > 0.0 {
            self.boost_cooldown = (self.boost_cooldown - dt).max(0.0);
        }
    }

    pub fn draw(&self) {
        if self.is_boosting() {
            draw_circle(
                self.position.x,
                self.position.y,
                SHIP_RADIUS * 1.7,
                Color::from_rgba(255, 170, 60, 90),
            );
        }

        // 機首と進行方向が分かれるドリフト中だけ、進んでいる向きを線で見せる。
        let speed = self.velocity.length();
        if self.drifting && speed > MIN_TRAVEL_SPEED {
            let dir = self.velocity / speed;
            let length = SHIP_RADIUS * (1.8 + (speed / MAX_SPEED).min(2.0));
            let end = self.position + dir * length;
            draw_line(
                self.position.x,
                self.position.y,
                end.x,
                end.y,
                2.0,
                Color::from_rgba(120, 210, 255, 210),
            );
        }

        let color = if self.is_boosting() {
            Color::from_rgba(255, 214, 120, 255)
        } else if self.drifting {
            Color::from_rgba(150, 214, 255, 255)
        } else {
            Color::from_rgba(240, 240, 250, 255)
        };
        let forward = self.heading_dir();
        let side = forward.perp();
        let nose = self.position + forward * SHIP_RADIUS * 1.4;
        let left = self.position - forward * SHIP_RADIUS + side * SHIP_RADIUS;
        let right = self.position - forward * SHIP_RADIUS - side * SHIP_RADIUS;
        draw_triangle(nose, left, right, color);
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

/// 速度があるときはその向き、止まっているときは機首。ブーストの出す方向に使う。
fn travel_dir(velocity: Vec2, heading_dir: Vec2) -> Vec2 {
    let speed = velocity.length();
    if speed > MIN_TRAVEL_SPEED {
        velocity / speed
    } else {
        heading_dir
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

    fn arena() -> Arena {
        Arena::new(20_000.0, 20_000.0)
    }

    fn run(ship: &mut Ship, aim: Vec2, seconds: f32) {
        run_with(ship, aim, ShipInput::default(), seconds);
    }

    fn run_with(ship: &mut Ship, aim: Vec2, input: ShipInput, seconds: f32) {
        let arena = arena();
        for _ in 0..(seconds / DT) as usize {
            ship.update(aim, input, &arena, DT);
        }
    }

    fn step(ship: &mut Ship, aim: Vec2, input: ShipInput) {
        ship.update(aim, input, &arena(), DT);
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
        step(&mut ship, vec2(0.0, 1000.0), ShipInput::default());
        assert!((wrap_angle(ship.heading - before)).abs() <= TURN_RATE * DT + 1e-6);
    }

    #[test]
    fn slides_along_wall() {
        let arena = Arena::new(400.0, 400.0);
        let mut ship = Ship::new(vec2(0.0, -150.0));
        // 右上の遠くを狙い、上の壁に押し付けながら右へ進ませる。
        for _ in 0..60 {
            ship.update(vec2(10_000.0, -10_000.0), ShipInput::default(), &arena, DT);
        }
        assert_eq!(ship.position.y, -200.0 + SHIP_RADIUS);
        assert_eq!(ship.velocity.y, 0.0);
        assert!(ship.velocity.x > 0.0);
    }

    #[test]
    fn drift_still_slides_along_wall() {
        let arena = Arena::new(400.0, 400.0);
        let mut ship = Ship::new(vec2(0.0, -150.0));
        ship.velocity = vec2(200.0, -400.0);
        for _ in 0..30 {
            ship.update(
                vec2(0.0, -10_000.0),
                ShipInput {
                    drift: true,
                    boost: false,
                },
                &arena,
                DT,
            );
        }
        assert_eq!(ship.position.y, -200.0 + SHIP_RADIUS);
        assert_eq!(ship.velocity, vec2(200.0, 0.0));
    }

    #[test]
    fn boost_does_not_turn_faster() {
        let mut ship = Ship::new(Vec2::ZERO);
        let before = ship.heading;
        step(
            &mut ship,
            vec2(0.0, 1000.0),
            ShipInput {
                drift: false,
                boost: true,
            },
        );
        assert!((wrap_angle(ship.heading - before)).abs() <= TURN_RATE * DT + 1e-6);
    }

    #[test]
    fn drift_keeps_speed_and_direction_while_nose_turns() {
        let mut ship = Ship::new(Vec2::ZERO);
        ship.velocity = vec2(400.0, 0.0);
        ship.heading = 0.0;
        let frames = 20;
        for _ in 0..frames {
            step(
                &mut ship,
                vec2(0.0, -10_000.0),
                ShipInput {
                    drift: true,
                    boost: false,
                },
            );
        }
        assert_eq!(ship.velocity, vec2(400.0, 0.0));
        let expected = -TURN_RATE * DT * frames as f32;
        assert!((ship.heading - expected).abs() < 1e-4);
    }

    #[test]
    fn drift_in_dead_zone_does_not_stop_or_turn() {
        let mut ship = Ship::new(Vec2::ZERO);
        ship.velocity = vec2(450.0, -20.0);
        ship.heading = 0.4;
        let velocity = ship.velocity;
        let heading = ship.heading;
        for _ in 0..60 {
            let aim = ship.position;
            step(
                &mut ship,
                aim,
                ShipInput {
                    drift: true,
                    boost: false,
                },
            );
        }
        assert_eq!(ship.velocity, velocity);
        assert_eq!(ship.heading, heading);
    }

    #[test]
    fn drift_does_not_adopt_cursor_speed() {
        let mut ship = Ship::new(Vec2::ZERO);
        ship.velocity = vec2(100.0, 0.0);
        run_with(
            &mut ship,
            vec2(10_000.0, 0.0),
            ShipInput {
                drift: true,
                boost: false,
            },
            1.0,
        );
        assert_eq!(ship.velocity, vec2(100.0, 0.0));
    }

    #[test]
    fn releasing_drift_steers_again() {
        let mut ship = Ship::new(Vec2::ZERO);
        ship.velocity = vec2(500.0, 0.0);
        ship.heading = 0.0;
        run(&mut ship, vec2(0.0, -50_000.0), 2.0);
        assert!(ship.velocity.y < -MAX_SPEED * 0.9);
        assert!(ship.velocity.x.abs() < 40.0);
    }

    #[test]
    fn boost_while_drifting_speeds_up_along_travel() {
        let mut ship = Ship::new(Vec2::ZERO);
        ship.velocity = vec2(400.0, 0.0);
        ship.heading = 0.0;
        let before = ship.velocity.normalize();
        step(
            &mut ship,
            vec2(0.0, -10_000.0),
            ShipInput {
                drift: true,
                boost: true,
            },
        );
        for _ in 0..40 {
            step(
                &mut ship,
                vec2(0.0, -10_000.0),
                ShipInput {
                    drift: true,
                    boost: false,
                },
            );
        }
        let after = ship.velocity.normalize();
        let cross = after.x * before.y - after.y * before.x;
        assert!(cross.abs() < 1e-3);
        assert!(ship.velocity.length() > MAX_SPEED * 1.9);
        // 機首は上へ回っている。進行方向（+x）とは別。
        assert!(ship.heading < -0.5);
        // ブーストが終わったあとも、ドリフト中は上がった速度を保つ。
        assert!(!ship.is_boosting());
        assert!(ship.drifting);
    }

    #[test]
    fn boost_from_rest_follows_heading_and_is_temporary() {
        let mut ship = Ship::new(Vec2::ZERO);
        let heading = ship.heading;
        let origin = ship.position;
        step(
            &mut ship,
            origin,
            ShipInput {
                drift: false,
                boost: true,
            },
        );
        for _ in 0..30 {
            let aim = ship.position;
            step(&mut ship, aim, ShipInput::default());
        }
        assert_eq!(ship.heading, heading);
        assert!(ship.velocity.y < -MAX_SPEED);
        assert!(ship.velocity.x.abs() < 1e-2);

        run(&mut ship, vec2(0.0, -50_000.0), 3.0);
        assert!((ship.velocity.length() - MAX_SPEED).abs() < 15.0);
        assert_eq!(ship.boost_cooldown, 0.0);
        assert!(!ship.is_boosting());
    }

    #[test]
    fn boost_cannot_be_reused_until_cooldown_ends() {
        let mut ship = Ship::new(Vec2::ZERO);
        let aim = vec2(0.0, -50_000.0);
        run(&mut ship, aim, 1.5);
        let cruise = ship.velocity.length();
        assert!((cruise - MAX_SPEED).abs() < 5.0);

        step(
            &mut ship,
            aim,
            ShipInput {
                drift: false,
                boost: true,
            },
        );
        assert!(ship.is_boosting());
        assert!(ship.boost_charge() > 0.9);

        while ship.is_boosting() {
            step(&mut ship, aim, ShipInput::default());
        }
        assert!(ship.boost_cooldown > 0.0);
        assert!(ship.boost_charge() < 0.05);
        assert!(ship.velocity.length() > MAX_SPEED + 200.0);

        let speed = ship.velocity.length();
        let cooldown = ship.boost_cooldown;
        step(
            &mut ship,
            aim,
            ShipInput {
                drift: false,
                boost: true,
            },
        );
        assert!(!ship.is_boosting());
        assert!(ship.boost_cooldown < cooldown);
        assert!(ship.velocity.length() < speed);

        for _ in 0..10_000 {
            if ship.boost_cooldown <= 0.0 {
                break;
            }
            step(&mut ship, aim, ShipInput::default());
        }
        assert_eq!(ship.boost_cooldown, 0.0);
        assert_eq!(ship.boost_charge(), 1.0);

        step(
            &mut ship,
            aim,
            ShipInput {
                drift: false,
                boost: true,
            },
        );
        assert!(ship.is_boosting());
    }
}
