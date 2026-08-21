//! Main GameBot Engine in Rust for Last War: Survival.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use colored::Colorize;

use crate::core::action::ActionManager;
use crate::core::state::{load_actions_from_config, GameState};
use crate::core::state_graph::StateGraph;
use crate::core::state_thread::StateDetectorThread;
use crate::vision::window::WindowInfo;
use crate::vision::window_tracker::WindowTracker;
use crate::io::input::{HotkeyCallback, InputManager};
use crate::ui::ConfigWindow;

pub struct GameBot {
    pub window_title: String,
    pub window_tracker: WindowTracker,
    pub state_thread: StateDetectorThread,
    pub input_manager: InputManager,
    pub action_manager: Arc<ActionManager>,
    pub graph: StateGraph,
    _handles: Vec<JoinHandle<()>>,
}

impl GameBot {
    pub fn new(
        window_title: &str,
        periodic_interval: Duration,
        asset_root: &str,
        save_last_screenshot: bool,
        enable_global_input_listener: bool,
        initial_root_state: Option<GameState>,
    ) -> Self {
        let mut handles = Vec::new();

        // 1. Start WindowTracker Thread
        let (window_tracker, wt_handle) = WindowTracker::start(
            window_title,
            Duration::from_millis(500),
            Some(Box::new(|old: &WindowInfo, new: &WindowInfo| {
                println!(
                    "{}",
                    format!(
                        "\n[Window Tracker] Geometry updated: Pos: ({}, {}) ➔ ({}, {}) | Dim: {}x{} ➔ {}x{}",
                        old.x, old.y, new.x, new.y, old.width, old.height, new.width, new.height
                    )
                    .yellow()
                );
            })),
        );
        handles.push(wt_handle);

        // 2. Initialize ActionManager
        let initial_actions = load_actions_from_config("config/actions.yaml")
            .or_else(|_| load_actions_from_config("config/states.yaml"))
            .unwrap_or_default();
        let initial_sequences = crate::core::state::load_sequences_from_config("config/sequences.yaml")
            .or_else(|_| crate::core::state::load_sequences_from_config("config/states.yaml"))
            .unwrap_or_default();
        let action_manager = Arc::new(ActionManager::new(initial_actions, initial_sequences));

        // 3. Start StateDetector Thread (triggered on any mouse/keyboard activity & periodically)
        let (state_thread, st_handle) = StateDetectorThread::start(
            window_tracker.clone(),
            periodic_interval,
            asset_root,
            save_last_screenshot,
            initial_root_state,
            Some(Arc::clone(&action_manager)),
            Some(Box::new(|old_state: GameState, new_state: GameState, source: &str| {
                println!(
                    "{}",
                    format!("\n[State Change ({})] {} ➔ {}", source, old_state, new_state).green().bold()
                );
            })),
        );
        handles.push(st_handle);

        // 4. Setup Global Hotkey Handler (Ctrl+O, Ctrl+H, Ctrl+S, Ctrl+P)
        let am_clone = Arc::clone(&action_manager);
        let st_clone_for_hk = state_thread.clone();
        let wt_clone_for_hk = window_tracker.clone();

        let on_hotkey: HotkeyCallback = Arc::new(move |shortcut: &str| {
            match shortcut {
                "open_config" | "ctrl_o" => {
                    println!("{}", "\n[Shortcut] Opening / Toggling Configuration Window...".cyan().bold());
                    ConfigWindow::open_or_toggle(
                        am_clone.clone(),
                        st_clone_for_hk.clone(),
                        wt_clone_for_hk.clone(),
                    );
                }
                "quick_launcher" | "ctrl_x" => {
                    println!("{}", "\n[Shortcut] Opening / Toggling Quick Launcher (Ctrl+X)...".cyan().bold());
                    ConfigWindow::open_quick_launcher(
                        am_clone.clone(),
                        st_clone_for_hk.clone(),
                        wt_clone_for_hk.clone(),
                    );
                }
                "show_help" | "ctrl_h" => {
                    let shortcuts = crate::core::load_shortcuts_from_config("config/states.yaml");
                    println!("{}", "\n=== LWSC2 Global Shortcuts ===".bright_blue().bold());
                    println!("  {:<15} : Toggle Pause / Resume automated bot actions", shortcuts.toggle_pause.green().bold());
                    println!("  {:<15} : Open / Toggle Native Configuration Window", shortcuts.open_config.green().bold());
                    println!("  {:<15} : Open / Toggle Mini Quick Launcher Window", shortcuts.quick_launcher.green().bold());
                    println!("  {:<15} : Force immediate State Detection pass", shortcuts.force_detect.green().bold());
                    println!("  {:<15} : Display this Shortcuts Help", shortcuts.show_help.green().bold());
                    let act_shortcuts = am_clone.get_shortcuts();
                    if !act_shortcuts.is_empty() {
                        println!("{}", "\n=== Action-Specific Shortcuts ===".bright_blue().bold());
                        for (act_name, spec) in act_shortcuts {
                            println!("  {:<15} : Execute action '{}'", spec.green().bold(), act_name.yellow());
                        }
                    }
                    println!("  {:<15} : Gracefully stop and exit", "Ctrl+C".green().bold());
                }
                "force_detect" | "ctrl_s" => {
                    println!("{}", "\n[Shortcut] Forcing immediate state detection...".yellow().bold());
                    st_clone_for_hk.trigger_on_activity("manual_shortcut", Duration::from_millis(50), false);
                }
                "toggle_pause" | "ctrl_p" => {
                    let is_paused = st_clone_for_hk.toggle_pause();
                    if is_paused {
                        println!("{}", "\n[Bot Status] ⏸️  PAUSED - Automated actions suspended".yellow().bold());
                    } else {
                        println!("{}", "\n[Bot Status] ▶️  RESUMED - Automated actions active".green().bold());
                    }
                }
                s if s.starts_with("action:") => {
                    let action_name = s.strip_prefix("action:").unwrap();
                    let win = wt_clone_for_hk.get_window_info();
                    if !win.is_found {
                        println!("{}", "[Action Shortcut] Error: Game window not found".red());
                        return;
                    }
                    if !win.is_focused {
                        println!("{}", format!("[Action Shortcut] Ignored '{}': Game window not focused (bot is paused)", action_name).yellow());
                        return;
                    }

                    println!("{}", format!("\n[Action Shortcut] Triggering action '{}'...", action_name).cyan().bold());

                    let capturer = crate::vision::ScreenCapturer::new();
                    if let Some(frame) = capturer.capture_roi(win.x, win.y, win.width, win.height) {
                        let current_state = st_clone_for_hk.get_current_state();
                        let mut matcher = crate::vision::matching::TemplateMatcher::new(".");
                        if let Some(action_res) = am_clone.execute_single_action(
                            action_name,
                            current_state,
                            &frame,
                            &mut matcher,
                            true, // bypass cooldown when triggered directly via manual shortcut
                            true, // bypass state check on manual trigger
                        ) {
                            if action_res.executed {
                                if let Some((cx, cy)) = action_res.click_coords {
                                    let screen_x = win.x + cx;
                                    let screen_y = win.y + cy;
                                    println!(
                                        "{}",
                                        format!(
                                            "[Action Shortcut] Action '{}' executed -> Clicking at ({}, {}) [save_cursor: {}]",
                                            action_name, screen_x, screen_y, action_res.save_cursor
                                        ).green().bold()
                                    );
                                    crate::io::input::send_x11_click_ex(win.window_id, screen_x as i16, screen_y as i16, action_res.save_cursor);
                                    st_clone_for_hk.trigger_on_activity("manual_action_click", Duration::from_millis(150), false);
                                } else if let Some(((sx, sy), (ex, ey))) = action_res.drag_coords {
                                    let screen_sx = win.x + sx;
                                    let screen_sy = win.y + sy;
                                    let screen_ex = win.x + ex;
                                    let screen_ey = win.y + ey;
                                    println!(
                                        "{}",
                                        format!(
                                            "[Action Shortcut] Action '{}' executed -> Dragging from ({}, {}) to ({}, {}) [save_cursor: {} | duration: {}ms]",
                                            action_name, screen_sx, screen_sy, screen_ex, screen_ey, action_res.save_cursor, action_res.drag_duration_ms
                                        ).green().bold()
                                    );
                                    
                                    let has_templates = !action_res.sweep_templates.is_empty();
                                    let templates = action_res.sweep_templates.clone();
                                    let mut cb = || -> bool {
                                        if !has_templates { return false; }
                                        let capturer = crate::vision::ScreenCapturer::new();
                                        if let Some(frame) = capturer.capture_region(win.x, win.y, win.width, win.height) {
                                            for t in &templates {
                                                let mut matcher = crate::vision::matching::TemplateMatcher::new(".");
                                                let res = matcher.find_match(&frame, t, 0.7, None);
                                                if res.matched {
                                                    println!("\n[Sweep] Found POI '{}' at ({}, {}) with {:.2}% confidence!", t, res.center_x, res.center_y, res.confidence * 100.0);
                                                    return true;
                                                }
                                            }
                                        }
                                        false
                                    };

                                    crate::io::input::send_x11_drag(
                                        win.window_id, 
                                        screen_sx as i16, screen_sy as i16, 
                                        screen_ex as i16, screen_ey as i16, 
                                        action_res.drag_duration_ms, 
                                        action_res.save_cursor,
                                        Some(&mut cb)
                                    );
                                } else if let Some(ref script_path) = action_res.script {
                                    let _ = frame.save("last_screenshot.png");
                                    crate::core::action::run_python_script(
                                        script_path,
                                        &action_res.script_args,
                                        win.window_id,
                                        Some(current_state.name()),
                                        Some("last_screenshot.png"),
                                    );
                                }
                            } else {
                                println!(
                                    "{}",
                                    format!("[Action Shortcut] Action '{}' not executed: {}", action_name, action_res.reason).yellow()
                                );
                            }
                        }
                    }
                }
                s if s.starts_with("sequence:") => {
                    let seq_name = s.strip_prefix("sequence:").unwrap();
                    let win = wt_clone_for_hk.get_window_info();
                    if !win.is_focused {
                        println!("{}", format!("[Sequence Shortcut] Ignored '{}': Game window not focused (bot is paused)", seq_name).yellow());
                        return;
                    }

                    println!("{}", format!("\n[Sequence Shortcut] Triggering sequence '{}'...", seq_name).cyan().bold());
                    if am_clone.trigger_sequence(seq_name) {
                        println!("{}", format!("[Sequence] Sequence '{}' started.", seq_name).green());
                        st_clone_for_hk.trigger_on_activity("sequence_start", Duration::from_millis(50), false);
                    } else {
                        println!("{}", format!("[Sequence] Failed to start sequence '{}' (not found or disabled).", seq_name).red());
                    }
                }
                _ => {}
            }
        });

        // 5. Start InputManager
        let input_manager = InputManager::new(
            state_thread.clone(),
            window_tracker.clone(),
            enable_global_input_listener,
            Some(on_hotkey),
        );

        let graph = StateGraph::new();

        Self {
            window_title: window_title.to_string(),
            window_tracker,
            state_thread,
            input_manager,
            action_manager,
            graph,
            _handles: handles,
        }
    }

    pub fn start(&self) {
        let win_info = self.window_tracker.get_window_info();
        thread::sleep(Duration::from_millis(600)); // allow initial detection pass
        let initial_res = self.state_thread.get_current_result();
        let current_root = self.state_thread.get_current_root_state();

        println!("{}", "====================================================================".bright_blue());
        println!("{}", "  LWSC2 - Last War Survival GameBot Initialized (Rust Edition)".bright_cyan().bold());
        println!("{}", "  [Window Tracker: ACTIVE] [State Detector: ACTIVE] [Input Listener: CONTINUOUS]".bright_blue());
        println!("{}", "====================================================================".bright_blue());
        println!("{}", " [Game Window Geometry & Position]".bold());
        for line in win_info.format_summary().lines() {
            println!("   {}", line);
        }
        println!("{}", "--------------------------------------------------------------------".bright_blue());

        if !win_info.is_found {
            println!("{}", format!("[Warning] Window '{}' not detected yet.", self.window_title).yellow());
            println!("  Waiting for the game window to open to begin capturing...\n");
        }

        println!("{}", " [Startup Game State]".bold());
        if let Some(root) = current_root {
            println!("   Root State   : {}", root.name().cyan().bold());
        } else {
            println!("   Root State   : {}", "UNKNOWN (Evaluating...)".yellow());
        }

        if let Some(res) = initial_res {
            println!("   State Name   : {}", res.state.name().green().bold());
            println!("   Display Name : {}", res.display_name);
            println!("   Layer        : {}", res.state_type.as_str());
            if let Some(root) = res.root_state {
                println!("   Resolved Root: {}", root.name().cyan().bold());
            }
            println!("   Confidence   : {:.2}%", res.confidence * 100.0);
            if let Some(ref tmpl) = res.matched_template {
                println!("   Matched By   : {}", tmpl);
            }
            if let Some(bbox) = res.match_box {
                println!("   Anchor Box   : {:?}", bbox);
            }
            if let Some(center) = res.match_center {
                println!("   Anchor Center: {:?}", center);
            }
        } else {
            println!("   State Name   : UNKNOWN (Evaluating...)");
        }
        println!("{}", "--------------------------------------------------------------------".bright_blue());
        println!(" [Active Shortcuts]");
        println!("   {} : Open/Toggle Configuration Window", "Ctrl+O".green().bold());
        println!("   {} : Open/Toggle Mini Quick Launcher", "Ctrl+X".green().bold());
        println!("   {} : Force immediate State Detection pass", "Ctrl+S".green().bold());
        println!("   {} : Display global shortcuts help", "Ctrl+H".green().bold());
        println!("{}", "====================================================================".bright_blue());
        println!("{}", "Bot is active: Continuous detection on ANY mouse or keyboard event. Press Ctrl+C to exit.\n".bold());

        // Run GUI event loop on main thread (blocks until app exits)
        ConfigWindow::run_on_main_thread(
            Arc::clone(&self.action_manager),
            self.state_thread.clone(),
            self.window_tracker.clone(),
        );
    }

    pub fn stop(&self) {
        self.input_manager.stop();
        self.state_thread.stop();
        self.window_tracker.stop();
    }
}
