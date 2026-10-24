use macroquad::prelude::*;

use crate::level::Level;

pub const PLAYER_RADIUS: f32 = 14.0;
const GRAVITY: f32 = 1600.0;

pub struct Player {
    pub position: Vec2,
    pub velocity: Vec2,
}

impl Player {
    pub fn new(position: Vec2) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
        }
    }

    pub fn update(&mut self, level: &Level, dt: f32) {
        self.velocity.y += GRAVITY * dt;
        self.position += self.velocity * dt;

        // 地形に当たっても死なず、めり込む向きの速度だけ消して滑らせる。
        if let Some(contact) = level.push_out(self.position, PLAYER_RADIUS) {
            self.position = contact.position;
            let into = self.velocity.dot(contact.normal);
            if into < 0.0 {
                self.velocity -= contact.normal * into;
            }
        }
    }

    pub fn draw(&self) {
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

    #[test]
    fn falls_under_gravity() {
        let mut player = Player::new(Vec2::ZERO);
        for _ in 0..120 {
            player.update(&Level::default(), 1.0 / 120.0);
        }
        assert!((player.velocity.y - GRAVITY).abs() < 1e-2);
        assert!(player.position.y > 0.0);
    }

    #[test]
    fn slides_along_ceiling() {
        let mut level = Level::default();
        level.add(Rect::new(-1000.0, -100.0, 2000.0, 100.0)); // 下端は y = 0
        let mut player = Player::new(vec2(0.0, PLAYER_RADIUS + 1.0));
        player.velocity = vec2(300.0, -500.0);
        player.update(&level, 1.0 / 120.0);
        assert_eq!(player.position.y, PLAYER_RADIUS);
        assert!(player.velocity.y >= 0.0);
        assert_eq!(player.velocity.x, 300.0);
    }
}
