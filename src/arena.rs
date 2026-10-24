use macroquad::prelude::*;

/// 壁で囲まれた長方形の場。原点を中心に置き、座標は y 下向き（画面と同じ向き）。
pub struct Arena {
    pub half_size: Vec2,
}

/// 円を場の内側に押し戻した結果。
pub struct WallHit {
    pub position: Vec2,
    /// 当たった壁の、場の内側を向く法線。2 面に同時に当たると角の向きになる。
    pub normal: Vec2,
}

impl Arena {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            half_size: vec2(width, height) / 2.0,
        }
    }

    /// 半径 `radius` の円が壁にめり込んでいれば内側へ押し戻す。めり込んでいなければ `None`。
    pub fn push_inside(&self, position: Vec2, radius: f32) -> Option<WallHit> {
        let limit = self.half_size - Vec2::splat(radius);
        let clamped = position.clamp(-limit, limit);
        if clamped == position {
            return None;
        }
        Some(WallHit {
            position: clamped,
            normal: (clamped - position).normalize_or_zero(),
        })
    }

    pub fn draw(&self) {
        let grid = 100.0;
        let grid_color = Color::from_rgba(38, 44, 58, 255);
        let mut x = -self.half_size.x + grid;
        while x < self.half_size.x {
            draw_line(x, -self.half_size.y, x, self.half_size.y, 1.0, grid_color);
            x += grid;
        }
        let mut y = -self.half_size.y + grid;
        while y < self.half_size.y {
            draw_line(-self.half_size.x, y, self.half_size.x, y, 1.0, grid_color);
            y += grid;
        }

        draw_rectangle_lines(
            -self.half_size.x,
            -self.half_size.y,
            self.half_size.x * 2.0,
            self.half_size.y * 2.0,
            6.0,
            Color::from_rgba(120, 180, 255, 255),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_is_untouched() {
        let arena = Arena::new(200.0, 100.0);
        assert!(arena.push_inside(vec2(10.0, 10.0), 5.0).is_none());
    }

    #[test]
    fn pushes_back_from_right_wall() {
        let arena = Arena::new(200.0, 100.0);
        let hit = arena.push_inside(vec2(120.0, 0.0), 5.0).unwrap();
        assert_eq!(hit.position, vec2(95.0, 0.0));
        assert_eq!(hit.normal, vec2(-1.0, 0.0));
    }
}
