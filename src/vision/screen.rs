//! Ultra-fast screen and sub-rectangle ROI capture in pure Rust using X11 protocol (x11rb).

use image::{ImageBuffer, Rgba, RgbaImage};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

pub struct ScreenCapturer {
    conn: Option<RustConnection>,
    root: u32,
}

impl ScreenCapturer {
    pub fn new() -> Self {
        if let Ok((conn, screen_num)) = RustConnection::connect(None) {
            let root = conn.setup().roots[screen_num].root;
            Self {
                conn: Some(conn),
                root,
            }
        } else {
            Self { conn: None, root: 0 }
        }
    }

    /// Captures strictly a sub-rectangle ROI directly from X11 server without transferring the rest of the screen.
    ///
    /// :param x: Absolute screen X
    /// :param y: Absolute screen Y
    /// :param width: Width of the ROI in pixels
    /// :param height: Height of the ROI in pixels
    pub fn capture_roi(&self, x: i32, y: i32, width: u32, height: u32) -> Option<RgbaImage> {
        if width == 0 || height == 0 {
            return None;
        }

        if let Some(ref conn) = self.conn {
            let px = x.max(0) as i16;
            let py = y.max(0) as i16;
            let pw = width.min(10000) as u16;
            let ph = height.min(10000) as u16;

            // XGetImage requests ONLY the bytes of the sub-rectangle (pw x ph) from the X11 server.
            // No full-screen or unneeded game areas are copied or allocated.
            if let Ok(cookie) = conn.get_image(ImageFormat::Z_PIXMAP, self.root, px, py, pw, ph, !0) {
                if let Ok(reply) = cookie.reply() {
                    let data = reply.data;
                    let mut img_buf: RgbaImage = ImageBuffer::new(width, height);

                    let mut src_idx = 0;
                    for cy in 0..height {
                        for cx in 0..width {
                            if src_idx + 3 < data.len() {
                                let b = data[src_idx];
                                let g = data[src_idx + 1];
                                let r = data[src_idx + 2];
                                img_buf.put_pixel(cx, cy, Rgba([r, g, b, 255]));
                                src_idx += 4;
                            }
                        }
                    }
                    return Some(img_buf);
                }
            }
        }

        None
    }

    /// Captures the full game window area if needed for multi-template scanning.
    pub fn capture_region(&self, x: i32, y: i32, width: u32, height: u32) -> Option<RgbaImage> {
        self.capture_roi(x, y, width, height)
    }
}
