//! Native pure Rust X11 Configuration & Action Manager Window (Ctrl+O).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, Gcontext,
    PropMode, Rectangle, Window, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::core::action::ActionManager;
use crate::core::state::load_actions_from_config;
use crate::core::state_thread::StateDetectorThread;
use crate::vision::window_tracker::WindowTracker;

static IS_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);

pub struct ConfigWindow;

impl ConfigWindow {
    /// Opens the configuration window in a background thread if not already open.
    /// If already open, toggles it closed.
    pub fn open_or_toggle(
        action_manager: Arc<ActionManager>,
        state_thread: StateDetectorThread,
        window_tracker: WindowTracker,
    ) -> Option<JoinHandle<()>> {
        if IS_WINDOW_OPEN.load(Ordering::SeqCst) {
            IS_WINDOW_OPEN.store(false, Ordering::SeqCst);
            return None;
        }

        IS_WINDOW_OPEN.store(true, Ordering::SeqCst);

        let handle = thread::Builder::new()
            .name("ConfigWindowThread".to_string())
            .spawn(move || {
                run_config_window(action_manager, state_thread, window_tracker);
                IS_WINDOW_OPEN.store(false, Ordering::SeqCst);
            })
            .ok();

        handle
    }

    pub fn is_open() -> bool {
        IS_WINDOW_OPEN.load(Ordering::SeqCst)
    }
}

fn run_config_window(
    action_manager: Arc<ActionManager>,
    state_thread: StateDetectorThread,
    window_tracker: WindowTracker,
) {
    let (conn, screen_num) = match RustConnection::connect(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[ConfigWindow] Error: Could not connect to X11: {}", e);
            return;
        }
    };

    let screen = &conn.setup().roots[screen_num];
    let win_id = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };

    let win_width = 720;
    let win_height = 540;

    let win_aux = CreateWindowAux::new()
        .background_pixel(0x00181825) // Dark theme background
        .event_mask(
            EventMask::EXPOSURE
                | EventMask::KEY_PRESS
                | EventMask::BUTTON_PRESS
                | EventMask::STRUCTURE_NOTIFY,
        );

    let create_res = conn.create_window(
        screen.root_depth,
        win_id,
        screen.root,
        150,
        150,
        win_width,
        win_height,
        1,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &win_aux,
    );

    if create_res.is_err() {
        return;
    }

    // Set Window Title
    let title = "LWSC2 - Configuration & Action Manager (Ctrl+O)";
    let _ = conn.change_property8(
        PropMode::REPLACE,
        win_id,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        title.as_bytes(),
    );

    // Map window
    if conn.map_window(win_id).is_err() {
        return;
    }
    let _ = conn.flush();

    // Create GCs for drawing
    let gc_bg = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };
    let gc_card_active = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };
    let gc_card_inactive = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };
    let gc_text_white = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };
    let gc_text_cyan = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };
    let gc_badge_green = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };
    let gc_badge_red = match conn.generate_id() {
        Ok(id) => id,
        Err(_) => return,
    };

    let _ = conn.create_gc(gc_bg, win_id, &CreateGCAux::new().foreground(0x00181825));
    let _ = conn.create_gc(gc_card_active, win_id, &CreateGCAux::new().foreground(0x0024273a));
    let _ = conn.create_gc(gc_card_inactive, win_id, &CreateGCAux::new().foreground(0x001e1e2e));
    let _ = conn.create_gc(gc_text_white, win_id, &CreateGCAux::new().foreground(0x00cad3f5));
    let _ = conn.create_gc(gc_text_cyan, win_id, &CreateGCAux::new().foreground(0x008aadf4));
    let _ = conn.create_gc(gc_badge_green, win_id, &CreateGCAux::new().foreground(0x00a6da95));
    let _ = conn.create_gc(gc_badge_red, win_id, &CreateGCAux::new().foreground(0x00ed8796));

    let mut notification_msg = String::new();
    let mut notification_expire = std::time::Instant::now();

    while IS_WINDOW_OPEN.load(Ordering::SeqCst) {
        // Poll and handle X11 events
        while let Ok(Some(event)) = conn.poll_for_event() {
            match event {
                Event::Expose(_) => {
                    redraw_ui(
                        &conn,
                        win_id,
                        win_width,
                        win_height,
                        &action_manager,
                        &state_thread,
                        &window_tracker,
                        gc_bg,
                        gc_card_active,
                        gc_card_inactive,
                        gc_text_white,
                        gc_text_cyan,
                        gc_badge_green,
                        gc_badge_red,
                        &notification_msg,
                    );
                }
                Event::ButtonPress(bp) => {
                    let click_y = bp.event_y as i32;
                    let actions = action_manager.list_actions();
                    let start_y = 150;
                    let card_height = 65;

                    for (idx, action) in actions.iter().enumerate() {
                        let y = start_y + (idx as i32) * card_height;
                        if click_y >= y && click_y <= y + card_height - 8 {
                            let new_state = !action.enabled;
                            action_manager.set_action_enabled(&action.name, new_state);
                            notification_msg = format!(
                                "Action '{}' set to {}",
                                action.name,
                                if new_state { "ACTIVE" } else { "INACTIVE" }
                            );
                            notification_expire = std::time::Instant::now() + Duration::from_secs(3);
                            break;
                        }
                    }

                    // Check Bottom Buttons (Save, Reload)
                    if bp.event_y >= 490 && bp.event_y <= 525 {
                        if bp.event_x >= 30 && bp.event_x <= 150 {
                            // Save
                            if save_actions_to_yaml(&action_manager, "config/states.yaml") {
                                notification_msg = "Saved configuration to states.yaml".to_string();
                            } else {
                                notification_msg = "Error saving to states.yaml".to_string();
                            }
                            notification_expire = std::time::Instant::now() + Duration::from_secs(3);
                        } else if bp.event_x >= 160 && bp.event_x <= 280 {
                            // Reload
                            if let Ok(loaded) = load_actions_from_config("config/states.yaml") {
                                for a in loaded {
                                    action_manager.set_action_enabled(&a.name, a.enabled);
                                }
                                notification_msg = "Reloaded config from states.yaml".to_string();
                            }
                            notification_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                    }

                    redraw_ui(
                        &conn,
                        win_id,
                        win_width,
                        win_height,
                        &action_manager,
                        &state_thread,
                        &window_tracker,
                        gc_bg,
                        gc_card_active,
                        gc_card_inactive,
                        gc_text_white,
                        gc_text_cyan,
                        gc_badge_green,
                        gc_badge_red,
                        &notification_msg,
                    );
                }
                Event::KeyPress(kp) => {
                    let ctrl_pressed = (u16::from(kp.state) & 0x0004) != 0;
                    // Keycode 9 = Escape, 24 = Q, 32 = O, 39 = S, 27 = R
                    if kp.detail == 9 || kp.detail == 24 || (kp.detail == 32 && ctrl_pressed) {
                        // Escape, Q, or Ctrl+O -> Close
                        IS_WINDOW_OPEN.store(false, Ordering::SeqCst);
                        break;
                    }
                    match kp.detail {
                        39 => {
                            // 'S' -> Save
                            if save_actions_to_yaml(&action_manager, "config/states.yaml") {
                                notification_msg = "Saved configuration to states.yaml".to_string();
                            }
                            notification_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        27 => {
                            // 'R' -> Reload
                            if let Ok(loaded) = load_actions_from_config("config/states.yaml") {
                                for a in loaded {
                                    action_manager.set_action_enabled(&a.name, a.enabled);
                                }
                                notification_msg = "Reloaded config from states.yaml".to_string();
                            }
                            notification_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        // Keys 1..9 (Keycodes 10..18 in standard X11)
                        kc @ 10..=18 => {
                            let idx = (kc - 10) as usize;
                            let actions = action_manager.list_actions();
                            if let Some(action) = actions.get(idx) {
                                let new_state = !action.enabled;
                                action_manager.set_action_enabled(&action.name, new_state);
                                notification_msg = format!(
                                    "Action '{}' set to {}",
                                    action.name,
                                    if new_state { "ACTIVE" } else { "INACTIVE" }
                                );
                                notification_expire = std::time::Instant::now() + Duration::from_secs(3);
                            }
                        }
                        _ => {}
                    }

                    redraw_ui(
                        &conn,
                        win_id,
                        win_width,
                        win_height,
                        &action_manager,
                        &state_thread,
                        &window_tracker,
                        gc_bg,
                        gc_card_active,
                        gc_card_inactive,
                        gc_text_white,
                        gc_text_cyan,
                        gc_badge_green,
                        gc_badge_red,
                        &notification_msg,
                    );
                }
                Event::DestroyNotify(_) => {
                    IS_WINDOW_OPEN.store(false, Ordering::SeqCst);
                    break;
                }
                _ => {}
            }
        }

        if !notification_msg.is_empty() && std::time::Instant::now() > notification_expire {
            notification_msg.clear();
            redraw_ui(
                &conn,
                win_id,
                win_width,
                win_height,
                &action_manager,
                &state_thread,
                &window_tracker,
                gc_bg,
                gc_card_active,
                gc_card_inactive,
                gc_text_white,
                gc_text_cyan,
                gc_badge_green,
                gc_badge_red,
                &notification_msg,
            );
        }

        thread::sleep(Duration::from_millis(50));
    }

    let _ = conn.destroy_window(win_id);
    let _ = conn.flush();
}

fn redraw_ui(
    conn: &RustConnection,
    win: Window,
    w: u16,
    h: u16,
    action_manager: &ActionManager,
    state_thread: &StateDetectorThread,
    window_tracker: &WindowTracker,
    gc_bg: Gcontext,
    gc_card_active: Gcontext,
    gc_card_inactive: Gcontext,
    gc_text_white: Gcontext,
    gc_text_cyan: Gcontext,
    gc_badge_green: Gcontext,
    gc_badge_red: Gcontext,
    notification_msg: &str,
) {
    // 1. Clear background
    let bg_rect = Rectangle {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    let _ = conn.poly_fill_rectangle(win, gc_bg, &[bg_rect]);

    // 2. Header
    let _ = conn.image_text8(win, gc_text_cyan, 30, 35, b"=== LWSC2 BOT CONFIGURATION & ACTION MANAGER ===");

    // 3. Status Section
    let current_state = state_thread.get_current_state();
    let current_root = state_thread.get_current_root_state();
    let win_info = window_tracker.get_window_info();

    let state_line = format!(
        "Game State: {} | Root: {}",
        current_state.name(),
        current_root.map(|r| r.name()).unwrap_or("UNKNOWN")
    );
    let _ = conn.image_text8(win, gc_text_white, 30, 65, state_line.as_bytes());

    let target_line = format!(
        "Target Window: {} ({}) | Focused: {}",
        win_info.title,
        if win_info.is_found { format!("{}x{} px", win_info.width, win_info.height) } else { "Not Found".to_string() },
        if win_info.is_focused { "YES" } else { "NO" }
    );
    let _ = conn.image_text8(win, gc_text_white, 30, 85, target_line.as_bytes());

    // 4. Action List Section
    let _ = conn.image_text8(win, gc_text_cyan, 30, 125, b"--- Automated Actions (Click or press [1-9] to Toggle) ---");

    let actions = action_manager.list_actions();
    let start_y = 145;
    let card_height = 65;

    for (idx, action) in actions.iter().enumerate() {
        let y = start_y + (idx as i16) * card_height;
        let card_gc = if action.enabled { gc_card_active } else { gc_card_inactive };

        // Card Box
        let card_rect = Rectangle {
            x: 30,
            y,
            width: w - 60,
            height: card_height as u16 - 8,
        };
        let _ = conn.poly_fill_rectangle(win, card_gc, &[card_rect]);

        // Action Name
        let name_line = format!("[{}] {}", idx + 1, action.name);
        let _ = conn.image_text8(win, gc_text_white, 45, y + 22, name_line.as_bytes());

        // Status Badge
        if action.enabled {
            let _ = conn.image_text8(win, gc_badge_green, w as i16 - 150, y + 22, b"[ ACTIVE ]");
        } else {
            let _ = conn.image_text8(win, gc_badge_red, w as i16 - 150, y + 22, b"[ INACTIVE ]");
        }

        // Details line
        let roi_str = if let Some(roi) = action.roi {
            format!("ROI: {:.0}-{:.0}% X, {:.0}-{:.0}% Y", roi.xmin * 100.0, roi.xmax * 100.0, roi.ymin * 100.0, roi.ymax * 100.0)
        } else {
            "ROI: Global".to_string()
        };
        let detail_line = format!(
            "State: {:<12} | {} | Cooldown: {:.1}s",
            action.state.map(|s| s.name()).unwrap_or("ALL"),
            roi_str,
            action.cooldown_s
        );
        let _ = conn.image_text8(win, gc_text_cyan, 45, y + 42, detail_line.as_bytes());
    }

    // 5. Notification Bar
    if !notification_msg.is_empty() {
        let notif_line = format!(">> {}", notification_msg);
        let _ = conn.image_text8(win, gc_badge_green, 30, h as i16 - 55, notif_line.as_bytes());
    }

    // 6. Footer Buttons / Hints
    let footer_hints = b"[S] Save to states.yaml  |  [R] Reload config  |  [Esc / Ctrl+O] Close";
    let _ = conn.image_text8(win, gc_text_white, 30, h as i16 - 20, footer_hints);

    let _ = conn.flush();
}

fn save_actions_to_yaml(manager: &ActionManager, yaml_path: &str) -> bool {
    let content = match std::fs::read_to_string(yaml_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let mut cfg: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let updated_actions = manager.list_actions();
    if let Ok(actions_value) = serde_yaml::to_value(updated_actions) {
        if let Some(map) = cfg.as_mapping_mut() {
            map.insert(serde_yaml::Value::String("actions".to_string()), actions_value);
            if let Ok(updated_str) = serde_yaml::to_string(&cfg) {
                return std::fs::write(yaml_path, updated_str).is_ok();
            }
        }
    }
    false
}
