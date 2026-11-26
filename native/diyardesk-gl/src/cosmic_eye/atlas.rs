//! Crisp monospace ASCII glyph strip atlas for the `cosmic-eye` theme.
//!
//! Rasterizes 64 ASCII characters once via `fontdue` (using the system
//! monospace font), measures each glyph's true ink coverage, and sorts them
//! strictly from darkest (`' '`) to brightest (`'@'`, `'M'`, `'#'`, ...) so
//! luminance-to-ASCII mapping produces smooth, organic shading identical to a
//! live terminal ASCII renderer.

use crate::text;

pub const GLYPH_COUNT: usize = 64;
pub const TILE_W: usize = 18; // 1 px zero padding left & right + 16 px glyph box
pub const TILE_H: usize = 28; // 1 px zero padding top & bottom + 26 px glyph box

/// 64 distinct printable ASCII characters spanning from empty space through
/// delicate punctuation, slashes/brackets (for organic skin/lid texture),
/// lowercase/uppercase letters, and dense block-like symbols.
const RAW_CHARSET: &str =
    " .'`\",:;!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

pub struct AsciiAtlas {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub chars_sorted: [char; GLYPH_COUNT],
}

struct RasterizedGlyph {
    ch: char,
    coverage_sum: u64,
    tile: [u8; TILE_W * TILE_H],
}

fn build_fallback_glyph(ch: char, density_rank: usize) -> RasterizedGlyph {
    // Simple deterministic fallback if no system font file could be opened.
    let mut tile = [0u8; TILE_W * TILE_H];
    let mut sum = 0u64;
    if ch != ' ' {
        let frac = (density_rank as f32 / (GLYPH_COUNT - 1) as f32).clamp(0.05, 1.0);
        let half_w = (2.0 + frac * 5.0) as i32;
        let half_h = (3.0 + frac * 8.0) as i32;
        let cx = (TILE_W / 2) as i32;
        let cy = (TILE_H / 2) as i32;
        for y in (cy - half_h).max(2)..(cy + half_h).min((TILE_H - 2) as i32) {
            for x in (cx - half_w).max(2)..(cx + half_w).min((TILE_W - 2) as i32) {
                let v = (frac * 255.0) as u8;
                tile[(y as usize) * TILE_W + (x as usize)] = v;
                sum += v as u64;
            }
        }
    }
    RasterizedGlyph {
        ch,
        coverage_sum: sum,
        tile,
    }
}

pub fn build_ascii_atlas() -> AsciiAtlas {
    let font_opt = text::load_mono_font().ok();
    let font_px: f32 = 22.0;

    let mut glyphs: Vec<RasterizedGlyph> = Vec::with_capacity(GLYPH_COUNT);
    for (idx, ch) in RAW_CHARSET.chars().take(GLYPH_COUNT).enumerate() {
        let g = match &font_opt {
            Some(font) => {
                let mut tile = [0u8; TILE_W * TILE_H];
                let mut sum = 0u64;
                if ch != ' ' {
                    let (metrics, bitmap) = font.rasterize(ch, font_px);
                    let gw = metrics.width as i32;
                    let gh = metrics.height as i32;
                    // Baseline positioned cleanly at ~75% down the tile
                    let baseline_y = 20i32;
                    let left_x = ((TILE_W as i32 - gw) / 2).max(1);
                    let top_y = baseline_y - gh - metrics.ymin;
                    for gy in 0..gh {
                        let dy = top_y + gy;
                        if dy < 1 || dy >= (TILE_H as i32 - 1) {
                            continue;
                        }
                        for gx in 0..gw {
                            let dx = left_x + gx;
                            if dx < 1 || dx >= (TILE_W as i32 - 1) {
                                continue;
                            }
                            let raw = bitmap[(gy as usize) * (gw as usize) + (gx as usize)];
                            // Slight crispness boost so thin strokes stay legible
                            let boosted = ((raw as f32 / 255.0).powf(0.84) * 255.0)
                                .round()
                                .clamp(0.0, 255.0) as u8;
                            tile[(dy as usize) * TILE_W + (dx as usize)] = boosted;
                            sum += boosted as u64;
                        }
                    }
                }
                RasterizedGlyph {
                    ch,
                    coverage_sum: sum,
                    tile,
                }
            }
            None => build_fallback_glyph(ch, idx),
        };
        glyphs.push(g);
    }

    // Keep index 0 strictly as space, sort remaining 63 by measured ink coverage
    if glyphs.len() > 1 {
        glyphs[1..].sort_by_key(|g| g.coverage_sum);
    }

    let atlas_w = GLYPH_COUNT * TILE_W;
    let atlas_h = TILE_H;
    let mut rgba = vec![0u8; atlas_w * atlas_h * 4];
    let mut chars_sorted = [' '; GLYPH_COUNT];

    for (gi, g) in glyphs.iter().enumerate() {
        chars_sorted[gi] = g.ch;
        let x_off = gi * TILE_W;
        for y in 0..TILE_H {
            for x in 0..TILE_W {
                let cov = g.tile[y * TILE_W + x];
                let dst = (y * atlas_w + x_off + x) * 4;
                rgba[dst] = cov;
                rgba[dst + 1] = cov;
                rgba[dst + 2] = cov;
                rgba[dst + 3] = cov;
            }
        }
    }

    AsciiAtlas {
        width: atlas_w as i32,
        height: atlas_h as i32,
        rgba,
        chars_sorted,
    }
}
