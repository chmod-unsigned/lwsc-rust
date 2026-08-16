//! Input execution and pure Rust X11 continuous mouse, keyboard & global shortcut listener (zero CLI dependencies).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

use crate::core::detector::DetectionResult;
use crate::core::state_thread::StateDetectorThread;
use crate::vision::window::WindowInfo;
use crate::vision::window_tracker::WindowTracker;

pub type HotkeyCallback = Arc<dyn Fn(&str) + Send + Sync + 'static>;

pub struct InputManager {
    state_thread: StateDetectorThread,
    window_tracker: WindowTracker,
    running: Arc<AtomicBool>,
    _listener_handle: Option<JoinHandle<()>>,
}

impl InputManager {
    pub fn new(
        state_thread: StateDetectorThread,
        window_tracker: WindowTracker,
        enable_global_listener: bool,
        on_hotkey: Option<HotkeyCallback>,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let mut handle = None;

        if enable_global_listener {
            let st_clone = state_thread.clone();
            let wt_clone = window_tracker.clone();
            let running_clone = Arc::clone(&running);

            let join_handle = thread::Builder::new()
                .name("X11InputEventListenerThread".to_string())
                .spawn(move || {
                    let (conn, screen_num) = match RustConnection::connect(None) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("[InputManager] Warning: Could not connect to X11 for event tracking: {}", e);
                            return;
                        }
                    };

                    let root = conn.setup().roots[screen_num].root;
                    let mut prev_buttons_mask: u16 = 0;
                    let mut prev_px: i32 = -1;
                    let mut prev_py: i32 = -1;
                    let mut prev_keymap: [u8; 32] = [0; 32];
                    let mut last_motion_trigger = Instant::now();

                    let shortcuts_cfg = crate::core::load_shortcuts_from_config("config/states.yaml");
                    let mut triggers = Vec::new();

                    if let Some(trig) = parse_shortcut_trigger(&conn, "toggle_pause", &shortcuts_cfg.toggle_pause) {
                        triggers.push(trig);
                    }
                    if let Some(trig) = parse_shortcut_trigger(&conn, "open_config", &shortcuts_cfg.open_config) {
                        triggers.push(trig);
                    }
                    if let Some(trig) = parse_shortcut_trigger(&conn, "force_detect", &shortcuts_cfg.force_detect) {
                        triggers.push(trig);
                    }
                    if let Some(trig) = parse_shortcut_trigger(&conn, "show_help", &shortcuts_cfg.show_help) {
                        triggers.push(trig);
                    }

                    // Initial keymap state
                    if let Ok(cookie) = conn.query_keymap() {
                        if let Ok(reply) = cookie.reply() {
                            prev_keymap = reply.keys;
                        }
                    }

                    while running_clone.load(Ordering::Relaxed) {
                        let win = wt_clone.get_window_info();
                        let is_focused = is_game_focused(&conn, root, &win);
                        let mut mask_u16: u16 = 0;

                        // 1. Check Mouse / Pointer Events (movement, clicks, scroll)
                        if let Ok(cookie) = conn.query_pointer(root) {
                            if let Ok(reply) = cookie.reply() {
                                mask_u16 = u16::from(reply.mask);
                                let px = reply.root_x as i32;
                                let py = reply.root_y as i32;

                                if win.is_found {
                                    let in_bounds = px >= win.x
                                        && px <= (win.x + win.width as i32)
                                        && py >= win.y
                                        && py <= (win.y + win.height as i32);

                                    if in_bounds {
                                        let buttons = mask_u16 & 0x1F00;
                                        let prev_buttons = prev_buttons_mask & 0x1F00;

                                        if buttons != prev_buttons {
                                            st_clone.trigger_on_activity("mouse_click", Duration::from_millis(150), false);
                                        } else if (px != prev_px || py != prev_py) && last_motion_trigger.elapsed() >= Duration::from_millis(150) {
                                            last_motion_trigger = Instant::now();
                                            st_clone.trigger_on_activity("mouse_move", Duration::from_millis(50), false);
                                        }
                                    }
                                }

                                prev_buttons_mask = mask_u16;
                                prev_px = px;
                                prev_py = py;
                            }
                        }

                        // 2. Check Keyboard Events & Dynamic Hotkeys (only active when game window has focus)
                        if let Ok(cookie) = conn.query_keymap() {
                            if let Ok(reply) = cookie.reply() {
                                let keys = reply.keys;

                                let ctrl_active = is_key_down(&keys, 37)
                                    || is_key_down(&keys, 105)
                                    || (mask_u16 & 0x0004) != 0;
                                let alt_active = is_key_down(&keys, 64)
                                    || is_key_down(&keys, 108)
                                    || (mask_u16 & 0x0008) != 0;
                                let shift_active = is_key_down(&keys, 50)
                                    || is_key_down(&keys, 62)
                                    || (mask_u16 & 0x0001) != 0;

                                for trigger in triggers.iter_mut() {
                                    let key_pressed = is_key_down(&keys, trigger.keycode);
                                    let ctrl_matches = !trigger.require_ctrl || ctrl_active;
                                    let alt_matches = !trigger.require_alt || alt_active;
                                    let shift_matches = !trigger.require_shift || shift_active;

                                    let fully_pressed = key_pressed && ctrl_matches && alt_matches && shift_matches;

                                    if fully_pressed && !trigger.prev_pressed && is_focused {
                                        if let Some(ref cb) = on_hotkey {
                                            cb(trigger.id);
                                        }
                                    }
                                    trigger.prev_pressed = fully_pressed;
                                }

                                if is_focused {
                                    if keys != prev_keymap {
                                        st_clone.trigger_on_activity("keyboard", Duration::from_millis(150), false);
                                        prev_keymap = keys;
                                    }
                                }
                            }
                        }

                        // Snappy 25ms polling loop for real-time continuous responsiveness
                        thread::sleep(Duration::from_millis(25));
                    }
                })
                .expect("Failed to start X11 input listener thread");

            handle = Some(join_handle);
        }

        Self {
            state_thread,
            window_tracker,
            running,
            _listener_handle: handle,
        }
    }

    pub fn click(
        &self,
        x: i32,
        y: i32,
        relative: bool,
        settle_delay: Duration,
        wait_detection: bool,
    ) -> Option<DetectionResult> {
        self.click_with_cursor_option(x, y, relative, false, settle_delay, wait_detection)
    }

    pub fn click_with_cursor_option(
        &self,
        x: i32,
        y: i32,
        relative: bool,
        save_cursor: bool,
        settle_delay: Duration,
        wait_detection: bool,
    ) -> Option<DetectionResult> {
        let (target_x, target_y) = if relative {
            let win = self.window_tracker.get_window_info();
            (win.x + x, win.y + y)
        } else {
            (x, y)
        };

        println!(
            "[Bot Click] Executing native X11 click at screen coords ({}, {}) (save_cursor: {})",
            target_x, target_y, save_cursor
        );
        send_x11_click_ex(target_x as i16, target_y as i16, save_cursor);
        
        self.state_thread.trigger_on_activity("bot_click", settle_delay, wait_detection)
    }

    /// Clicks a detected button and honors its `save_cursor` preference.
    pub fn click_button(
        &self,
        btn: &crate::core::button::ButtonDetection,
        settle_delay: Duration,
        wait_detection: bool,
    ) -> Option<DetectionResult> {
        self.click_with_cursor_option(
            btn.match_center.0,
            btn.match_center.1,
            true,
            btn.save_cursor,
            settle_delay,
            wait_detection,
        )
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new() -> Self {
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x853c49e6748fea9b);
        Self { state: seed ^ 0xda942042e4dd58b5 }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }

    /// Returns f32 in [0.0, 1.0)
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16777216.0
    }

    /// Returns f32 in [min, max]
    fn gen_range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }
}

/// Generates human-like trajectory points between start and target coords in strictly less than 300ms.
pub fn generate_human_path(
    start_x: i16,
    start_y: i16,
    target_x: i16,
    target_y: i16,
    max_duration_ms: u64,
) -> Vec<(i16, i16, Duration)> {
    let dx = (target_x - start_x) as f32;
    let dy = (target_y - start_y) as f32;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist < 2.0 {
        return vec![(target_x, target_y, Duration::from_millis(5))];
    }

    let mut rng = SimpleRng::new();

    // Bound total duration (e.g. 70ms to max_duration_ms, strictly <= 280ms)
    let max_dur = (max_duration_ms as f32).min(280.0);
    let total_duration_ms = (75.0 + (dist * 0.20).min(max_dur - 85.0) + rng.gen_range_f32(-10.0, 10.0))
        .clamp(60.0, max_dur);

    // Number of steps (interval ~6 to 10ms per step)
    let step_time_ms = rng.gen_range_f32(6.5, 9.5);
    let num_steps = ((total_duration_ms / step_time_ms).round() as usize).clamp(5, 35);

    // Perpendicular vector for natural curved trajectory
    let perp_x = -dy / dist;
    let perp_y = dx / dist;

    // Random curvature deviation
    let max_dev = (dist * 0.15).min(50.0);
    let dev1 = rng.gen_range_f32(-max_dev, max_dev);
    let dev2 = rng.gen_range_f32(-max_dev * 0.6, max_dev * 0.6);

    // Cubic Bézier control points
    let p0 = (start_x as f32, start_y as f32);
    let p1 = (p0.0 + dx * 0.30 + perp_x * dev1, p0.1 + dy * 0.30 + perp_y * dev1);
    let p2 = (p0.0 + dx * 0.70 + perp_x * dev2, p0.1 + dy * 0.70 + perp_y * dev2);
    let p3 = (target_x as f32, target_y as f32);

    let per_step_duration = Duration::from_millis((total_duration_ms / num_steps as f32).max(4.0) as u64);
    let mut path = Vec::with_capacity(num_steps);

    for i in 1..=num_steps {
        let u = i as f32 / num_steps as f32;
        // Smooth quintic easing (slow start, fast mid-motion, gentle deceleration into target)
        let t = u * u * u * (u * (u * 6.0 - 15.0) + 10.0);

        let one_minus_t = 1.0 - t;
        let omt2 = one_minus_t * one_minus_t;
        let omt3 = omt2 * one_minus_t;
        let t2 = t * t;
        let t3 = t2 * t;

        let mut bx = omt3 * p0.0 + 3.0 * omt2 * t * p1.0 + 3.0 * one_minus_t * t2 * p2.0 + t3 * p3.0;
        let mut by = omt3 * p0.1 + 3.0 * omt2 * t * p1.1 + 3.0 * one_minus_t * t2 * p2.1 + t3 * p3.1;

        // Subtle micro-tremor (dampens to zero at target)
        if i < num_steps {
            let tremor = (1.0 - u) * 1.0;
            bx += rng.gen_range_f32(-tremor, tremor);
            by += rng.gen_range_f32(-tremor, tremor);
        }

        path.push((bx.round() as i16, by.round() as i16, per_step_duration));
    }

    if let Some(last) = path.last_mut() {
        last.0 = target_x;
        last.1 = target_y;
    }

    path
}

/// RAII Guard that temporarily isolates physical mouse pointer input during bot actions
/// ensuring physical user movements do not perturb the bot's human Bézier trajectory or click.
pub struct InputGrabGuard<'a> {
    conn: &'a RustConnection,
    grabbed: bool,
}

impl<'a> InputGrabGuard<'a> {
    pub fn grab_pointer(conn: &'a RustConnection, root: u32) -> Self {
        use x11rb::protocol::xproto::{EventMask, GrabMode, GrabStatus, ConnectionExt};

        let grabbed = if let Ok(cookie) = conn.grab_pointer(
            false,
            root,
            EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            x11rb::NONE,
            x11rb::NONE,
            x11rb::CURRENT_TIME,
        ) {
            if let Ok(reply) = cookie.reply() {
                reply.status == GrabStatus::SUCCESS
            } else {
                false
            }
        } else {
            false
        };

        let _ = conn.flush();
        Self { conn, grabbed }
    }
}

impl<'a> Drop for InputGrabGuard<'a> {
    fn drop(&mut self) {
        if self.grabbed {
            use x11rb::protocol::xproto::ConnectionExt;
            let _ = self.conn.ungrab_pointer(x11rb::CURRENT_TIME);
            let _ = self.conn.flush();
        }
    }
}

/// Dispatches native hardware-level mouse click to X11 via human-like Bézier movement in <300ms.
/// If `save_cursor` is true, the cursor position is recorded prior to movement and restored immediately after.
/// Automatically isolates physical mouse movements during the motion to avoid interference.
pub fn send_x11_click_ex(target_x: i16, target_y: i16, save_cursor: bool) -> bool {
    use x11rb::protocol::xtest::ConnectionExt;
    use x11rb::protocol::xproto::ConnectionExt as XProtoExt;

    if let Ok((conn, screen_num)) = RustConnection::connect(None) {
        let root = conn.setup().roots[screen_num].root;

        // 1. Capture OLD pointer position before moving
        let current_pos = if let Ok(cookie) = conn.query_pointer(root) {
            if let Ok(reply) = cookie.reply() {
                Some((reply.root_x, reply.root_y))
            } else {
                None
            }
        } else {
            None
        };

        let (start_x, start_y) = current_pos.unwrap_or((target_x, target_y));
        if save_cursor {
            println!("[Bot Cursor] Saved OLD mouse position: ({}, {})", start_x, start_y);
        }

        // 2. Isolate physical mouse movements during bot movement & click execution
        let _grab_guard = InputGrabGuard::grab_pointer(&conn, root);

        // 3. Human-like movement path towards target (in <250ms)
        let forward_path = generate_human_path(start_x, start_y, target_x, target_y, 250);
        let total_forward_ms: u128 = forward_path.iter().map(|(_, _, d)| d.as_millis()).sum();
        println!(
            "[Bot Movement] Human path ({}, {}) -> ({}, {}): {} steps in {}ms",
            start_x, start_y, target_x, target_y, forward_path.len(), total_forward_ms
        );

        for (px, py, step_delay) in forward_path {
            let _ = conn.xtest_fake_input(6, 0, 0, root, px, py, 0);
            let _ = conn.flush();
            thread::sleep(step_delay);
        }

        // Micro-hesitation before click (15-20ms)
        thread::sleep(Duration::from_millis(15));

        // 4. Button 1 press (ButtonPress = 4, detail = 1)
        let _ = conn.xtest_fake_input(4, 1, 0, root, target_x, target_y, 0);
        let _ = conn.flush();
        thread::sleep(Duration::from_millis(40));

        // 5. Button 1 release (ButtonRelease = 5, detail = 1)
        let _ = conn.xtest_fake_input(5, 1, 0, root, target_x, target_y, 0);
        let _ = conn.flush();

        // 6. Restore original cursor position if save_cursor was requested
        if save_cursor {
            let return_path = generate_human_path(target_x, target_y, start_x, start_y, 160);
            for (px, py, step_delay) in return_path {
                let _ = conn.xtest_fake_input(6, 0, 0, root, px, py, 0);
                let _ = conn.flush();
                thread::sleep(step_delay);
            }
            let _ = conn.warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, start_x, start_y);
            let _ = conn.flush();
            println!("[Bot Cursor] Successfully returned to OLD position: ({}, {})", start_x, start_y);
        }

        return true;
    }
    false
}

/// Dispatches native hardware-level mouse click to X11 via XTest extension
pub fn send_x11_click(target_x: i16, target_y: i16) -> bool {
    send_x11_click_ex(target_x, target_y, false)
}

/// Dispatches native keyboard key press and release to X11 via XTest extension
pub fn send_x11_key(key_name: &str) -> bool {
    use x11rb::protocol::xtest::ConnectionExt;

    if let Ok((conn, screen_num)) = RustConnection::connect(None) {
        let root = conn.setup().roots[screen_num].root;
        let keysym = match key_name.to_lowercase().as_str() {
            "escape" | "esc" => 0xff1b,
            "return" | "enter" => 0xff0d,
            "space" => 0x0020,
            "tab" => 0xff09,
            _ => 0,
        };

        if keysym != 0 {
            if let Some(keycode) = find_keycode_for_keysym(&conn, keysym) {
                // KeyPress = 2, KeyRelease = 3
                let _ = conn.xtest_fake_input(2, keycode, 0, root, 0, 0, 0);
                let _ = conn.flush();
                thread::sleep(Duration::from_millis(40));
                let _ = conn.xtest_fake_input(3, keycode, 0, root, 0, 0, 0);
                let _ = conn.flush();
                return true;
            }
        }
    }
    false
}

fn is_key_down(keys: &[u8; 32], keycode: u8) -> bool {
    let byte_idx = (keycode / 8) as usize;
    let bit_idx = keycode % 8;
    if byte_idx < 32 {
        (keys[byte_idx] & (1 << bit_idx)) != 0
    } else {
        false
    }
}

fn find_keycode_for_keysym(conn: &RustConnection, target_keysym: u32) -> Option<u8> {
    let setup = conn.setup();
    let min_keycode = setup.min_keycode;
    let max_keycode = setup.max_keycode;
    let count = max_keycode.saturating_sub(min_keycode) + 1;

    if let Ok(cookie) = conn.get_keyboard_mapping(min_keycode, count) {
        if let Ok(reply) = cookie.reply() {
            let per_key = reply.keysyms_per_keycode as usize;
            if per_key > 0 {
                for (idx, chunk) in reply.keysyms.chunks(per_key).enumerate() {
                    for &sym in chunk {
                        if sym == target_keysym {
                            return Some(min_keycode + idx as u8);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Checks in real-time if the game window currently possesses active input focus.
fn is_game_focused(conn: &RustConnection, root: u32, win: &WindowInfo) -> bool {
    if !win.is_found {
        return false;
    }

    if let Some(target_id) = win.window_id {
        // Query _NET_ACTIVE_WINDOW for real-time focus
        if let Ok(cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") {
            if let Ok(atom_reply) = cookie.reply() {
                if let Ok(prop_cookie) = conn.get_property(false, root, atom_reply.atom, AtomEnum::WINDOW, 0, 1) {
                    if let Ok(reply) = prop_cookie.reply() {
                        if let Some(mut iter) = reply.value32() {
                            if let Some(active_id) = iter.next() {
                                if active_id == target_id {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: check direct input focus
        if let Ok(cookie) = conn.get_input_focus() {
            if let Ok(reply) = cookie.reply() {
                if reply.focus == target_id {
                    return true;
                }
            }
        }
    }

    win.is_focused
}

#[derive(Debug, Clone)]
pub struct ShortcutTrigger {
    pub id: &'static str,
    pub keycode: u8,
    pub require_ctrl: bool,
    pub require_alt: bool,
    pub require_shift: bool,
    pub prev_pressed: bool,
}

fn resolve_keysym_from_str(s: &str) -> u32 {
    match s.trim().to_lowercase().as_str() {
        "a" => 0x0061,
        "b" => 0x0062,
        "c" => 0x0063,
        "d" => 0x0064,
        "e" => 0x0065,
        "f" => 0x0066,
        "g" => 0x0067,
        "h" => 0x0068,
        "i" => 0x0069,
        "j" => 0x006a,
        "k" => 0x006b,
        "l" => 0x006c,
        "m" => 0x006d,
        "n" => 0x006e,
        "o" => 0x006f,
        "p" => 0x0070,
        "q" => 0x0071,
        "r" => 0x0072,
        "s" => 0x0073,
        "t" => 0x0074,
        "u" => 0x0075,
        "v" => 0x0076,
        "w" => 0x0077,
        "x" => 0x0078,
        "y" => 0x0079,
        "z" => 0x007a,
        "f1" => 0xffbe,
        "f2" => 0xffbf,
        "f3" => 0xffc0,
        "f4" => 0xffc1,
        "f5" => 0xffc2,
        "f6" => 0xffc3,
        "f7" => 0xffc4,
        "f8" => 0xffc5,
        "f9" => 0xffc6,
        "f10" => 0xffc7,
        "f11" => 0xffc8,
        "f12" => 0xffc9,
        "space" => 0x0020,
        "escape" | "esc" => 0xff1b,
        "return" | "enter" => 0xff0d,
        "tab" => 0xff09,
        "pause" => 0xff13,
        _ => 0,
    }
}

fn parse_shortcut_trigger(conn: &RustConnection, id: &'static str, spec: &str) -> Option<ShortcutTrigger> {
    let lower_parts: Vec<String> = spec.split('+').map(|s| s.trim().to_lowercase()).collect();
    let mut require_ctrl = false;
    let mut require_alt = false;
    let mut require_shift = false;
    let mut key_part = "";

    for part in &lower_parts {
        match part.as_str() {
            "ctrl" | "control" => require_ctrl = true,
            "alt" => require_alt = true,
            "shift" => require_shift = true,
            k => key_part = k,
        }
    }

    let keysym = resolve_keysym_from_str(key_part);
    if keysym != 0 {
        if let Some(keycode) = find_keycode_for_keysym(conn, keysym) {
            return Some(ShortcutTrigger {
                id,
                keycode,
                require_ctrl,
                require_alt,
                require_shift,
                prev_pressed: false,
            });
        }
    }
    None
}
