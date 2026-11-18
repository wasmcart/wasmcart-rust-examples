//! A 5x7 bitmap font, baked into an atlas texture at start-up, and the HUD
//! quad list text and bars are drawn with.

/// Atlas: 16 x 8 cells of 8 x 8 pixels, ASCII 0..127. Cell 127 is solid,
/// for bars and panels.
pub const ATLAS_W: u32 = 128;
pub const ATLAS_H: u32 = 64;
pub const SOLID: u8 = 127;

/// Rows top to bottom, bit 4 is the leftmost pixel.
fn glyph(c: u8) -> [u8; 7] {
    match c {
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [31, 2, 4, 2, 1, 17, 14],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 30, 1, 1, 17, 14],
        b'6' => [6, 8, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 2, 12],
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [28, 18, 17, 17, 17, 18, 28],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 2, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 17, 25, 21, 19, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 17, 10, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b':' => [0, 12, 12, 0, 12, 12, 0],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b',' => [0, 0, 0, 0, 12, 4, 8],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b'/' => [1, 1, 2, 4, 8, 16, 16],
        b'\'' => [4, 4, 8, 0, 0, 0, 0],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b'x' => [0, 0, 17, 10, 4, 10, 17],
        b'%' => [24, 25, 2, 4, 8, 19, 3],
        b'>' => [8, 4, 2, 1, 2, 4, 8],
        b'<' => [2, 4, 8, 16, 8, 4, 2],
        _ => [0; 7],
    }
}

/// The atlas as RGBA8 (white glyphs, alpha coverage).
pub fn atlas() -> Vec<u8> {
    let mut px = vec![0u8; (ATLAS_W * ATLAS_H * 4) as usize];
    for c in 0u8..128 {
        let (cx, cy) = ((c % 16) as u32 * 8, (c / 16) as u32 * 8);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let on = if c == SOLID {
                    true
                } else {
                    y < 7 && x < 5 && glyph(c)[y as usize] & (16 >> x) != 0
                };
                if on {
                    let i = (((cy + y) * ATLAS_W + cx + x) * 4) as usize;
                    px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    px
}

/// One HUD quad: screen rect in pixels, atlas rect in texels, colour.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

#[derive(Default)]
pub struct Hud {
    pub quads: Vec<Quad>,
}

pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    [r, g, b, a]
}

impl Hud {
    pub fn clear(&mut self) {
        self.quads.clear();
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let (cx, cy) = ((SOLID % 16) as f32 * 8.0 + 2.0, (SOLID / 16) as f32 * 8.0 + 2.0);
        self.quads.push(Quad { rect: [x, y, w, h], uv: [cx, cy, 4.0, 4.0], color });
    }
    /// Width in pixels of `s` at `scale`.
    pub fn width(s: &str, scale: f32) -> f32 {
        let n = s.len() as f32;
        if n == 0.0 {
            0.0
        } else {
            (n * 6.0 - 1.0) * scale
        }
    }
    fn raw(&mut self, s: &str, x: f32, y: f32, scale: f32, color: [f32; 4]) {
        let mut cx = x;
        for &c in s.as_bytes() {
            let c = if c.is_ascii_lowercase() && c != b'x' { c.to_ascii_uppercase() } else { c };
            if c != b' ' {
                let (u, v) = ((c % 16) as f32 * 8.0, (c / 16) as f32 * 8.0);
                self.quads.push(Quad { rect: [cx, y, 5.0 * scale, 7.0 * scale], uv: [u, v, 5.0, 7.0], color });
            }
            cx += 6.0 * scale;
        }
    }
    /// Text with a dark drop shadow so it reads over any part of the scene.
    pub fn text(&mut self, s: &str, x: f32, y: f32, scale: f32, color: [f32; 4]) {
        let o = (scale * 0.5).max(1.0);
        self.raw(s, x + o, y + o, scale, [0.0, 0.0, 0.02, color[3] * 0.75]);
        self.raw(s, x, y, scale, color);
    }
    pub fn text_center(&mut self, s: &str, cx: f32, y: f32, scale: f32, color: [f32; 4]) {
        self.text(s, cx - Self::width(s, scale) / 2.0, y, scale, color);
    }
    pub fn text_right(&mut self, s: &str, rx: f32, y: f32, scale: f32, color: [f32; 4]) {
        self.text(s, rx - Self::width(s, scale), y, scale, color);
    }
}
