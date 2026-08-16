//! Window locator, geometry inspector, and active focus tracking using 100% pure Rust X11 connection (x11rb).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_found: bool,
    pub is_focused: bool,
    pub source: String,
    pub window_id: Option<u32>,
}

impl WindowInfo {
    pub fn center(&self) -> (i32, i32) {
        (self.x + (self.width / 2) as i32, self.y + (self.height / 2) as i32)
    }

    pub fn aspect_ratio(&self) -> String {
        if self.width == 0 || self.height == 0 {
            return "unknown".to_string();
        }
        let gcd = gcd(self.width, self.height);
        format!("{}:{}", self.width / gcd, self.height / gcd)
    }

    pub fn format_summary(&self) -> String {
        if !self.is_found {
            format!(
                "Status       : NOT FOUND (Waiting for game window...)\n\
                 Position     : ({}, {})\n\
                 Dimensions   : {} x {} px\n\
                 Focused      : NO\n\
                 Center Point : ({}, {})\n\
                 Aspect Ratio : {}",
                self.x, self.y, self.width, self.height, self.center().0, self.center().1, self.aspect_ratio()
            )
        } else {
            format!(
                "Status       : FOUND ({}, Window ID: 0x{:x})\n\
                 Position     : X={} px, Y={} px\n\
                 Dimensions   : {} x {} px (Width x Height)\n\
                 Focused      : {}\n\
                 Center Point : ({}, {})\n\
                 Aspect Ratio : {}",
                self.source,
                self.window_id.unwrap_or(0),
                self.x,
                self.y,
                self.width,
                self.height,
                if self.is_focused { "YES (Active)" } else { "NO (Background)" },
                self.center().0,
                self.center().1,
                self.aspect_ratio()
            )
        }
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

pub struct WindowManager {
    target_title: String,
}

impl WindowManager {
    pub fn new(target_title: &str) -> Self {
        Self {
            target_title: target_title.to_string(),
        }
    }

    pub fn get_window_info(&self) -> WindowInfo {
        if let Some(info) = self.find_x11_window() {
            return info;
        }

        // Fallback default resolution
        WindowInfo {
            title: self.target_title.clone(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            is_found: false,
            is_focused: false,
            source: "monitor_fallback".to_string(),
            window_id: None,
        }
    }

    pub fn get_active_window_id(&self, conn: &RustConnection, root: u32) -> Option<u32> {
        let net_active = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW").ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom);

        if let Some(atom) = net_active {
            if let Ok(cookie) = conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, 1) {
                if let Ok(reply) = cookie.reply() {
                    if let Some(mut iter) = reply.value32() {
                        return iter.next();
                    }
                }
            }
        }

        // Fallback to get_input_focus
        if let Ok(cookie) = conn.get_input_focus() {
            if let Ok(reply) = cookie.reply() {
                return Some(reply.focus);
            }
        }

        None
    }

    pub fn list_all_windows(&self) -> Vec<(u32, String, i32, i32, u32, u32, bool)> {
        let mut results = Vec::new();
        let (conn, screen_num) = match RustConnection::connect(None) {
            Ok(c) => c,
            Err(_) => return results,
        };

        let root = conn.setup().roots[screen_num].root;
        let active_win_id = self.get_active_window_id(&conn, root);

        let net_client_list = conn.intern_atom(false, b"_NET_CLIENT_LIST").ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom);

        let mut windows = Vec::new();
        if let Some(atom) = net_client_list {
            if let Ok(cookie) = conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, 1024) {
                if let Ok(reply) = cookie.reply() {
                    windows = reply.value32().map(|iter| iter.collect()).unwrap_or_default();
                }
            }
        }

        if windows.is_empty() {
            if let Ok(cookie) = conn.query_tree(root) {
                if let Ok(reply) = cookie.reply() {
                    windows = reply.children;
                }
            }
        }

        let net_wm_name = conn.intern_atom(false, b"_NET_WM_NAME").ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom);
        let utf8_string = conn.intern_atom(false, b"UTF8_STRING").ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom);

        for win in windows {
            let mut title = None;

            if let (Some(atom_name), Some(atom_utf8)) = (net_wm_name, utf8_string) {
                if let Ok(cookie) = conn.get_property(false, win, atom_name, atom_utf8, 0, 1024) {
                    if let Ok(reply) = cookie.reply() {
                        if !reply.value.is_empty() {
                            title = String::from_utf8(reply.value).ok();
                        }
                    }
                }
            }

            if title.is_none() {
                if let Ok(cookie) = conn.get_property(false, win, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024) {
                    if let Ok(reply) = cookie.reply() {
                        if !reply.value.is_empty() {
                            title = String::from_utf8(reply.value).ok();
                        }
                    }
                }
            }

            if let Some(t) = title {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    if let Ok(geo_cookie) = conn.get_geometry(win) {
                        if let Ok(geo) = geo_cookie.reply() {
                            if let Ok(trans_cookie) = conn.translate_coordinates(win, root, 0, 0) {
                                if let Ok(trans) = trans_cookie.reply() {
                                    let is_focused = active_win_id == Some(win);
                                    results.push((
                                        win,
                                        trimmed.to_string(),
                                        trans.dst_x as i32,
                                        trans.dst_y as i32,
                                        geo.width as u32,
                                        geo.height as u32,
                                        is_focused,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        results
    }

    fn find_x11_window(&self) -> Option<WindowInfo> {
        let windows = self.list_all_windows();
        let target_lower = self.target_title.to_lowercase();

        for (win_id, title, x, y, w, h, is_focused) in windows {
            if title.to_lowercase().contains(&target_lower) {
                return Some(WindowInfo {
                    title: title.clone(),
                    x: x.max(0),
                    y: y.max(0),
                    width: w,
                    height: h,
                    is_found: true,
                    is_focused,
                    source: format!("x11 ({})", title),
                    window_id: Some(win_id),
                });
            }
        }

        None
    }
}
