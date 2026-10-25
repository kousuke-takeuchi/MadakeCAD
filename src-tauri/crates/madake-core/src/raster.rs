//! シートのPNG化 (エージェントが描いた図面を目で確かめるため)。
//!
//! SVG出力 ([`crate::svg`]) をそのまま resvg でラスタ化する。書体はPDF出力と同じ割り当て
//! ([`crate::pdf::preferred_families`]) なので日本語も描ける。

use crate::tidy::Rect;

/// 出力画像の長辺の上限 (px)。超えるときは倍率を下げて収める。
pub const MAX_IMAGE_PX: u32 = 2400;

/// SVGのユーザー単位 (96dpi) の1mmあたりのピクセル数。
const SVG_PX_PER_MM: f32 = 96.0 / 25.4;

/// ラスタ化した画像と、実際に使った倍率。
#[derive(Debug, Clone)]
pub struct SheetImage {
    pub png: Vec<u8>,
    /// 1mmあたりのピクセル数 (上限で縮めた後)。
    pub px_per_mm: f32,
    pub width: u32,
    pub height: u32,
}

/// シートのSVGをPNGにする。`px_per_mm`は1mmあたりのピクセル数、`region`は切り抜く範囲 (mm)。
/// 長辺が [`MAX_IMAGE_PX`] を超えるときは倍率を下げる。
pub fn svg_to_png(svg: &str, px_per_mm: f32, region: Option<Rect>) -> Result<SheetImage, String> {
    let mut options = resvg::usvg::Options::default();
    *options.fontdb_mut() = crate::pdf::fontdb().clone();
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let paper = Rect::from_corners(
        crate::geometry::Point::new(0.0, 0.0),
        crate::geometry::Point::new(
            f64::from(tree.size().width() / SVG_PX_PER_MM),
            f64::from(tree.size().height() / SVG_PX_PER_MM),
        ),
    );
    // 範囲は用紙の内側へ切り詰める。面積が残らなければ断る
    let area = match region {
        Some(r) => Rect {
            min: crate::geometry::Point::new(r.min.x.max(paper.min.x), r.min.y.max(paper.min.y)),
            max: crate::geometry::Point::new(r.max.x.min(paper.max.x), r.max.y.min(paper.max.y)),
        },
        None => paper,
    };
    if area.max.x <= area.min.x || area.max.y <= area.min.y {
        return Err("範囲が空か、用紙の外です (mm で min < max、用紙の内側を指定する)".into());
    }
    let (w_mm, h_mm) = (
        (area.max.x - area.min.x) as f32,
        (area.max.y - area.min.y) as f32,
    );
    if px_per_mm <= 0.0 || !px_per_mm.is_finite() {
        return Err("倍率は正の数で指定する".into());
    }
    let longest = w_mm.max(h_mm) * px_per_mm;
    let px_per_mm = if longest > MAX_IMAGE_PX as f32 {
        MAX_IMAGE_PX as f32 / w_mm.max(h_mm)
    } else {
        px_per_mm
    };
    let width = ((w_mm * px_per_mm).round() as u32).max(1);
    let height = ((h_mm * px_per_mm).round() as u32).max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or("画像を確保できません")?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    let k = px_per_mm / SVG_PX_PER_MM;
    let transform = resvg::tiny_skia::Transform::from_row(
        k,
        0.0,
        0.0,
        k,
        -(area.min.x as f32) * px_per_mm,
        -(area.min.y as f32) * px_per_mm,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let png = pixmap.encode_png().map_err(|e| e.to_string())?;
    Ok(SheetImage {
        png,
        px_per_mm,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::{Orientation, PaperSize, Sheet};
    use crate::symbol::builtin_symbols;
    use crate::tidy::Rect;

    fn a3_svg() -> String {
        let sheet = Sheet::new("試験", PaperSize::A3, Orientation::Landscape);
        crate::svg::sheet_to_svg(&sheet, &builtin_symbols(), &[])
    }

    fn png_size(png: &[u8]) -> (u32, u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNGでない");
        let be = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
        (be(16), be(20))
    }

    /// A sheet becomes a PNG whose size is the paper size times the requested pixels per millimetre.
    /// シートは用紙寸法×指定した1mmあたりのピクセル数の大きさのPNGになる。
    #[test]
    fn a_sheet_becomes_a_png_of_the_paper_size() {
        let image = svg_to_png(&a3_svg(), 2.0, None).unwrap();
        assert_eq!(png_size(&image.png), (840, 594));
        assert_eq!(image.px_per_mm, 2.0);
    }

    /// A region in millimetres crops the image, so the agent can zoom into one part of the sheet.
    /// mm で指定した範囲で切り抜けるので、エージェントはシートの一部を拡大して見られる。
    #[test]
    fn a_region_crops_the_image() {
        let region = Rect::from_corners(Point::new(100.0, 50.0), Point::new(150.0, 80.0));
        let image = svg_to_png(&a3_svg(), 10.0, Some(region)).unwrap();
        assert_eq!(png_size(&image.png), (500, 300));
    }

    /// An image larger than the limit is scaled down to fit, and the scale actually used is reported.
    /// 上限より大きい画像は収まるように縮め、実際に使った倍率を返す。
    #[test]
    fn a_large_image_is_scaled_down_to_the_limit() {
        let image = svg_to_png(&a3_svg(), 20.0, None).unwrap();
        let (w, h) = png_size(&image.png);
        assert_eq!((w, h), (image.width, image.height));
        assert_eq!(w.max(h), MAX_IMAGE_PX);
        assert!(image.px_per_mm < 20.0);
    }

    /// A region outside the paper or with no area is refused.
    /// 用紙の外や面積の無い範囲は断る。
    #[test]
    fn an_empty_region_is_refused() {
        let empty = Rect::from_corners(Point::new(10.0, 10.0), Point::new(10.0, 50.0));
        assert!(svg_to_png(&a3_svg(), 4.0, Some(empty)).is_err());
        let outside = Rect::from_corners(Point::new(500.0, 400.0), Point::new(600.0, 500.0));
        assert!(svg_to_png(&a3_svg(), 4.0, Some(outside)).is_err());
    }
}
