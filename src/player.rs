use macroquad::prelude::*;

use crate::level::Level;

pub const PLAYER_RADIUS: f32 = 14.0;
const GRAVITY: f32 = 1600.0;
/// 速さの上限。巻き取りで勢いが際限なく増え、操作が追いつかなくなるのを防ぐ。
const MAX_SPEED: f32 = 900.0;
/// ワイヤーが届く最大の長さ。地形の刺し先どうしの間隔（350 以下）より十分長くする。
pub const MAX_WIRE_LENGTH: f32 = 600.0;
/// これより短いワイヤーにはしない。支点の真下で振り子が暴れないようにする。
const MIN_WIRE_LENGTH: f32 = 40.0;
/// ワイヤーを巻き取る・伸ばす速さ。刺さった後の長さは操作したときだけ変わる。
/// 振り子だけでは掴み直すたびに低くなっていくので、巻き取りで高さと勢いを取り戻せるようにする。
const REEL_SPEED: f32 = 250.0;
/// ワイヤーで敵を倒したとき、その場所へ引き寄せられる速さ。
const PULL_SPEED: f32 = 700.0;
/// 外したワイヤーが手元へ戻るまでの時間。戻るまでは次を撃てない。
const RETRACT_TIME: f32 = 0.15;
/// A / D で振り子を揺らすときに、振れる向きへ足す加速度。
const SWING_ACCEL: f32 = 900.0;

/// ワイヤーの操作。
#[derive(Clone, Copy, Default)]
pub struct WireInput {
    /// カーソルのワールド座標。
    pub aim: Vec2,
    /// 左クリックを押した瞬間。次に回る更新で 1 回だけ真になる。
    pub pressed: bool,
    /// ワイヤーの長さの操作。-1 で巻き取り、1 で伸ばす、0 でそのまま。
    pub reel: f32,
    /// 振り子を揺らす向き。-1 で左、1 で右、0 で揺らさない。
    pub swing: f32,
}

/// 刺さっているワイヤー。
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub point: Vec2,
    pub length: f32,
}

pub struct Player {
    pub position: Vec2,
    pub velocity: Vec2,
    pub anchor: Option<Anchor>,
    /// 手元へ戻っている途中のワイヤーの、戻り始めた位置と残り時間。
    retract: Option<(Vec2, f32)>,
}

impl Player {
    pub fn new(position: Vec2) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            anchor: None,
            retract: None,
        }
    }

    /// ワイヤーが手元にあり、撃てる状態か。
    pub fn can_shoot(&self) -> bool {
        self.anchor.is_none() && self.retract.is_none()
    }

    pub fn update(&mut self, input: WireInput, level: &Level, dt: f32) {
        // ワイヤーは「撃って刺す → クリックで手元へ戻す → また撃つ」を繰り返す。
        // 戻すとそのときの速さと向きのまま飛ぶ。
        if let Some((_, left)) = &mut self.retract {
            *left -= dt;
            if *left <= 0.0 {
                self.retract = None;
            }
        }
        if input.pressed {
            if let Some(anchor) = self.anchor.take() {
                self.retract = Some((anchor.point, RETRACT_TIME));
            } else if self.can_shoot() {
                self.shoot_wire(input.aim, level);
            }
        }

        self.velocity.y += GRAVITY * dt;
        self.velocity = self.velocity.clamp_length_max(MAX_SPEED);
        self.position += self.velocity * dt;

        // ワイヤーは縄として扱い、伸び切ったときだけ引き戻す（たるむことはできる）。
        if let Some(anchor) = &mut self.anchor {
            anchor.length = (anchor.length + input.reel * REEL_SPEED * dt)
                .clamp(MIN_WIRE_LENGTH, MAX_WIRE_LENGTH);
            let anchor = *anchor;
            let from_anchor = self.position - anchor.point;
            let distance = from_anchor.length();
            // 振れる向き（ワイヤーに垂直な向きのうち、横成分が入力と同じ側）へ加速して揺らす。
            if input.swing != 0.0 && distance > 0.0 {
                let mut tangent = from_anchor.perp() / distance;
                if tangent.x * input.swing < 0.0 {
                    tangent = -tangent;
                }
                self.velocity += tangent * SWING_ACCEL * dt;
            }
            if distance > anchor.length {
                let outward = from_anchor / distance;
                self.position = anchor.point + outward * anchor.length;
                let away = self.velocity.dot(outward);
                if away > 0.0 {
                    self.velocity -= outward * away;
                }
            }
        }

        // 地形に当たっても死なず、めり込む向きの速度だけ消して滑らせる。
        if let Some(contact) = level.push_out(self.position, PLAYER_RADIUS) {
            self.position = contact.position;
            let into = self.velocity.dot(contact.normal);
            if into < 0.0 {
                self.velocity -= contact.normal * into;
            }
        }
    }

    pub fn shoot_wire(&mut self, aim: Vec2, level: &Level) {
        let dir = (aim - self.position).normalize_or_zero();
        if dir == Vec2::ZERO {
            return;
        }
        match level.raycast(self.position, dir, MAX_WIRE_LENGTH) {
            Some(point) => {
                self.anchor = Some(Anchor {
                    point,
                    length: point.distance(self.position).max(MIN_WIRE_LENGTH),
                });
            }
            None => {
                // 届かなければ伸び切ったところから手元へ戻る。
                self.retract = Some((self.position + dir * MAX_WIRE_LENGTH, RETRACT_TIME));
            }
        }
    }

    /// ワイヤーで敵を倒したときに呼ぶ。ワイヤーは外れ、`point` へ向かう勢いが付く。
    pub fn pull_toward(&mut self, point: Vec2) {
        self.anchor = None;
        self.retract = None;
        let dir = (point - self.position).normalize_or_zero();
        self.velocity = dir * PULL_SPEED;
    }

    /// 今クリックしたらワイヤーが刺さる点。照準の表示に使う。
    pub fn wire_target(&self, aim: Vec2, level: &Level) -> Option<Vec2> {
        let dir = (aim - self.position).normalize_or_zero();
        level.raycast(self.position, dir, MAX_WIRE_LENGTH)
    }

    pub fn draw(&self) {
        let wire_color = Color::from_rgba(255, 200, 80, 255);
        if let Some(anchor) = self.anchor {
            draw_line(
                self.position.x,
                self.position.y,
                anchor.point.x,
                anchor.point.y,
                2.0,
                wire_color,
            );
            draw_circle(anchor.point.x, anchor.point.y, 4.0, wire_color);
        }
        if let Some((from, left)) = self.retract {
            // 戻り始めた位置から手元へ縮んでいくように描く。
            let t = (left / RETRACT_TIME).clamp(0.0, 1.0);
            let tip = self.position + (from - self.position) * t;
            draw_line(
                self.position.x,
                self.position.y,
                tip.x,
                tip.y,
                1.5,
                Color {
                    a: 0.7,
                    ..wire_color
                },
            );
        }
        draw_circle(
            self.position.x,
            self.position.y,
            PLAYER_RADIUS,
            Color::from_rgba(240, 240, 250, 255),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    /// 下端が y = 0 の天井。
    fn ceiling() -> Level {
        let mut level = Level::default();
        level.add(Rect::new(-10_000.0, -100.0, 20_000.0, 100.0));
        level
    }

    fn press(aim: Vec2) -> WireInput {
        WireInput {
            aim,
            pressed: true,
            reel: 0.0,
            swing: 0.0,
        }
    }

    fn idle() -> WireInput {
        WireInput::default()
    }

    #[test]
    fn falls_under_gravity() {
        let mut player = Player::new(Vec2::ZERO);
        for _ in 0..60 {
            player.update(WireInput::default(), &Level::default(), DT);
        }
        assert!((player.velocity.y - GRAVITY * 0.5).abs() < 1e-2);
        // さらに落ち続けても上限で止まる。
        for _ in 0..240 {
            player.update(WireInput::default(), &Level::default(), DT);
        }
        assert!((player.velocity.length() - MAX_SPEED).abs() < 1e-2);
        assert!(player.position.y > 0.0);
    }

    #[test]
    fn slides_along_ceiling() {
        let level = ceiling();
        let mut player = Player::new(vec2(0.0, PLAYER_RADIUS + 1.0));
        player.velocity = vec2(300.0, -500.0);
        player.update(WireInput::default(), &level, DT);
        assert_eq!(player.position.y, PLAYER_RADIUS);
        assert!(player.velocity.y >= 0.0);
        assert_eq!(player.velocity.x, 300.0);
    }

    #[test]
    fn wire_attaches_to_ceiling_in_range() {
        let level = ceiling();
        let mut player = Player::new(vec2(0.0, 400.0));
        player.update(press(vec2(0.0, -1.0)), &level, DT);
        let anchor = player.anchor.unwrap();
        assert_eq!(anchor.point, vec2(0.0, 0.0));
        assert_eq!(anchor.length, 400.0);
        // 操作しなければ長さは変わらない。
        for _ in 0..60 {
            player.update(idle(), &level, DT);
        }
        assert_eq!(player.anchor.unwrap().length, 400.0);
    }

    #[test]
    fn reel_changes_wire_length_within_limits() {
        let level = ceiling();
        let mut player = Player::new(vec2(0.0, 400.0));
        player.update(press(vec2(0.0, -1.0)), &level, DT);
        let reel_in = WireInput {
            reel: -1.0,
            ..idle()
        };
        player.update(reel_in, &level, DT);
        assert_eq!(player.anchor.unwrap().length, 400.0 - REEL_SPEED * DT);
        for _ in 0..600 {
            player.update(reel_in, &level, DT);
        }
        assert_eq!(player.anchor.unwrap().length, MIN_WIRE_LENGTH);
        let reel_out = WireInput {
            reel: 1.0,
            ..idle()
        };
        for _ in 0..600 {
            player.update(reel_out, &level, DT);
        }
        assert_eq!(player.anchor.unwrap().length, MAX_WIRE_LENGTH);
    }

    #[test]
    fn wire_misses_out_of_range() {
        let level = ceiling();
        let mut player = Player::new(vec2(0.0, MAX_WIRE_LENGTH + 50.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        assert!(player.anchor.is_none());
        // もう一度クリックするまで撃ち直さない。
        player.position.y = 300.0;
        player.update(idle(), &level, DT);
        assert!(player.anchor.is_none());
    }

    #[test]
    fn swinging_keeps_wire_length_and_moves_sideways() {
        let level = ceiling();
        // 支点の左下から振り出す。
        let mut player = Player::new(vec2(-300.0, 300.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        let anchor = player.anchor.unwrap();
        for _ in 0..60 {
            player.update(idle(), &level, DT);
            assert!(player.position.distance(anchor.point) <= anchor.length + 1e-3);
        }
        assert!(player.velocity.x > 0.0);
    }

    #[test]
    fn clicking_while_attached_pulls_wire_back_and_keeps_velocity() {
        let level = ceiling();
        let mut player = Player::new(vec2(-300.0, 300.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        for _ in 0..30 {
            player.update(idle(), &level, DT);
        }
        let before = player.velocity;
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        assert!(player.anchor.is_none());
        assert_eq!(player.velocity, before + vec2(0.0, GRAVITY * DT));
    }

    #[test]
    fn cannot_shoot_until_wire_is_back() {
        let level = ceiling();
        let mut player = Player::new(vec2(0.0, 300.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        assert!(!player.can_shoot());
        // 戻っている途中のクリックは無視される。
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        assert!(player.anchor.is_none());
        for _ in 0..(RETRACT_TIME / DT) as usize + 1 {
            player.update(idle(), &level, DT);
        }
        assert!(player.can_shoot());
        player.update(press(player.position + vec2(0.0, -1.0)), &level, DT);
        assert!(player.anchor.is_some());
    }

    #[test]
    fn swing_input_pushes_pendulum_sideways() {
        let level = ceiling();
        let swing = |dir: f32| WireInput {
            swing: dir,
            ..idle()
        };
        // 支点の真下で止まっている状態から、右へ揺らす。
        let mut player = Player::new(vec2(0.0, 300.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        for _ in 0..30 {
            player.update(swing(1.0), &level, DT);
        }
        assert!(player.velocity.x > 0.0);
        let anchor = player.anchor.unwrap();
        assert!(player.position.distance(anchor.point) <= anchor.length + 1e-3);

        let mut player = Player::new(vec2(0.0, 300.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        for _ in 0..30 {
            player.update(swing(-1.0), &level, DT);
        }
        assert!(player.velocity.x < 0.0);
    }
}
