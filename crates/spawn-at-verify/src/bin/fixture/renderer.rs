//! Softbuffer frame drawing and bitmap text rendering.

use super::font;
use std::num::NonZeroU32;

struct Surface<'a> {
    buffer: &'a mut [u32],
    stride: usize,
    height: usize,
}

impl<'a> Surface<'a> {
    /// Renders a single character glyph into the software surface buffer.
    fn draw_char(&mut self, c: char, px: usize, py: usize, scale: usize, color: u32) {
        let ascii = c as u32;
        if !(32..=126).contains(&ascii) {
            return;
        }
        let glyph = font::FONT_DATA[(ascii - 32) as usize];
        for (row, &row_bits) in glyph.iter().enumerate().take(font::FONT_HEIGHT) {
            for col in 0..font::FONT_WIDTH {
                if (row_bits >> (9 - col)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let x = px + col * scale + sx;
                            let y = py + row * scale + sy;
                            if x < self.stride && y < self.height {
                                self.buffer[y * self.stride + x] = color;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Draws a line of text starting at the given coordinates using the bitmap font.
    fn draw_text_line(&mut self, text: &str, px: usize, py: usize, scale: usize, color: u32) {
        let char_w = font::FONT_WIDTH * scale + (scale / 2).max(1);
        for (i, c) in text.chars().enumerate() {
            let x = px + i * char_w;
            if x + char_w > self.stride {
                break;
            }
            self.draw_char(c, x, py, scale, color);
        }
    }
}

/// Paints the window frame with title, timing status, and test assertions.
pub(crate) fn draw_frame(
    surface: &mut softbuffer::Surface<
        std::rc::Rc<winit::window::Window>,
        std::rc::Rc<winit::window::Window>,
    >,
    width: u32,
    height: u32,
    title: &str,
    paint_delay_ms: u64,
    expected_text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(nz_w), Some(nz_h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
        surface.resize(nz_w, nz_h)?;
        let mut buffer = surface.buffer_mut()?;

        // Background: #1e2238 (slate dark blue)
        let bg_color = 0xFF1E2238u32;
        // Border: #38bdf8 (cyan 400)
        let border_color = 0xFF38BDF8u32;
        let border_px = 3u32;

        let w_usize = width as usize;
        let h_usize = height as usize;

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if x < border_px
                    || x >= width.saturating_sub(border_px)
                    || y < border_px
                    || y >= height.saturating_sub(border_px)
                {
                    buffer[idx] = border_color;
                } else {
                    buffer[idx] = bg_color;
                }
            }
        }

        let mut surf = Surface {
            buffer: &mut buffer,
            stride: w_usize,
            height: h_usize,
        };

        // Title line
        let title_line = format!("Title: {}", title);
        surf.draw_text_line(&title_line, 20, 24, 1, 0xFF38BDF8);

        // Paint delay line
        let delay_line = format!("Paint Delay: {} ms", paint_delay_ms);
        surf.draw_text_line(&delay_line, 20, 52, 1, 0xFFFBBF24);

        // Expected header
        surf.draw_text_line("Expected Behavior:", 20, 84, 1, 0xFF94A3B8);

        // Expected description (word wrapped at 32 characters)
        let desc = if expected_text.is_empty() {
            "Window positioned by spawn-at"
        } else {
            expected_text
        };

        let mut cur_y = 108;
        let mut line_buf = String::new();
        for word in desc.split_whitespace() {
            if line_buf.len() + word.len() + 1 > 32 && !line_buf.is_empty() {
                surf.draw_text_line(&line_buf, 20, cur_y, 1, 0xFF4ADE80);
                cur_y += 24;
                line_buf.clear();
            }
            if !line_buf.is_empty() {
                line_buf.push(' ');
            }
            line_buf.push_str(word);
        }
        if !line_buf.is_empty() {
            surf.draw_text_line(&line_buf, 20, cur_y, 1, 0xFF4ADE80);
        }

        buffer.present()?;
    }
    Ok(())
}
