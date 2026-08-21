//! Native eframe/egui Configuration & Action Manager Window (Ctrl+O).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eframe::egui;

use crate::core::action::{ActionManager, SequenceDefinition};
use crate::core::state::{load_shortcuts_from_config, ShortcutsConfig};
use crate::core::state_thread::StateDetectorThread;
use crate::vision::window_tracker::WindowTracker;

static IS_EVENT_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);
static SHOW_FULL_CONFIG_REQUEST: AtomicBool = AtomicBool::new(false);
static SHOW_QUICK_LAUNCHER_REQUEST: AtomicBool = AtomicBool::new(false);
static EGUI_CTX: std::sync::RwLock<Option<egui::Context>> = std::sync::RwLock::new(None);

#[derive(PartialEq, Clone, Copy)]
pub enum WindowMode {
    FullConfig,
    QuickLauncher,
}

pub struct ConfigWindow;

impl ConfigWindow {
    /// Opens or toggles the full configuration window (Ctrl+O).
    pub fn open_or_toggle(
        _action_manager: Arc<ActionManager>,
        _state_thread: StateDetectorThread,
        _window_tracker: WindowTracker,
    ) {
        SHOW_FULL_CONFIG_REQUEST.store(true, Ordering::SeqCst);
        if let Ok(guard) = EGUI_CTX.read() {
            if let Some(ref ctx) = *guard {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint();
            }
        }
    }

    /// Opens or toggles the mini Quick Launcher window (Ctrl+X).
    pub fn open_quick_launcher(
        _action_manager: Arc<ActionManager>,
        _state_thread: StateDetectorThread,
        _window_tracker: WindowTracker,
    ) {
        SHOW_QUICK_LAUNCHER_REQUEST.store(true, Ordering::SeqCst);
        if let Ok(guard) = EGUI_CTX.read() {
            if let Some(ref ctx) = *guard {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint();
            }
        }
    }

    /// Initializes and runs the native eframe event loop on the main thread.
    /// The window is created hidden initially and toggled visible on hotkey triggers.
    pub fn run_on_main_thread(
        action_manager: Arc<ActionManager>,
        state_thread: StateDetectorThread,
        window_tracker: WindowTracker,
    ) {
        IS_EVENT_LOOP_RUNNING.store(true, Ordering::SeqCst);

        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([760.0, 580.0])
                .with_always_on_top()
                .with_visible(false) // Start invisible/hidden
                .with_title("LWSC2 - Configuration & Action Manager (Ctrl+O)"),
            ..Default::default()
        };

        let result = eframe::run_native(
            "LWSC2 Manager",
            options,
            Box::new(move |cc| {
                let mut visuals = eframe::egui::Visuals::dark();
                visuals.window_rounding = 8.0.into();
                visuals.menu_rounding = 8.0.into();
                visuals.widgets.noninteractive.rounding = 4.0.into();
                visuals.widgets.inactive.rounding = 4.0.into();
                visuals.widgets.hovered.rounding = 4.0.into();
                visuals.widgets.active.rounding = 4.0.into();

                visuals.window_fill = eframe::egui::Color32::from_rgb(26, 28, 34);
                visuals.panel_fill = eframe::egui::Color32::from_rgb(26, 28, 34);

                cc.egui_ctx.set_visuals(visuals);

                let mut app = Lwsc2ConfigApp::new(action_manager, state_thread, window_tracker, WindowMode::FullConfig);
                app.is_visible = false; // Hidden initially
                Box::new(app)
            }),
        );
        println!("[UI] eframe::run_native finished on main thread: {:?}", result);
        IS_EVENT_LOOP_RUNNING.store(false, Ordering::SeqCst);
    }

    pub fn is_open() -> bool {
        IS_EVENT_LOOP_RUNNING.load(Ordering::SeqCst)
    }
}

#[derive(PartialEq)]
enum ConfigTab {
    Dashboard,
    Actions,
    Sequences,
    Shortcuts,
}

struct Lwsc2ConfigApp {
    action_manager: Arc<ActionManager>,
    state_thread: StateDetectorThread,
    window_tracker: WindowTracker,
    notification_msg: String,
    notification_expire: std::time::Instant,
    mode: WindowMode,
    current_tab: ConfigTab,
    is_visible: bool,
    sequences: Vec<SequenceDefinition>,
    actions: Vec<crate::core::action::ActionDefinition>,
    shortcuts: ShortcutsConfig,
    selected_action_idx: usize,
    selected_seq_idx: usize,
}

impl Lwsc2ConfigApp {
    fn new(
        action_manager: Arc<ActionManager>,
        state_thread: StateDetectorThread,
        window_tracker: WindowTracker,
        initial_mode: WindowMode,
    ) -> Self {
        let sequences = action_manager.list_sequences();
        let actions = action_manager.list_actions();
        let shortcuts = load_shortcuts_from_config("config/shortcuts.yaml");
        Self {
            action_manager,
            state_thread,
            window_tracker,
            notification_msg: String::new(),
            notification_expire: std::time::Instant::now(),
            mode: initial_mode,
            current_tab: ConfigTab::Dashboard,
            is_visible: true,
            sequences,
            actions,
            shortcuts,
            selected_action_idx: 0,
            selected_seq_idx: 0,
        }
    }

    fn notify(&mut self, msg: impl Into<String>) {
        self.notification_msg = msg.into();
        self.notification_expire = std::time::Instant::now() + std::time::Duration::from_secs(3);
    }

}

fn execute_action_worker(
    action_manager: &Arc<ActionManager>,
    state_thread: &StateDetectorThread,
    window_tracker: &WindowTracker,
    action_name: &str,
) {
    use colored::Colorize;
    let win = window_tracker.get_window_info();
    if !win.is_found {
        println!("{}", "[Quick Launcher] Error: Game window not found".red());
        return;
    }

    let capturer = crate::vision::ScreenCapturer::new();
    if let Some(frame) = capturer.capture_roi(win.x, win.y, win.width, win.height) {
        let current_state = state_thread.get_current_state();
        let mut matcher = crate::vision::matching::TemplateMatcher::new(".");
        if let Some(action_res) = action_manager.execute_single_action(
            action_name,
            current_state,
            &frame,
            &mut matcher,
            true, // bypass cooldown on manual trigger
            true, // bypass state check on manual trigger
        ) {
            if action_res.executed {
                if let Some((cx, cy)) = action_res.click_coords {
                    let screen_x = win.x + cx;
                    let screen_y = win.y + cy;
                    println!(
                        "{}",
                        format!(
                            "[Quick Launcher] Action '{}' executed -> Clicking at ({}, {})",
                            action_name, screen_x, screen_y
                        ).green().bold()
                    );
                    crate::io::input::send_x11_click_ex(
                        win.window_id,
                        screen_x as i16,
                        screen_y as i16,
                        action_res.save_cursor,
                    );
                    state_thread.trigger_on_activity(
                        "quick_launcher_click",
                        std::time::Duration::from_millis(150),
                        false,
                    );
                } else if let Some(((sx, sy), (ex, ey))) = action_res.drag_coords {
                    let screen_sx = win.x + sx;
                    let screen_sy = win.y + sy;
                    let screen_ex = win.x + ex;
                    let screen_ey = win.y + ey;
                    println!(
                        "{}",
                        format!(
                            "[Quick Launcher] Action '{}' executed -> Dragging from ({}, {}) to ({}, {})",
                            action_name, screen_sx, screen_sy, screen_ex, screen_ey
                        ).green().bold()
                    );
                    crate::io::input::send_x11_drag(
                        win.window_id,
                        screen_sx as i16,
                        screen_sy as i16,
                        screen_ex as i16,
                        screen_ey as i16,
                        action_res.drag_duration_ms,
                        action_res.save_cursor,
                        None,
                    );
                    state_thread.trigger_on_activity(
                        "quick_launcher_drag",
                        std::time::Duration::from_millis(150),
                        false,
                    );
                } else if let Some(ref script_path) = action_res.script {
                    let screenshot_file = "last_screenshot.png";
                    let _ = frame.save(screenshot_file);
                    let win_id = win.window_id;
                    let state_str = current_state.to_string();
                    let script = script_path.clone();
                    let args = action_res.script_args.clone();
                    thread::spawn(move || {
                        crate::core::action::run_python_script(
                            &script,
                            &args,
                            win_id,
                            Some(&state_str),
                            Some(screenshot_file),
                        );
                    });
                }
            } else {
                println!(
                    "{}",
                    format!("[Quick Launcher] Action '{}' skipped: {}", action_name, action_res.reason).yellow()
                );
            }
        }
    }
}

fn execute_sequence_worker(
    action_manager: &Arc<ActionManager>,
    state_thread: &StateDetectorThread,
    seq_name: &str,
) {
    use colored::Colorize;
    if action_manager.trigger_sequence(seq_name) {
        println!("{}", format!("[Quick Launcher] Sequence '{}' started.", seq_name).green().bold());
        state_thread.trigger_on_activity(
            "quick_launcher_seq",
            std::time::Duration::from_millis(50),
            false,
        );
    } else {
        println!("{}", format!("[Quick Launcher] Failed to start sequence '{}' (not found or already active).", seq_name).red());
    }
}

impl eframe::App for Lwsc2ConfigApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Store context EVERY frame so request_repaint() from other threads always
        // uses the latest event loop proxy (it changes after minimize/restore cycles).
        if let Ok(mut guard) = EGUI_CTX.write() {
            *guard = Some(ctx.clone());
        }
        // Keep the event loop alive with a slow repaint cadence even when hidden,
        // so that SHOW_*_REQUEST flags get picked up without ctx.request_repaint()
        // from another thread needing to pierce epoll_wait.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        if SHOW_FULL_CONFIG_REQUEST.swap(false, Ordering::SeqCst) {
            if self.is_visible && self.mode == WindowMode::FullConfig {
                self.is_visible = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            } else {
                self.mode = WindowMode::FullConfig;
                self.is_visible = true;
                self.actions = self.action_manager.list_actions();
                self.sequences = self.action_manager.list_sequences();
                self.shortcuts = load_shortcuts_from_config("config/shortcuts.yaml");
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(760.0, 580.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("LWSC2 - Configuration & Action Manager (Ctrl+O)".to_string()));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                thread::spawn(|| {
                    thread::sleep(Duration::from_millis(50));
                    raise_x11_window_to_top("LWSC2");
                });
            }
        }

        if SHOW_QUICK_LAUNCHER_REQUEST.swap(false, Ordering::SeqCst) {
            if self.is_visible && self.mode == WindowMode::QuickLauncher {
                self.is_visible = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            } else {
                self.mode = WindowMode::QuickLauncher;
                self.is_visible = true;
                self.actions = self.action_manager.list_actions();
                self.sequences = self.action_manager.list_sequences();
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(480.0, 160.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("LWSC2 - Quick Launcher (Ctrl+X)".to_string()));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                thread::spawn(|| {
                    thread::sleep(Duration::from_millis(50));
                    raise_x11_window_to_top("LWSC2");
                });
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.is_visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        if !self.is_visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        match self.mode {
            WindowMode::QuickLauncher => {
                self.render_quick_launcher(ctx);
            }
            WindowMode::FullConfig => {
                self.render_full_config(ctx);
            }
        }
    }
}

pub fn raise_x11_window_to_top(title: &str) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, EventMask};

    if let Ok((conn, screen_num)) = x11rb::connect(None) {
        let setup = conn.setup();
        let root = setup.roots[screen_num].root;

        if let Ok(net_client_list) = conn.intern_atom(false, b"_NET_CLIENT_LIST") {
            if let Ok(atom_reply) = net_client_list.reply() {
                if let Ok(prop_cookie) = conn.get_property(false, root, atom_reply.atom, AtomEnum::WINDOW, 0, 1024) {
                    if let Ok(prop_reply) = prop_cookie.reply() {
                        if let Some(windows) = prop_reply.value32() {
                            for win_id in windows {
                                if let Ok(name_cookie) = conn.get_property(false, win_id, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024) {
                                    if let Ok(name_reply) = name_cookie.reply() {
                                        let name = String::from_utf8_lossy(&name_reply.value);
                                        if name.contains(title) || title.contains(&*name) {
                                            // 1. Set _NET_WM_STATE_ABOVE and _NET_WM_STATE_STAYS_ON_TOP
                                            if let (Ok(net_wm_state), Ok(net_wm_above), Ok(net_wm_stays_on_top)) = (
                                                conn.intern_atom(false, b"_NET_WM_STATE"),
                                                conn.intern_atom(false, b"_NET_WM_STATE_ABOVE"),
                                                conn.intern_atom(false, b"_NET_WM_STATE_STAYS_ON_TOP"),
                                            ) {
                                                if let (Ok(state_atom), Ok(above_atom), Ok(stays_atom)) = (
                                                    net_wm_state.reply(),
                                                    net_wm_above.reply(),
                                                    net_wm_stays_on_top.reply(),
                                                ) {
                                                    let event = x11rb::protocol::xproto::ClientMessageEvent {
                                                        response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
                                                        format: 32,
                                                        sequence: 0,
                                                        window: win_id,
                                                        type_: state_atom.atom,
                                                        data: x11rb::protocol::xproto::ClientMessageData::from([
                                                            1, // _NET_WM_STATE_ADD
                                                            above_atom.atom,
                                                            stays_atom.atom,
                                                            1, // normal source indication
                                                            0,
                                                        ]),
                                                    };
                                                    let _ = conn.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event);
                                                }
                                            }

                                            // 2. Set _NET_ACTIVE_WINDOW
                                            if let Ok(net_active) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") {
                                                if let Ok(active_atom) = net_active.reply() {
                                                    let event = x11rb::protocol::xproto::ClientMessageEvent {
                                                        response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
                                                        format: 32,
                                                        sequence: 0,
                                                        window: win_id,
                                                        type_: active_atom.atom,
                                                        data: x11rb::protocol::xproto::ClientMessageData::from([
                                                            1, // source: application
                                                            0, // timestamp
                                                            0,
                                                            0,
                                                            0,
                                                        ]),
                                                    };
                                                    let _ = conn.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event);
                                                }
                                            }

                                            // 3. ConfigureWindow stack above
                                            let values = x11rb::protocol::xproto::ConfigureWindowAux::default()
                                                .stack_mode(x11rb::protocol::xproto::StackMode::ABOVE);
                                            let _ = conn.configure_window(win_id, &values);
                                            let _ = conn.flush();
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Lwsc2ConfigApp {
    fn render_quick_launcher(&mut self, ctx: &egui::Context) {
        let mut action_to_run = None;
        let mut seq_to_run = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("⚡ Quick Launcher")
                        .size(16.0)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("[Esc: Fermer]")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(140, 140, 140)),
                    );
                    if ui.small_button("⚙ Options").clicked() {
                        SHOW_FULL_CONFIG_REQUEST.store(true, Ordering::SeqCst);
                    }
                });
            });

            ui.separator();
            ui.add_space(6.0);

            // Row 1: Actions Dropdown + Play button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Action :").strong().size(13.0));
                ui.add_space(14.0);

                let selected_action_label = if let Some(a) = self.actions.get(self.selected_action_idx) {
                    let sc = a
                        .shortcut
                        .as_deref()
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| format!(" [{}]", s.trim()))
                        .unwrap_or_default();
                    if !a.display_name.is_empty() {
                        format!("{}{}", a.display_name, sc)
                    } else {
                        format!("{}{}", a.name, sc)
                    }
                } else {
                    "Aucune action disponible".to_string()
                };

                egui::ComboBox::from_id_source("quick_action_select")
                    .selected_text(selected_action_label)
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for (idx, a) in self.actions.iter().enumerate() {
                            let sc = a
                                .shortcut
                                .as_deref()
                                .filter(|s| !s.trim().is_empty())
                                .map(|s| format!(" [{}]", s.trim()))
                                .unwrap_or_default();
                            let text = if !a.display_name.is_empty() {
                                format!("{}. {}{}", idx + 1, a.display_name, sc)
                            } else {
                                format!("{}. {}{}", idx + 1, a.name, sc)
                            };
                            ui.selectable_value(&mut self.selected_action_idx, idx, text);
                        }
                    });

                let play_btn = egui::Button::new(
                    egui::RichText::new("▶ Play")
                        .strong()
                        .color(egui::Color32::BLACK),
                )
                .fill(egui::Color32::from_rgb(100, 220, 120));

                if ui.add_sized([65.0, 24.0], play_btn).clicked() {
                    if let Some(a) = self.actions.get(self.selected_action_idx) {
                        action_to_run = Some(a.name.clone());
                    }
                }
            });

            ui.add_space(6.0);

            // Row 2: Sequences Dropdown + Play button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Séquence :").strong().size(13.0));
                ui.add_space(2.0);

                let selected_seq_label = if let Some(s) = self.sequences.get(self.selected_seq_idx) {
                    let sc = s
                        .shortcut
                        .as_deref()
                        .filter(|sc| !sc.trim().is_empty())
                        .map(|sc| format!(" [{}]", sc.trim()))
                        .unwrap_or_default();
                    format!("{}{}", s.name, sc)
                } else {
                    "Aucune séquence disponible".to_string()
                };

                egui::ComboBox::from_id_source("quick_seq_select")
                    .selected_text(selected_seq_label)
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for (idx, s) in self.sequences.iter().enumerate() {
                            let sc = s
                                .shortcut
                                .as_deref()
                                .filter(|sc| !sc.trim().is_empty())
                                .map(|sc| format!(" [{}]", sc.trim()))
                                .unwrap_or_default();
                            let text = format!("{}. {}{}", idx + 1, s.name, sc);
                            ui.selectable_value(&mut self.selected_seq_idx, idx, text);
                        }
                    });

                let play_btn = egui::Button::new(
                    egui::RichText::new("▶ Play")
                        .strong()
                        .color(egui::Color32::BLACK),
                )
                .fill(egui::Color32::from_rgb(100, 180, 255));

                if ui.add_sized([65.0, 24.0], play_btn).clicked() {
                    if let Some(s) = self.sequences.get(self.selected_seq_idx) {
                        seq_to_run = Some(s.name.clone());
                    }
                }
            });
        });

        if let Some(action_name) = action_to_run {
            self.is_visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));

            let am = Arc::clone(&self.action_manager);
            let st = self.state_thread.clone();
            let wt = self.window_tracker.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(80));
                execute_action_worker(&am, &st, &wt, &action_name);
            });
        } else if let Some(seq_name) = seq_to_run {
            self.is_visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));

            let am = Arc::clone(&self.action_manager);
            let st = self.state_thread.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(80));
                execute_sequence_worker(&am, &st, &seq_name);
            });
        }
    }

    fn render_full_config(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("💾 Save Configurations").clicked() {
                    // Propagate edits to ActionManager
                    for seq in &self.sequences {
                        self.action_manager.update_sequence(seq.clone());
                    }
                    for act in &self.actions {
                        self.action_manager.update_action(act.clone());
                    }
                    if save_all_configs(&self.actions, &self.sequences, &self.shortcuts) {
                        self.notify("Configurations saved successfully!");
                    } else {
                        self.notify("Error saving configuration files");
                    }
                }
                if ui.button("🔄 Reload Config").clicked() {
                    if self.action_manager.reload_from_yaml("config/actions.yaml").is_ok() {
                        self.sequences = self.action_manager.list_sequences();
                        self.actions = self.action_manager.list_actions();
                        self.shortcuts = load_shortcuts_from_config("config/shortcuts.yaml");
                        self.notify("Reloaded configs from disk");
                    } else {
                        self.notify("Error reloading config files");
                    }
                }
                ui.add_space(20.0);
                if !self.notification_msg.is_empty() && std::time::Instant::now() < self.notification_expire {
                    ui.label(egui::RichText::new(&self.notification_msg).color(egui::Color32::from_rgb(100, 220, 120)).strong());
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LWSC2 BOT CONFIGURATION");
            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, ConfigTab::Dashboard, "Dashboard");
                ui.selectable_value(&mut self.current_tab, ConfigTab::Actions, "Actions (Touches/Boutons)");
                ui.selectable_value(&mut self.current_tab, ConfigTab::Sequences, "Sequences (Macros)");
                ui.selectable_value(&mut self.current_tab, ConfigTab::Shortcuts, "Raccourcis Globaux");
            });
            ui.separator();

            match self.current_tab {
                ConfigTab::Dashboard => {
                    let win_info = self.window_tracker.get_window_info();
                    let current_state = self.state_thread.get_current_state();
                    let current_root = self.state_thread.get_current_root_state();

                    ui.group(|ui| {
                        ui.label(format!("Target Window: {} (Focused: {})", win_info.title, win_info.is_focused));
                        ui.label(format!("Game State: {:?}", current_state));
                        ui.label(format!("Root State: {:?}", current_root));
                    });
                }
                ConfigTab::Actions => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Actions Automatisées & Raccourcis Clavier");
                        let mut action_to_remove = None;
                        let mut notify_msg = None;
                        let current_win = self.window_tracker.get_window_info();
                        let current_st = self.state_thread.get_current_state();

                        for (idx, action) in self.actions.iter_mut().enumerate() {
                            let shortcut_suffix = action
                                .shortcut
                                .as_deref()
                                .filter(|s| !s.trim().is_empty())
                                .map(|s| format!("  [{}]", s.trim()))
                                .unwrap_or_default();

                            let label = if !action.display_name.is_empty() {
                                format!("{}. {} ({}){}", idx + 1, action.display_name, action.name, shortcut_suffix)
                            } else {
                                format!("{}. {}{}", idx + 1, action.name, shortcut_suffix)
                            };

                            egui::CollapsingHeader::new(label)
                                .id_source(format!("action_header_idx_{}", idx))
                                .default_open(false)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut action.enabled, "Enabled (Actif)");
                                        ui.add_space(20.0);
                                        if ui.button("🗑 Supprimer l'action").clicked() {
                                            action_to_remove = Some(idx);
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Identifiant YAML (Clé) :");
                                        ui.add(egui::TextEdit::singleline(&mut action.name).desired_width(180.0));
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Nom affiché (Display Name) :");
                                        ui.add(egui::TextEdit::singleline(&mut action.display_name).desired_width(220.0));
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Raccourci Clavier (Shortcut) :");
                                        let mut sc = action.shortcut.clone().unwrap_or_default();
                                        if ui.add(egui::TextEdit::singleline(&mut sc).desired_width(120.0)).changed() {
                                            action.shortcut = if sc.trim().is_empty() { None } else { Some(sc.trim().to_string()) };
                                        }
                                        ui.label("(ex: ctrl+1, ctrl+b, alt+a)");
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Bouton lié (Button ID) :");
                                        let mut btn = action.button.clone().unwrap_or_default();
                                        if ui.add(egui::TextEdit::singleline(&mut btn).desired_width(180.0)).changed() {
                                            action.button = if btn.trim().is_empty() { None } else { Some(btn.trim().to_string()) };
                                        }
                                        ui.label("(Hérite auto du ROI et du template)");
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Description :");
                                        ui.add(egui::TextEdit::singleline(&mut action.description).desired_width(300.0));
                                    });

                                    ui.separator();
                                    
                                    ui.horizontal(|ui| {
                                        ui.label("Type d'Action :");
                                        egui::ComboBox::from_id_source(format!("type_idx_{}", idx))
                                            .selected_text(action.action_type.as_str())
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::ClickTemplate, "Click Template");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::ClickCoords, "Click Coords");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::ClickRoi, "Click ROI");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::KeyPress, "Key Press");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::DragDrop, "Drag & Drop");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::Custom, "Custom");
                                            });
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Cooldown (s) :");
                                        ui.add(egui::DragValue::new(&mut action.cooldown_s).speed(0.1).clamp_range(0.0..=600.0));
                                        ui.label("Priorité :");
                                        ui.add(egui::DragValue::new(&mut action.priority).speed(1).clamp_range(1..=10));
                                    });
                                    
                                    if action.action_type == crate::core::action::ActionType::DragDrop {
                                        ui.horizontal(|ui| {
                                            ui.label("Drag Start (X, Y):");
                                            let mut start_x = action.drag_start.map(|c| c.0).unwrap_or(0.5);
                                            let mut start_y = action.drag_start.map(|c| c.1).unwrap_or(0.5);
                                            ui.add(egui::DragValue::new(&mut start_x).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut start_y).speed(0.01).clamp_range(0.0..=1.0));
                                            action.drag_start = Some((start_x, start_y));
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Drag End (X, Y):");
                                            let mut end_x = action.drag_end.map(|c| c.0).unwrap_or(0.5);
                                            let mut end_y = action.drag_end.map(|c| c.1).unwrap_or(0.5);
                                            ui.add(egui::DragValue::new(&mut end_x).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut end_y).speed(0.01).clamp_range(0.0..=1.0));
                                            action.drag_end = Some((end_x, end_y));
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Duration (ms):");
                                            ui.add(egui::DragValue::new(&mut action.drag_duration_ms).speed(10.0).clamp_range(100..=10000));
                                        });
                                    }

                                    if action.action_type == crate::core::action::ActionType::ClickCoords {
                                        ui.horizontal(|ui| {
                                            ui.label("Coordonnées (X, Y) :");
                                            let mut cx = action.coords.map(|c| c.0).unwrap_or(0.5);
                                            let mut cy = action.coords.map(|c| c.1).unwrap_or(0.5);
                                            ui.add(egui::DragValue::new(&mut cx).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut cy).speed(0.01).clamp_range(0.0..=1.0));
                                            action.coords = Some((cx, cy));
                                        });
                                    }

                                    if action.action_type == crate::core::action::ActionType::ClickRoi {
                                        ui.horizontal(|ui| {
                                            ui.label("ROI (xmin, xmax, ymin, ymax) :");
                                            let mut roi = action.roi.unwrap_or_else(|| crate::core::state::NormalizedROI::new(0.0, 1.0, 0.0, 1.0));
                                            ui.add(egui::DragValue::new(&mut roi.xmin).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut roi.xmax).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut roi.ymin).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut roi.ymax).speed(0.01).clamp_range(0.0..=1.0));
                                            action.roi = Some(roi);
                                        });
                                    }

                                    if action.action_type == crate::core::action::ActionType::KeyPress {
                                        ui.horizontal(|ui| {
                                            ui.label("Touche Clavier (Key Name) :");
                                            let mut key = action.key_name.clone().unwrap_or_default();
                                            if ui.add(egui::TextEdit::singleline(&mut key).hint_text("ex: Escape, Return, Space").desired_width(140.0)).changed() {
                                                action.key_name = if key.trim().is_empty() { None } else { Some(key.trim().to_string()) };
                                            }
                                        });
                                    }
                                    
                                    if action.action_type == crate::core::action::ActionType::Custom {
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("⚙ Configuration du Script Python").strong());
                                            
                                            // Search for python scripts in scripts/ folder
                                            let mut available_scripts = Vec::new();
                                            if let Ok(entries) = std::fs::read_dir("scripts") {
                                                for entry in entries.flatten() {
                                                    let path = entry.path();
                                                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                                        if ext.eq_ignore_ascii_case("py") {
                                                            available_scripts.push(path.to_string_lossy().to_string());
                                                        }
                                                    }
                                                }
                                            }
                                            available_scripts.sort();

                                            ui.horizontal(|ui| {
                                                ui.label("Script Python :");
                                                let mut script_val = action.script.clone().unwrap_or_default();
                                                if ui.add(egui::TextEdit::singleline(&mut script_val).hint_text("ex: scripts/search_gold_mine.py").desired_width(220.0)).changed() {
                                                    action.script = if script_val.trim().is_empty() { None } else { Some(script_val.trim().to_string()) };
                                                }

                                                if !available_scripts.is_empty() {
                                                    egui::ComboBox::from_id_source(format!("script_combo_{}", idx))
                                                        .selected_text(action.script.as_deref().unwrap_or("Choisir un script..."))
                                                        .show_ui(ui, |ui| {
                                                            for s_path in &available_scripts {
                                                                let is_sel = action.script.as_deref() == Some(s_path.as_str());
                                                                if ui.selectable_label(is_sel, s_path).clicked() {
                                                                    action.script = Some(s_path.clone());
                                                                }
                                                            }
                                                        });
                                                }
                                            });

                                            ui.horizontal(|ui| {
                                                ui.label("Arguments CLI :");
                                                let mut args_str = action.args.join(" ");
                                                if ui.add(egui::TextEdit::singleline(&mut args_str).hint_text("ex: --debug --speed 2").desired_width(240.0)).changed() {
                                                    action.args = args_str.split_whitespace().map(|s| s.to_string()).collect();
                                                }
                                                ui.label("(séparés par des espaces)");
                                            });

                                            ui.horizontal(|ui| {
                                                if let Some(ref scr) = action.script {
                                                    if ui.button("▶ Tester l'exécution du script").clicked() {
                                                        crate::core::action::run_python_script(
                                                            scr,
                                                            &action.args,
                                                            current_win.window_id,
                                                            Some(current_st.name()),
                                                            Some("last_screenshot.png"),
                                                        );
                                                        notify_msg = Some(format!("Script '{}' exécuté en arrière-plan !", scr));
                                                    }
                                                }
                                            });
                                        });
                                    }
                                    
                                    ui.horizontal(|ui| {
                                        ui.label("Min Confidence :");
                                        ui.add(egui::DragValue::new(&mut action.min_confidence).speed(0.01).clamp_range(0.1..=1.0));
                                        ui.checkbox(&mut action.save_cursor, "Save Cursor");
                                    });
                                });
                        }

                        if let Some(msg) = notify_msg {
                            self.notify(msg);
                        }

                        if let Some(idx) = action_to_remove {
                            self.actions.remove(idx);
                        }

                        ui.add_space(10.0);
                        if ui.button("➕ Ajouter une Action").clicked() {
                            self.actions.push(crate::core::action::ActionDefinition::new(
                                &format!("new_action_{}", self.actions.len() + 1),
                                "Nouvelle Action",
                            ));
                        }
                    });
                }
                ConfigTab::Sequences => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Séquences Automatisées (Macros)");
                        let mut seq_to_remove = None;

                        for (s_idx, seq) in self.sequences.iter_mut().enumerate() {
                            let shortcut_suffix = seq
                                .shortcut
                                .as_deref()
                                .filter(|s| !s.trim().is_empty())
                                .map(|s| format!("  [{}]", s.trim()))
                                .unwrap_or_default();
                            let label = format!("{}. {}{}", s_idx + 1, seq.name, shortcut_suffix);

                            egui::CollapsingHeader::new(label)
                                .id_source(format!("seq_header_idx_{}", s_idx))
                                .default_open(false)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut seq.enabled, "Enabled (Active)");
                                        ui.checkbox(&mut seq.repeat, "🔁 Loop (Répéter en boucle)");
                                        ui.add_space(20.0);
                                        if ui.button("🗑 Supprimer la séquence").clicked() {
                                            seq_to_remove = Some(s_idx);
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Identifiant (Clé YAML) :");
                                        ui.add(egui::TextEdit::singleline(&mut seq.name).desired_width(180.0));
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Raccourci Clavier (Shortcut) :");
                                        let mut sc = seq.shortcut.clone().unwrap_or_default();
                                        if ui.add(egui::TextEdit::singleline(&mut sc).desired_width(120.0)).changed() {
                                            seq.shortcut = if sc.trim().is_empty() { None } else { Some(sc.trim().to_string()) };
                                        }
                                        ui.label("(ex: shift+m, shift+g, maj+l)");
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Description :");
                                        ui.add(egui::TextEdit::singleline(&mut seq.description).desired_width(280.0));
                                    });

                                    ui.separator();
                                    
                                    let mut step_to_remove = None;
                                    let available_actions: Vec<String> = self.actions.iter().map(|a| a.name.clone()).collect();
                                    
                                    ui.label("Étapes de la séquence (en cas de timeout, passe automatiquement à la suivante) :");
                                    for (step_idx, step) in seq.steps.iter_mut().enumerate() {
                                        ui.group(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(format!("Étape {}: ", step_idx + 1));
                                                let display_label = self.actions.iter()
                                                    .find(|a| a.name.eq_ignore_ascii_case(&step.action))
                                                    .map(|a| if !a.display_name.is_empty() { format!("{} ({})", a.display_name, a.name) } else { a.name.clone() })
                                                    .unwrap_or_else(|| step.action.clone());

                                                egui::ComboBox::from_id_source(format!("seq_step_box_{}_{}", s_idx, step_idx))
                                                    .selected_text(display_label)
                                                    .width(240.0)
                                                    .show_ui(ui, |ui| {
                                                        for a in &self.actions {
                                                            let item_label = if !a.display_name.is_empty() {
                                                                format!("{} ({})", a.display_name, a.name)
                                                            } else {
                                                                a.name.clone()
                                                            };
                                                            ui.selectable_value(&mut step.action, a.name.clone(), item_label);
                                                        }
                                                    });
                                                ui.label("Timeout (s):");
                                                ui.add(egui::DragValue::new(&mut step.timeout_s).speed(0.1).clamp_range(0.1..=100.0));
                                                if ui.button("🗑").on_hover_text("Supprimer cette étape").clicked() {
                                                    step_to_remove = Some(step_idx);
                                                }
                                            });
                                        });
                                    }
                                    if let Some(idx) = step_to_remove {
                                        seq.steps.remove(idx);
                                    }
                                    if ui.button("➕ Ajouter une Étape").clicked() {
                                        let default_action = available_actions.first().cloned().unwrap_or_else(|| "unknown".to_string());
                                        seq.steps.push(crate::core::action::SequenceStep {
                                            action: default_action,
                                            timeout_s: 5.0,
                                        });
                                    }
                                    
                                    ui.separator();
                                    ui.heading("Plannings (Schedules)");
                                    if seq.schedules.is_none() {
                                        if ui.button("➕ Ajouter des Plannings").clicked() {
                                            seq.schedules = Some(crate::core::action::SequenceSchedules {
                                                every_day: None, monday: None, tuesday: None, wednesday: None,
                                                thursday: None, friday: None, saturday: None, sunday: None,
                                            });
                                        }
                                    } else {
                                        if ui.button("🗑 Supprimer tous les plannings").clicked() {
                                            seq.schedules = None;
                                        } else if let Some(schedules) = &mut seq.schedules {
                                            let days = [
                                                ("every_day", &mut schedules.every_day),
                                                ("monday", &mut schedules.monday),
                                                ("tuesday", &mut schedules.tuesday),
                                                ("wednesday", &mut schedules.wednesday),
                                                ("thursday", &mut schedules.thursday),
                                                ("friday", &mut schedules.friday),
                                                ("saturday", &mut schedules.saturday),
                                                ("sunday", &mut schedules.sunday),
                                            ];
                                            for (day_name, day_opt) in days {
                                                ui.horizontal(|ui| {
                                                    ui.label(format!("{}: ", day_name));
                                                    if day_opt.is_none() {
                                                        if ui.button("+").clicked() {
                                                            *day_opt = Some(vec!["12:00".to_string()]);
                                                        }
                                                    } else if let Some(day_vec) = day_opt {
                                                        let mut to_remove = None;
                                                        for (i, time_str) in day_vec.iter_mut().enumerate() {
                                                            ui.add(egui::TextEdit::singleline(time_str).desired_width(50.0));
                                                            if ui.button("x").clicked() {
                                                                to_remove = Some(i);
                                                            }
                                                        }
                                                        if let Some(idx) = to_remove {
                                                            day_vec.remove(idx);
                                                        }
                                                        if ui.button("+").clicked() {
                                                            day_vec.push("12:00".to_string());
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                });
                        }

                        if let Some(idx) = seq_to_remove {
                            self.sequences.remove(idx);
                        }

                        ui.add_space(10.0);
                        if ui.button("➕ Ajouter une Séquence").clicked() {
                            self.sequences.push(crate::core::action::SequenceDefinition {
                                name: format!("new_sequence_{}", self.sequences.len() + 1),
                                description: "Nouvelle séquence".to_string(),
                                enabled: true,
                                repeat: false,
                                shortcut: None,
                                schedules: None,
                                steps: Vec::new(),
                            });
                        }
                    });
                }
                ConfigTab::Shortcuts => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Raccourcis Clavier Globaux (Global Hotkeys)");
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Pause / Reprise (Toggle Pause) :");
                                ui.add(egui::TextEdit::singleline(&mut self.shortcuts.toggle_pause).desired_width(120.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Ouvrir Configuration (Open Config) :");
                                ui.add(egui::TextEdit::singleline(&mut self.shortcuts.open_config).desired_width(120.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Lanceur Rapide (Quick Launcher) :");
                                ui.add(egui::TextEdit::singleline(&mut self.shortcuts.quick_launcher).desired_width(120.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Détection Manuelle (Force Detect) :");
                                ui.add(egui::TextEdit::singleline(&mut self.shortcuts.force_detect).desired_width(120.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Afficher Aide (Show Help) :");
                                ui.add(egui::TextEdit::singleline(&mut self.shortcuts.show_help).desired_width(120.0));
                            });
                        });
                    });
                }
            }
        });
    }
}

fn save_all_configs(
    actions: &[crate::core::action::ActionDefinition],
    sequences: &[crate::core::action::SequenceDefinition],
    shortcuts: &ShortcutsConfig,
) -> bool {
    // 1. Save actions
    let mut actions_map = std::collections::BTreeMap::new();
    for a in actions {
        let mut entry = serde_yaml::Mapping::new();
        if !a.display_name.is_empty() && a.display_name != a.name {
            entry.insert(serde_yaml::Value::String("display_name".to_string()), serde_yaml::Value::String(a.display_name.clone()));
        }
        if !a.description.is_empty() {
            entry.insert(serde_yaml::Value::String("description".to_string()), serde_yaml::Value::String(a.description.clone()));
        }
        if !a.enabled {
            entry.insert(serde_yaml::Value::String("enabled".to_string()), serde_yaml::Value::Bool(false));
        }
        if let Some(ref btn) = a.button {
            entry.insert(serde_yaml::Value::String("button".to_string()), serde_yaml::Value::String(btn.clone()));
        }
        if let Some(st) = a.state {
            entry.insert(serde_yaml::Value::String("state".to_string()), serde_yaml::Value::String(st.name().to_string()));
        }
        if !a.parent_states.is_empty() {
            let p_seq = a.parent_states.iter().map(|s| serde_yaml::Value::String(s.name().to_string())).collect();
            entry.insert(serde_yaml::Value::String("parent_states".to_string()), serde_yaml::Value::Sequence(p_seq));
        }
        if a.action_type != crate::core::action::ActionType::ClickTemplate {
            entry.insert(serde_yaml::Value::String("action_type".to_string()), serde_yaml::Value::String(a.action_type.as_str().to_string()));
        }
        if let Some((sx, sy)) = a.drag_start {
            let d_seq = vec![
                serde_yaml::Value::Number(serde_yaml::Number::from(sx as f64)),
                serde_yaml::Value::Number(serde_yaml::Number::from(sy as f64)),
            ];
            entry.insert(serde_yaml::Value::String("drag_start".to_string()), serde_yaml::Value::Sequence(d_seq));
        }
        if let Some((ex, ey)) = a.drag_end {
            let d_seq = vec![
                serde_yaml::Value::Number(serde_yaml::Number::from(ex as f64)),
                serde_yaml::Value::Number(serde_yaml::Number::from(ey as f64)),
            ];
            entry.insert(serde_yaml::Value::String("drag_end".to_string()), serde_yaml::Value::Sequence(d_seq));
        }
        if a.drag_duration_ms != 300 {
            entry.insert(serde_yaml::Value::String("drag_duration_ms".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(a.drag_duration_ms)));
        }
        if let Some((cx, cy)) = a.coords {
            let c_seq = vec![
                serde_yaml::Value::Number(serde_yaml::Number::from(cx as f64)),
                serde_yaml::Value::Number(serde_yaml::Number::from(cy as f64)),
            ];
            entry.insert(serde_yaml::Value::String("coords".to_string()), serde_yaml::Value::Sequence(c_seq));
        }
        if let Some(r) = a.roi {
            let mut roi_map = serde_yaml::Mapping::new();
            roi_map.insert(serde_yaml::Value::String("xmin".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(r.xmin as f64)));
            roi_map.insert(serde_yaml::Value::String("xmax".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(r.xmax as f64)));
            roi_map.insert(serde_yaml::Value::String("ymin".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(r.ymin as f64)));
            roi_map.insert(serde_yaml::Value::String("ymax".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(r.ymax as f64)));
            entry.insert(serde_yaml::Value::String("roi".to_string()), serde_yaml::Value::Mapping(roi_map));
        }
        if let Some(ref kn) = a.key_name {
            entry.insert(serde_yaml::Value::String("key_name".to_string()), serde_yaml::Value::String(kn.clone()));
        }
        if a.save_cursor {
            entry.insert(serde_yaml::Value::String("save_cursor".to_string()), serde_yaml::Value::Bool(true));
        }
        if let Some(ref tmpl) = a.template {
            entry.insert(serde_yaml::Value::String("template".to_string()), serde_yaml::Value::String(tmpl.clone()));
        }
        if let Some(ref ctmpl) = a.click_template {
            entry.insert(serde_yaml::Value::String("click_template".to_string()), serde_yaml::Value::String(ctmpl.clone()));
        }
        if (a.cooldown_s - 1.0).abs() > f32::EPSILON {
            entry.insert(serde_yaml::Value::String("cooldown_s".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(a.cooldown_s as f64)));
        }
        if a.priority != 10 {
            entry.insert(serde_yaml::Value::String("priority".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(a.priority)));
        }
        if let Some(ref sc) = a.shortcut {
            entry.insert(serde_yaml::Value::String("shortcut".to_string()), serde_yaml::Value::String(sc.clone()));
        }
        if let Some(ref scr) = a.script {
            entry.insert(serde_yaml::Value::String("script".to_string()), serde_yaml::Value::String(scr.clone()));
        }
        if !a.args.is_empty() {
            let args_seq = a.args.iter().map(|arg| serde_yaml::Value::String(arg.clone())).collect();
            entry.insert(serde_yaml::Value::String("args".to_string()), serde_yaml::Value::Sequence(args_seq));
        }
        actions_map.insert(a.name.clone(), serde_yaml::Value::Mapping(entry));
    }

    if let Ok(act_str) = serde_yaml::to_string(&actions_map) {
        let _ = std::fs::write("config/actions.yaml", act_str);
    }

    // 2. Save sequences
    let mut seqs_map = std::collections::BTreeMap::new();
    for s in sequences {
        let mut entry = serde_yaml::Mapping::new();
        if !s.description.is_empty() {
            entry.insert(serde_yaml::Value::String("description".to_string()), serde_yaml::Value::String(s.description.clone()));
        }
        if !s.enabled {
            entry.insert(serde_yaml::Value::String("enabled".to_string()), serde_yaml::Value::Bool(false));
        }
        if s.repeat {
            entry.insert(serde_yaml::Value::String("loop".to_string()), serde_yaml::Value::Bool(true));
        }
        if let Some(ref sc) = s.shortcut {
            entry.insert(serde_yaml::Value::String("shortcut".to_string()), serde_yaml::Value::String(sc.clone()));
        }
        let steps_seq: Vec<serde_yaml::Value> = s.steps.iter().map(|step| {
            let mut st_map = serde_yaml::Mapping::new();
            st_map.insert(serde_yaml::Value::String("action".to_string()), serde_yaml::Value::String(step.action.clone()));
            if (step.timeout_s - 5.0).abs() > f32::EPSILON {
                st_map.insert(serde_yaml::Value::String("timeout_s".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(step.timeout_s as f64)));
            }
            serde_yaml::Value::Mapping(st_map)
        }).collect();
        entry.insert(serde_yaml::Value::String("steps".to_string()), serde_yaml::Value::Sequence(steps_seq));

        if let Some(ref scheds) = s.schedules {
            let mut sched_map = serde_yaml::Mapping::new();
            let days = [
                ("every_day", &scheds.every_day),
                ("monday", &scheds.monday),
                ("tuesday", &scheds.tuesday),
                ("wednesday", &scheds.wednesday),
                ("thursday", &scheds.thursday),
                ("friday", &scheds.friday),
                ("saturday", &scheds.saturday),
                ("sunday", &scheds.sunday),
            ];
            for (day_name, day_opt) in days {
                if let Some(day_vec) = day_opt {
                    if !day_vec.is_empty() {
                        let seq = day_vec.iter().map(|t| serde_yaml::Value::String(t.clone())).collect();
                        sched_map.insert(serde_yaml::Value::String(day_name.to_string()), serde_yaml::Value::Sequence(seq));
                    }
                }
            }
            if !sched_map.is_empty() {
                entry.insert(serde_yaml::Value::String("schedules".to_string()), serde_yaml::Value::Mapping(sched_map));
            }
        }
        seqs_map.insert(s.name.clone(), serde_yaml::Value::Mapping(entry));
    }

    if let Ok(seq_str) = serde_yaml::to_string(&seqs_map) {
        let _ = std::fs::write("config/sequences.yaml", seq_str);
    }

    // 3. Save shortcuts
    if let Ok(short_str) = serde_yaml::to_string(shortcuts) {
        let _ = std::fs::write("config/shortcuts.yaml", short_str);
    }

    true
}
