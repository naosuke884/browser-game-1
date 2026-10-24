use macroquad::prelude::*;

use crate::level::Level;

/// 天井の下端の高さ。
const CEILING_BOTTOM: f32 = 120.0;
/// 天井を一度に足す幅。
const SEGMENT_WIDTH: f32 = 400.0;

/// 地形を右へ作り足していく。今は平らな天井だけを作る。
pub struct Generator {
    /// まだ地形を作っていない範囲の左端。
    next_x: f32,
}

impl Generator {
    pub fn new(start_x: f32) -> Self {
        Self { next_x: start_x }
    }

    /// `until_x` まで地形を作り足す。
    pub fn fill(&mut self, level: &mut Level, until_x: f32) {
        while self.next_x < until_x {
            // 上端は画面より十分上に置き、天井の上を回り込めないようにする。
            level.add(Rect::new(
                self.next_x,
                -1000.0,
                SEGMENT_WIDTH,
                1000.0 + CEILING_BOTTOM,
            ));
            self.next_x += SEGMENT_WIDTH;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_up_to_requested_x_without_gaps() {
        let mut level = Level::default();
        let mut generator = Generator::new(0.0);
        generator.fill(&mut level, 1000.0);
        generator.fill(&mut level, 1500.0);
        let mut blocks = level.blocks.clone();
        blocks.sort_by(|a, b| a.x.total_cmp(&b.x));
        assert!(blocks.last().unwrap().right() >= 1500.0);
        for pair in blocks.windows(2) {
            assert_eq!(pair[0].right(), pair[1].x);
        }
    }
}
