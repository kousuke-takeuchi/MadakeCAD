use serde::{Deserialize, Serialize};

/// 2D座標。単位はmm(用紙座標系、左上原点・下向きY)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    /// グリッドピッチに丸める。
    pub fn snapped(&self, pitch: f64) -> Self {
        Self::new(
            (self.x / pitch).round() * pitch,
            (self.y / pitch).round() * pitch,
        )
    }

    pub fn distance_to(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}
