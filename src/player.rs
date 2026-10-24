use macroquad::prelude::*;

use crate::level::Level;

pub const PLAYER_RADIUS: f32 = 14.0;
const GRAVITY: f32 = 1600.0;
/// 速さの上限。振り子の勢いは削らず、巻き取りや一押しを重ねて操作が追いつかなくなるのだけ防ぐ。
const MAX_SPEED: f32 = 1600.0;
/// ワイヤーが届く最大の長さ。地形の刺し先どうしの間隔（350 以下）より十分長くする。
pub const MAX_WIRE_LENGTH: f32 = 600.0;
/// これより短いワイヤーにはしない。支点の真下で振り子が暴れないようにする。
const MIN_WIRE_LENGTH: f32 = 40.0;
/// マウスのホイール 1 目盛りで変わるワイヤーの長さ。刺さった後の長さは操作したときだけ変わる。
const WHEEL_STEP: f32 = 40.0;
/// ワイヤーの長さが目標の長さへ近づく速さ。ホイールの目盛りごとにカクつかないようにする。
const REEL_SPEED: f32 = 800.0;
/// ワイヤーで敵を倒したとき、その場所へ引き寄せられる速さ。
const PULL_SPEED: f32 = 700.0;
/// ワイヤーの伸びにくさ（ばね定数、自機の重さを 1 とした値）。
/// 小さいほどよく伸びる。ぶら下がって止まっているとき、重力で GRAVITY / この値 だけ伸びる。
const WIRE_STIFFNESS: f32 = 150.0;
/// 伸び縮みの揺れを収める強さ。0 だとびよんびよんと揺れ続ける。
const WIRE_DAMPING: f32 = 4.0;
/// ワイヤーが伸びられる上限（長さに対する比）。速すぎるときに伸び切って飛んでいかないようにする。
const MAX_STRETCH: f32 = 1.4;
/// 画面に描くワイヤーの節の数。節どうしをつなぎ、しなって見えるようにする。
const ROPE_POINTS: usize = 20;
/// 外したワイヤーが手元へ戻るまでの時間。戻るまでは次を撃てない。
const RETRACT_TIME: f32 = 0.15;

/// ワイヤーの操作。
#[derive(Clone, Copy, Default)]
pub struct WireInput {
    /// カーソルのワールド座標。
    pub aim: Vec2,
    /// 左クリックを押した瞬間。次に回る更新で 1 回だけ真になる。
    pub pressed: bool,
    /// マウスのホイールを回した目盛りの数。正で伸ばし、負で巻き取る。
    pub wheel: f32,
}

/// 刺さっているワイヤー。
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub point: Vec2,
    pub length: f32,
    /// ホイールで指定した長さ。`length` はこの長さへ REEL_SPEED で近づく。
    pub target_length: f32,
}

pub struct Player {
    pub position: Vec2,
    pub velocity: Vec2,
    pub anchor: Option<Anchor>,
    /// 手元へ戻っている途中のワイヤーの、戻り始めた位置と残り時間。
    retract: Option<(Vec2, f32)>,
    /// 描画用のワイヤーの節。支点から自機まで並ぶ。見た目だけで、動きには影響しない。
    rope: Vec<Vec2>,
    rope_prev: Vec<Vec2>,
}

impl Player {
    pub fn new(position: Vec2) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            anchor: None,
            retract: None,
            rope: Vec::new(),
            rope_prev: Vec::new(),
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

        if let Some(anchor) = &mut self.anchor {
            anchor.target_length = (anchor.target_length + input.wheel * WHEEL_STEP)
                .clamp(MIN_WIRE_LENGTH, MAX_WIRE_LENGTH);
        }

        self.velocity.y += GRAVITY * dt;
        self.pull_by_wire(dt);
        self.velocity = self.velocity.clamp_length_max(MAX_SPEED);
        self.position += self.velocity * dt;
        self.limit_stretch();

        // 地形に当たっても死なず、めり込む向きの速度だけ消して滑らせる。
        if let Some(contact) = level.push_out(self.position, PLAYER_RADIUS) {
            self.position = contact.position;
            let into = self.velocity.dot(contact.normal);
            if into < 0.0 {
                self.velocity -= contact.normal * into;
            }
        }
        self.update_rope(dt);
    }

    /// ワイヤーをばねとして扱い、長さより離れているときだけ支点へ引っ張る（押し返しはしない）。
    /// 少し伸びて、引っ張られた後に反動で戻るので、しなるような動きになる。
    /// 支点へ向かう力だけなので、巻き取ると角運動量を保ったまま速く回る。
    fn pull_by_wire(&mut self, dt: f32) {
        let Some(anchor) = &mut self.anchor else {
            return;
        };
        // 長さを目標へ近づける。伸ばすときは縄が繰り出されるだけなので、たるんで自由に落ちる。
        let step = REEL_SPEED * dt;
        anchor.length += (anchor.target_length - anchor.length).clamp(-step, step);
        let offset = self.position - anchor.point;
        let distance = offset.length();
        if distance <= anchor.length {
            return;
        }
        let outward = offset / distance;
        let stretch = distance - anchor.length;
        let moving_away = self.velocity.dot(outward);
        let pull = (WIRE_STIFFNESS * stretch + WIRE_DAMPING * moving_away).max(0.0);
        self.velocity -= outward * pull * dt;
    }

    /// 伸び過ぎたワイヤーを上限で止める。
    fn limit_stretch(&mut self) {
        let Some(anchor) = self.anchor else {
            return;
        };
        let offset = self.position - anchor.point;
        let distance = offset.length();
        let limit = anchor.length * MAX_STRETCH;
        if distance <= limit {
            return;
        }
        let outward = offset / distance;
        self.position = anchor.point + outward * limit;
        let away = self.velocity.dot(outward);
        if away > 0.0 {
            self.velocity -= outward * away;
        }
    }

    /// 描画用のワイヤーの節を 1 刻み動かす。節は重力で垂れ、動きに遅れて付いてくるので、
    /// たるむと垂れ下がり、振ると後ろへしなる。張っているときはまっすぐに伸びる。
    fn update_rope(&mut self, dt: f32) {
        let Some(anchor) = self.anchor else {
            self.rope.clear();
            return;
        };
        if self.rope.len() != ROPE_POINTS {
            self.rope = (0..ROPE_POINTS)
                .map(|i| {
                    anchor
                        .point
                        .lerp(self.position, i as f32 / (ROPE_POINTS - 1) as f32)
                })
                .collect();
            self.rope_prev = self.rope.clone();
        }
        let gravity = vec2(0.0, GRAVITY) * dt * dt;
        for i in 1..ROPE_POINTS - 1 {
            let current = self.rope[i];
            let moved = (current - self.rope_prev[i]) * 0.98;
            self.rope_prev[i] = current;
            self.rope[i] = current + moved + gravity;
        }
        let segment = anchor.length / (ROPE_POINTS - 1) as f32;
        let last = ROPE_POINTS - 1;
        for _ in 0..12 {
            self.rope[0] = anchor.point;
            self.rope[last] = self.position;
            for i in 0..last {
                let delta = self.rope[i + 1] - self.rope[i];
                let length = delta.length();
                if length <= segment || length == 0.0 {
                    continue;
                }
                // 縄なので縮むことはできても、節の間隔より伸びることはできない。
                let correction = delta * ((length - segment) / length);
                let (a_pinned, b_pinned) = (i == 0, i + 1 == last);
                match (a_pinned, b_pinned) {
                    (true, true) => {}
                    (true, false) => self.rope[i + 1] -= correction,
                    (false, true) => self.rope[i] += correction,
                    (false, false) => {
                        self.rope[i] += correction * 0.5;
                        self.rope[i + 1] -= correction * 0.5;
                    }
                }
            }
        }
        self.rope[0] = anchor.point;
        self.rope[last] = self.position;
    }

    pub fn shoot_wire(&mut self, aim: Vec2, level: &Level) {
        let dir = (aim - self.position).normalize_or_zero();
        if dir == Vec2::ZERO {
            return;
        }
        match level.raycast(self.position, dir, MAX_WIRE_LENGTH) {
            Some(point) => {
                let length = point.distance(self.position).max(MIN_WIRE_LENGTH);
                self.anchor = Some(Anchor {
                    point,
                    length,
                    target_length: length,
                });
                self.rope.clear();
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
            if self.rope.len() == ROPE_POINTS {
                for pair in self.rope.windows(2) {
                    draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, 2.0, wire_color);
                }
            } else {
                draw_line(
                    self.position.x,
                    self.position.y,
                    anchor.point.x,
                    anchor.point.y,
                    2.0,
                    wire_color,
                );
            }
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
            ..Default::default()
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
    fn wheel_changes_wire_length_within_limits() {
        let level = ceiling();
        let mut player = Player::new(vec2(0.0, 400.0));
        player.update(press(vec2(0.0, -1.0)), &level, DT);
        let wheel = |steps: f32| WireInput {
            wheel: steps,
            ..idle()
        };
        // 1 目盛り巻き取ると、少しずつ短くなって 1 目盛り分で止まる。
        player.update(wheel(-1.0), &level, DT);
        assert_eq!(player.anchor.unwrap().length, 400.0 - REEL_SPEED * DT);
        for _ in 0..60 {
            player.update(idle(), &level, DT);
        }
        assert_eq!(player.anchor.unwrap().length, 400.0 - WHEEL_STEP);
        // 回しすぎても範囲に収まる。
        player.update(wheel(-100.0), &level, DT);
        for _ in 0..120 {
            player.update(idle(), &level, DT);
        }
        assert_eq!(player.anchor.unwrap().length, MIN_WIRE_LENGTH);
        player.update(wheel(100.0), &level, DT);
        assert_eq!(player.anchor.unwrap().target_length, MAX_WIRE_LENGTH);
    }

    /// 横向きに張った振り子を放すと、反対側でほぼ同じ高さまで上がる（勢いが目減りしない）。
    #[test]
    fn swinging_keeps_its_energy() {
        // 地形のない所で、支点の真横に張った状態から放す。
        let level = Level::default();
        let mut player = Player::new(vec2(-300.0, 0.0));
        player.anchor = Some(Anchor {
            point: Vec2::ZERO,
            length: 300.0,
            target_length: 300.0,
        });
        let mut highest_right = f32::MAX;
        for _ in 0..240 {
            player.update(idle(), &level, DT);
            if player.position.x > 0.0 {
                highest_right = highest_right.min(player.position.y);
            }
        }
        assert!(
            highest_right < 300.0 * 0.05,
            "y = {highest_right} までしか戻らなかった"
        );
    }

    #[test]
    fn reeling_in_while_swinging_speeds_up() {
        let level = ceiling();
        let mut player = Player::new(vec2(-300.0, 300.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        for _ in 0..20 {
            player.update(idle(), &level, DT);
        }
        let mut reeled = Player {
            position: player.position,
            velocity: player.velocity,
            anchor: player.anchor,
            retract: None,
            rope: Vec::new(),
            rope_prev: Vec::new(),
        };
        let wheel_in = WireInput {
            wheel: -3.0,
            ..idle()
        };
        reeled.update(wheel_in, &level, DT);
        player.update(idle(), &level, DT);
        for _ in 0..10 {
            reeled.update(idle(), &level, DT);
            player.update(idle(), &level, DT);
        }
        assert!(reeled.velocity.length() > player.velocity.length());
    }

    #[test]
    fn wire_stretches_and_bounces_back() {
        // 地形のない所で、支点の真下、ワイヤーの長さちょうどの位置から放す。
        let level = Level::default();
        let mut player = Player::new(vec2(0.0, 300.0));
        player.anchor = Some(Anchor {
            point: Vec2::ZERO,
            length: 300.0,
            target_length: 300.0,
        });
        let mut lowest = 0.0_f32;
        let mut bounced = false;
        for _ in 0..120 {
            player.update(idle(), &level, DT);
            lowest = lowest.max(player.position.y);
            bounced |= player.velocity.y < 0.0;
        }
        // 重力で伸び、反動で上へ戻る。
        assert!(lowest > 300.0 + GRAVITY / WIRE_STIFFNESS);
        assert!(bounced);
        // 揺れは収まり、重力とつり合う長さで止まる。
        for _ in 0..1200 {
            player.update(idle(), &level, DT);
        }
        let rest = 300.0 + GRAVITY / WIRE_STIFFNESS;
        assert!(
            (player.position.y - rest).abs() < 1.0,
            "{}",
            player.position.y
        );
    }

    #[test]
    fn rope_sags_when_slack() {
        // ワイヤーより近くにいてたるんでいると、描画用の節は 2 点を結ぶ線より下に垂れる。
        let level = Level::default();
        let mut player = Player::new(vec2(100.0, 0.0));
        player.anchor = Some(Anchor {
            point: Vec2::ZERO,
            length: 300.0,
            target_length: 300.0,
        });
        for _ in 0..120 {
            player.position = vec2(100.0, 0.0);
            player.velocity = Vec2::ZERO;
            player.update(idle(), &level, DT);
        }
        let middle = player.rope[ROPE_POINTS / 2];
        assert!(middle.y > 50.0, "{middle}");
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
    fn swinging_stays_within_stretch_limit_and_moves_sideways() {
        let level = ceiling();
        // 支点の左下から振り出す。
        let mut player = Player::new(vec2(-300.0, 300.0));
        player.update(press(vec2(0.0, 0.0)), &level, DT);
        let anchor = player.anchor.unwrap();
        for _ in 0..60 {
            player.update(idle(), &level, DT);
            // ばねなので少し伸びるが、上限より伸びることはない。
            assert!(player.position.distance(anchor.point) <= anchor.length * MAX_STRETCH + 1e-3);
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
}
