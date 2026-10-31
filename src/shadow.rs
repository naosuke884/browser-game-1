//! 同じコースを同時に走っている他の人の影。
//! 自機の状態を決まった形のバイト列にして送り、届いた他の人の状態を少し遅らせて補間して描く。
//! 影はゲームの判定に関わらない。届いたバイト列は信用せず、読めないものは捨てる（決して panic しない）。

use std::collections::HashMap;

use macroquad::prelude::*;

use crate::game::FALL_LIMIT_Y;
use crate::player::{Player, MAX_WIRE_LENGTH, PLAYER_RADIUS};

/// 送る中身の形式の版。形式を変えたら上げ、古い形式の人の状態は読まずに捨てる。
const FORMAT_VERSION: u8 = 1;
/// 1 人ぶんの状態のバイト数。
/// `[版: u8][フラグ: u8][送った時刻 ms: u32][位置 x, y: f32][支点 x, y: f32]`（little endian）。
const STATE_LEN: usize = 22;
/// フラグ: ワイヤーが刺さっている。
const FLAG_ANCHORED: u8 = 1;
/// 送る間隔（秒）。1 秒に 12 回。部屋は 1 人 1 秒に 30 回までしか受け付けない。
pub const SEND_INTERVAL: f32 = 1.0 / 12.0;
/// 届いた状態をこれだけ遅らせて描く（秒）。部屋は 100ms ごとにまとめて配るので、それより長くする。
const RENDER_DELAY: f64 = 0.2;
/// これだけ何も届かない人は、いなくなったものとして消す（秒）。
const PEER_TIMEOUT: f64 = 2.0;
/// 1 人ぶんに覚えておく状態の数。
const MAX_SAMPLES: usize = 16;
/// 覚えておく人数の上限。部屋は 20 人までなので、それより多い ID は捨てる。
const MAX_PEERS: usize = 32;
/// 受け付ける位置の範囲。コースの外のありえない値は捨てる。
const X_RANGE: (f32, f32) = (-1.0e5, 1.0e7);
const Y_RANGE: (f32, f32) = (-2000.0, FALL_LIMIT_Y + 1000.0);
/// 支点は自機からこれより離れない。ワイヤーは最大の長さの 1.4 倍まで伸びる。
const MAX_ANCHOR_DISTANCE: f32 = MAX_WIRE_LENGTH * 1.5;

/// 1 人ぶんの状態。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    /// 送った人の時計での時刻（ミリ秒）。
    pub time_ms: u32,
    pub position: Vec2,
    pub anchor: Option<Vec2>,
}

impl State {
    pub fn of(player: &Player, time_ms: u32) -> Self {
        Self {
            time_ms,
            position: player.position,
            anchor: player.anchor.map(|a| a.point),
        }
    }

    pub fn encode(&self) -> [u8; STATE_LEN] {
        let mut out = [0; STATE_LEN];
        out[0] = FORMAT_VERSION;
        out[1] = if self.anchor.is_some() {
            FLAG_ANCHORED
        } else {
            0
        };
        out[2..6].copy_from_slice(&self.time_ms.to_le_bytes());
        let anchor = self.anchor.unwrap_or(self.position);
        for (i, value) in [self.position.x, self.position.y, anchor.x, anchor.y]
            .into_iter()
            .enumerate()
        {
            out[6 + i * 4..10 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// 長さ・版・値の範囲が合わなければ `None`。
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let bytes: &[u8; STATE_LEN] = bytes.try_into().ok()?;
        if bytes[0] != FORMAT_VERSION {
            return None;
        }
        let flags = bytes[1];
        let time_ms = u32::from_le_bytes(bytes[2..6].try_into().ok()?);
        let f = |i: usize| -> Option<f32> {
            let value = f32::from_le_bytes(bytes[6 + i * 4..10 + i * 4].try_into().ok()?);
            value.is_finite().then_some(value)
        };
        let position = vec2(f(0)?, f(1)?);
        let in_range = |v: Vec2| {
            (X_RANGE.0..=X_RANGE.1).contains(&v.x) && (Y_RANGE.0..=Y_RANGE.1).contains(&v.y)
        };
        if !in_range(position) {
            return None;
        }
        let anchor = if flags & FLAG_ANCHORED != 0 {
            let anchor = vec2(f(2)?, f(3)?);
            if !in_range(anchor) || anchor.distance(position) > MAX_ANCHOR_DISTANCE {
                return None;
            }
            Some(anchor)
        } else {
            None
        };
        Some(Self {
            time_ms,
            position,
            anchor,
        })
    }
}

/// 部屋から届いたもの。
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// 他の人から届いた状態。読めなかった分は入らない。
    States(Vec<(u16, State)>),
    /// その人が部屋を出た。
    Left(u16),
}

/// JS から受け取る 1 件のメッセージを読む。先頭の 1 バイトで種類を分ける。
/// - 0: 部屋から届いたバイナリ。`[送信者 ID: u16][長さ: u16][バイト列]` の繰り返し（big endian）
/// - 1: 部屋を出た人。`[ID: u16]`（big endian）
pub fn read_incoming(message: &[u8]) -> Option<Incoming> {
    let (&kind, rest) = message.split_first()?;
    match kind {
        0 => {
            let mut states = Vec::new();
            let mut rest = rest;
            while let Some((header, body)) = rest.split_first_chunk::<4>() {
                let from = u16::from_be_bytes([header[0], header[1]]);
                let len = u16::from_be_bytes([header[2], header[3]]) as usize;
                let Some(data) = body.get(..len) else {
                    // 途中で切れている。ここまでに読めた分だけ使う。
                    break;
                };
                if let Some(state) = State::decode(data) {
                    states.push((from, state));
                }
                rest = &body[len..];
            }
            Some(Incoming::States(states))
        }
        1 => {
            let id: [u8; 2] = rest.try_into().ok()?;
            Some(Incoming::Left(u16::from_be_bytes(id)))
        }
        _ => None,
    }
}

/// 1 人ぶんの、届いた状態の並び。
struct Peer {
    /// 送った人の時刻の順に並ぶ。
    samples: Vec<State>,
    /// 送った人の時計から自分の時計へ直す差（秒）。いちばん早く届いた状態で決める。
    offset: f64,
    last_seen: f64,
}

/// 他の人の影。
#[derive(Default)]
pub struct Shadows {
    peers: HashMap<u16, Peer>,
}

impl Shadows {
    /// `now` は自分の時計の今の時刻（秒）。
    pub fn receive(&mut self, incoming: Incoming, now: f64) {
        match incoming {
            Incoming::States(states) => {
                for (id, state) in states {
                    self.add(id, state, now);
                }
            }
            Incoming::Left(id) => {
                self.peers.remove(&id);
            }
        }
    }

    fn add(&mut self, id: u16, state: State, now: f64) {
        if !self.peers.contains_key(&id) && self.peers.len() >= MAX_PEERS {
            return;
        }
        let offset = now - state.time_ms as f64 / 1000.0;
        let peer = self.peers.entry(id).or_insert(Peer {
            samples: Vec::new(),
            offset,
            last_seen: now,
        });
        // 遅れて届いた状態ほど差が大きくなる。いちばん小さい差を使い、遅れの揺れを影の動きに出さない。
        // 相手がやり直して時刻が戻ったときは、覚えている状態を捨てて新しい差で始める。
        if peer
            .samples
            .last()
            .is_some_and(|s| state.time_ms < s.time_ms)
        {
            peer.samples.clear();
            peer.offset = offset;
        }
        peer.offset = peer.offset.min(offset);
        peer.last_seen = now;
        peer.samples.push(state);
        if peer.samples.len() > MAX_SAMPLES {
            peer.samples.remove(0);
        }
    }

    /// しばらく何も届かない人を消す。
    pub fn forget_stale(&mut self, now: f64) {
        self.peers.retain(|_, p| now - p.last_seen < PEER_TIMEOUT);
    }

    /// 今描く、各人の位置と支点。
    pub fn positions(&self, now: f64) -> Vec<(Vec2, Option<Vec2>)> {
        self.peers
            .values()
            .filter_map(|peer| {
                interpolate(&peer.samples, (now - peer.offset - RENDER_DELAY) * 1000.0)
            })
            .collect()
    }

    pub fn draw(&self, now: f64) {
        let color = Color::from_rgba(150, 170, 220, 90);
        for (position, anchor) in self.positions(now) {
            if let Some(anchor) = anchor {
                draw_line(position.x, position.y, anchor.x, anchor.y, 1.5, color);
            }
            draw_circle(position.x, position.y, PLAYER_RADIUS, color);
        }
    }
}

/// 送った人の時刻 `time_ms` での状態。前後の状態の間を補間する。
/// 最初の状態より前なら最初の、最後より後なら最後の状態をそのまま使う。
fn interpolate(samples: &[State], time_ms: f64) -> Option<(Vec2, Option<Vec2>)> {
    let first = samples.first()?;
    let last = samples.last()?;
    if time_ms <= first.time_ms as f64 {
        return Some((first.position, first.anchor));
    }
    if time_ms >= last.time_ms as f64 {
        return Some((last.position, last.anchor));
    }
    let pair = samples
        .windows(2)
        .find(|w| (w[0].time_ms as f64..=w[1].time_ms as f64).contains(&time_ms))?;
    let (a, b) = (pair[0], pair[1]);
    let span = (b.time_ms - a.time_ms) as f64;
    let t = if span > 0.0 {
        ((time_ms - a.time_ms as f64) / span) as f32
    } else {
        1.0
    };
    // 支点は刺し直すと飛ぶので補間しない。後の状態に合わせ、線は補間した位置から引く。
    Some((a.position.lerp(b.position, t), b.anchor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(time_ms: u32, x: f32) -> State {
        State {
            time_ms,
            position: vec2(x, 300.0),
            anchor: Some(vec2(x, 120.0)),
        }
    }

    fn frames(list: &[(u16, &[u8])]) -> Vec<u8> {
        let mut out = vec![0];
        for (from, data) in list {
            out.extend_from_slice(&from.to_be_bytes());
            out.extend_from_slice(&(data.len() as u16).to_be_bytes());
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn state_round_trips() {
        let s = state(1234, 5000.0);
        assert_eq!(State::decode(&s.encode()), Some(s));
        let loose = State { anchor: None, ..s };
        assert_eq!(State::decode(&loose.encode()), Some(loose));
    }

    #[test]
    fn rejects_bad_states() {
        let good = state(1, 100.0).encode();
        assert!(State::decode(&good[..STATE_LEN - 1]).is_none());
        let mut longer = good.to_vec();
        longer.push(0);
        assert!(State::decode(&longer).is_none());

        let mut old = good;
        old[0] = FORMAT_VERSION + 1;
        assert!(State::decode(&old).is_none());

        let mut nan = good;
        nan[6..10].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(State::decode(&nan).is_none());

        let far = State {
            position: vec2(100.0, 1.0e6),
            ..state(1, 100.0)
        };
        assert!(State::decode(&far.encode()).is_none());

        let long_wire = State {
            anchor: Some(vec2(100.0 + MAX_ANCHOR_DISTANCE * 2.0, 300.0)),
            ..state(1, 100.0)
        };
        assert!(State::decode(&long_wire.encode()).is_none());
    }

    #[test]
    fn reads_frames_and_skips_unreadable_ones() {
        let a = state(10, 100.0).encode();
        let b = state(20, 200.0).encode();
        let message = frames(&[(3, &a), (4, &[1, 2, 3]), (5, &b)]);
        assert_eq!(
            read_incoming(&message),
            Some(Incoming::States(vec![
                (3, state(10, 100.0)),
                (5, state(20, 200.0))
            ]))
        );
        // 途中で切れていたら、そこまでの分だけ読む。
        let cut = &message[..message.len() - 5];
        assert_eq!(
            read_incoming(cut),
            Some(Incoming::States(vec![(3, state(10, 100.0))]))
        );
        assert_eq!(read_incoming(&[1, 0, 7]), Some(Incoming::Left(7)));
        assert_eq!(read_incoming(&[1, 0]), None);
        assert_eq!(read_incoming(&[]), None);
        assert_eq!(read_incoming(&[9, 1, 2]), None);
    }

    /// 本番の部屋が実際に配ったバイト列（送信者 1 が State を 1 つ送った）を読める。
    #[test]
    fn reads_bytes_relayed_by_the_room() {
        let relayed = "000100160101d20400000000fa43000096430000fa430000f042";
        let mut message = vec![0];
        message.extend(
            (0..relayed.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&relayed[i..i + 2], 16).unwrap()),
        );
        let expected = State {
            time_ms: 1234,
            position: vec2(500.0, 300.0),
            anchor: Some(vec2(500.0, 120.0)),
        };
        assert_eq!(
            read_incoming(&message),
            Some(Incoming::States(vec![(1, expected)]))
        );
    }

    /// どんなバイト列を渡しても panic しない。
    #[test]
    fn never_panics_on_garbage() {
        let rng = macroquad::rand::RandGenerator::new();
        rng.srand(1);
        let mut shadows = Shadows::default();
        for i in 0..5000 {
            let len = rng.gen_range(0, 200);
            let mut bytes: Vec<u8> = (0..len).map(|_| rng.gen_range(0, 256) as u8).collect();
            // 読める形に近いものも混ぜる。
            if i % 3 == 0 && !bytes.is_empty() {
                bytes[0] = 0;
            }
            let _ = State::decode(&bytes);
            if let Some(incoming) = read_incoming(&bytes) {
                shadows.receive(incoming, i as f64 * 0.01);
            }
            let _ = shadows.positions(i as f64 * 0.01);
        }
    }

    #[test]
    fn interpolates_between_states_with_a_delay() {
        let mut shadows = Shadows::default();
        // 送った時刻 0ms と 100ms の状態が、同じ時刻 10.0 秒にまとめて届く。
        let incoming = Incoming::States(vec![(1, state(0, 0.0)), (1, state(100, 100.0))]);
        shadows.receive(incoming, 10.0);
        // 差はいちばん早く届いた（送った時刻が新しい）状態で決まる: 10.0 - 0.1 = 9.9。
        // 10.0 + 0.05 秒の時点では、送った時刻 (10.05 - 9.9 - 0.2) = -50ms → 最初の状態。
        assert_eq!(shadows.positions(10.05)[0].0.x, 0.0);
        // 10.15 秒では 50ms → 中間。
        let x = shadows.positions(10.15)[0].0.x;
        assert!((x - 50.0).abs() < 1e-3, "{x}");
        // 先へ進んでも最後の状態で止まる。
        assert_eq!(shadows.positions(20.0)[0].0.x, 100.0);
    }

    #[test]
    fn peers_leave_and_time_out() {
        let mut shadows = Shadows::default();
        shadows.receive(
            Incoming::States(vec![(1, state(0, 0.0)), (2, state(0, 0.0))]),
            0.0,
        );
        shadows.receive(Incoming::Left(1), 0.1);
        assert_eq!(shadows.positions(0.1).len(), 1);
        shadows.forget_stale(PEER_TIMEOUT + 0.1);
        assert!(shadows.positions(PEER_TIMEOUT + 0.1).is_empty());
    }

    #[test]
    fn restarting_peer_starts_over() {
        let mut shadows = Shadows::default();
        shadows.receive(Incoming::States(vec![(1, state(50_000, 9000.0))]), 100.0);
        // やり直して時刻が 0 に戻った。
        shadows.receive(Incoming::States(vec![(1, state(0, 300.0))]), 101.0);
        assert_eq!(shadows.positions(102.0)[0].0.x, 300.0);
    }
}
