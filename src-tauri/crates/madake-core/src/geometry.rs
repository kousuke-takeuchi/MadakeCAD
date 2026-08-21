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

#[cfg(test)]
mod tests {
    use super::*;

    /// snapped() rounds a coordinate to the nearest grid pitch (default 2.5 mm), so everything lands on the pin grid.
    /// snapped()は座標を最も近いグリッドピッチ(既定2.5mm)へ丸め、すべてがピングリッドに乗る。
    #[test]
    fn snapped_rounds_to_grid_pitch() {
        assert_eq!(Point::new(3.7, 6.3).snapped(2.5), Point::new(2.5, 7.5));
        assert_eq!(Point::new(-1.2, 1.26).snapped(2.5), Point::new(0.0, 2.5));
    }

    /// translated() returns a shifted copy and never mutates the original point.
    /// translated()は平行移動したコピーを返し、元の点を変更しない。
    #[test]
    fn translated_shifts_without_mutation() {
        let p = Point::new(1.0, 2.0);
        let q = p.translated(4.0, -2.0);
        assert_eq!(q, Point::new(5.0, 0.0));
        assert_eq!(p, Point::new(1.0, 2.0));
    }

    /// distance_to() is the Euclidean distance (a 3-4-5 triangle measures 5).
    /// distance_to()はユークリッド距離である(3-4-5の直角三角形で5になる)。
    #[test]
    fn distance_is_euclidean() {
        let d = Point::new(0.0, 0.0).distance_to(&Point::new(3.0, 4.0));
        assert!((d - 5.0).abs() < 1e-12);
    }
}
