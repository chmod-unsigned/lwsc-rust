//! Dedicated independent thread for Game State Detection in Rust.

use std::sync::{mpsc, Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::core::state::GameState;
use crate::core::detector::{StateDetector, DetectionResult};
use crate::vision::screen::ScreenCapturer;
use crate::vision::window_tracker::WindowTracker;

pub type StateCallback = Box<dyn Fn(GameState, GameState, &str) + Send + Sync + 'static>;

#[derive(Debug)]
pub enum DetectionRequest {
    Activity {
        source: String,
        settle_delay: Duration,
        resp_tx: Option<mpsc::Sender<DetectionResult>>,
    },
    Periodic,
}

#[derive(Clone)]
pub struct StateDetectorThread {
    req_tx: mpsc::Sender<DetectionRequest>,
    current_state: Arc<RwLock<GameState>>,
    current_root_state: Arc<RwLock<Option<GameState>>>,
    current_result: Arc<RwLock<Option<DetectionResult>>>,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl StateDetectorThread {
    pub fn start(
        window_tracker: WindowTracker,
        periodic_interval: Duration,
        asset_root: &str,
        save_last_screenshot: bool,
        initial_root: Option<GameState>,
        action_manager: Option<Arc<crate::core::action::ActionManager>>,
        on_transition: Option<StateCallback>,
    ) -> (Self, JoinHandle<()>) {
        let (req_tx, req_rx) = mpsc::channel::<DetectionRequest>();
        let current_state = Arc::new(RwLock::new(GameState::Unknown));
        let current_root_state = Arc::new(RwLock::new(initial_root));
        let current_result = Arc::new(RwLock::new(None));
        let running = Arc::new(AtomicBool::new(true));
        let paused = Arc::new(AtomicBool::new(false));

        let current_state_th = Arc::clone(&current_state);
        let current_root_state_th = Arc::clone(&current_root_state);
        let current_result_th = Arc::clone(&current_result);
        let running_th = Arc::clone(&running);
        let paused_th = Arc::clone(&paused);
        let asset_root_str = asset_root.to_string();
        let am_th = action_manager;

        let handle = thread::Builder::new()
            .name("StateDetectorThread".to_string())
            .spawn(move || {
                let capturer = ScreenCapturer::new();
                let mut detector = StateDetector::new(&asset_root_str);

                // Initial detection pass (only if window is found and focused)
                let initial_info = window_tracker.get_window_info();
                if initial_info.is_found && initial_info.is_focused {
                    let last_root = *current_root_state_th.read().unwrap();
                    let initial_res = perform_detection(
                        &window_tracker,
                        &capturer,
                        &mut detector,
                        last_root,
                        save_last_screenshot,
                        am_th.as_deref(),
                        paused_th.load(Ordering::Relaxed),
                    );
                    *current_state_th.write().unwrap() = initial_res.state;
                    if let Some(root) = initial_res.root_state {
                        *current_root_state_th.write().unwrap() = Some(root);
                    }
                    *current_result_th.write().unwrap() = Some(initial_res);
                }

                while running_th.load(Ordering::Relaxed) {
                    let req = match req_rx.recv_timeout(periodic_interval) {
                        Ok(r) => r,
                        Err(mpsc::RecvTimeoutError::Timeout) => DetectionRequest::Periodic,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };

                    let (mut source, mut resp_tx) = match req {
                        DetectionRequest::Activity { source, settle_delay, resp_tx } => {
                            if !settle_delay.is_zero() {
                                thread::sleep(settle_delay);
                            }
                            (source, resp_tx)
                        }
                        DetectionRequest::Periodic => ("periodic".to_string(), None),
                    };

                    // Drain any pending notifications queued during sleep/processing
                    while let Ok(next_req) = req_rx.try_recv() {
                        if let DetectionRequest::Activity { source: s, resp_tx: tx, .. } = next_req {
                            source = s;
                            if tx.is_some() {
                                resp_tx = tx;
                            }
                        }
                    }

                    let win_info = window_tracker.get_window_info();
                    if !win_info.is_found || !win_info.is_focused {
                        if let Some(tx) = resp_tx {
                            let dummy = DetectionResult {
                                state: GameState::Unknown,
                                state_type: crate::core::state::StateType::Special,
                                root_state: None,
                                modal_state: None,
                                visible_buttons: Vec::new(),
                                confidence: 0.0,
                                matched_template: None,
                                match_box: None,
                                match_center: None,
                                display_name: if !win_info.is_found {
                                    "Game Window Not Found".to_string()
                                } else {
                                    "Game Window Not Focused".to_string()
                                },
                            };
                            let _ = tx.send(dummy);
                        }
                        continue;
                    }

                    if let Some(am) = am_th.as_ref() {
                        am.evaluate_schedules();
                        
                        if !am.has_active_sequence() {
                            if let Some(next_seq) = am.pop_sequence_queue() {
                                am.trigger_sequence(&next_seq);
                            }
                        }
                    }

                    let last_root = *current_root_state_th.read().unwrap();
                    let fresh_res = perform_detection(
                        &window_tracker,
                        &capturer,
                        &mut detector,
                        last_root,
                        save_last_screenshot,
                        am_th.as_deref(),
                        paused_th.load(Ordering::Relaxed),
                    );
                    let old_state;
                    let mut state_changed = false;

                    {
                        let mut state_lock = current_state_th.write().unwrap();
                        let mut res_lock = current_result_th.write().unwrap();
                        old_state = *state_lock;
                        if old_state != fresh_res.state {
                            state_changed = true;
                            *state_lock = fresh_res.state;
                        }
                        if let Some(root) = fresh_res.root_state {
                            *current_root_state_th.write().unwrap() = Some(root);
                        }
                        *res_lock = Some(fresh_res.clone());
                    }

                    if state_changed {
                        if let Some(ref cb) = on_transition {
                            cb(old_state, fresh_res.state, &source);
                        }
                    }

                    if let Some(tx) = resp_tx {
                        let _ = tx.send(fresh_res);
                    }
                }
            })
            .expect("Failed to spawn StateDetectorThread");

        (
            Self {
                req_tx,
                current_state,
                current_root_state,
                current_result,
                running,
                paused,
            },
            handle,
        )
    }

    pub fn trigger_on_activity(
        &self,
        source: &str,
        settle_delay: Duration,
        wait_result: bool,
    ) -> Option<DetectionResult> {
        if wait_result {
            let (tx, rx) = mpsc::channel();
            let req = DetectionRequest::Activity {
                source: source.to_string(),
                settle_delay,
                resp_tx: Some(tx),
            };
            if self.req_tx.send(req).is_ok() {
                return rx.recv_timeout(Duration::from_millis(2000)).ok();
            }
        } else {
            let req = DetectionRequest::Activity {
                source: source.to_string(),
                settle_delay,
                resp_tx: None,
            };
            let _ = self.req_tx.send(req);
        }
        None
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn toggle_pause(&self) -> bool {
        let current = self.is_paused();
        let next = !current;
        self.paused.store(next, Ordering::Relaxed);
        next
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
    }

    pub fn get_current_state(&self) -> GameState {
        *self.current_state.read().unwrap()
    }

    pub fn get_current_root_state(&self) -> Option<GameState> {
        *self.current_root_state.read().unwrap()
    }

    pub fn get_current_result(&self) -> Option<DetectionResult> {
        self.current_result.read().unwrap().clone()
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

fn perform_detection(
    window_tracker: &WindowTracker,
    capturer: &ScreenCapturer,
    detector: &mut StateDetector,
    last_known_root: Option<GameState>,
    save_screenshot: bool,
    action_manager: Option<&crate::core::action::ActionManager>,
    is_paused: bool,
) -> DetectionResult {
    let win = window_tracker.get_window_info();
    if !win.is_found {
        return DetectionResult {
            state: GameState::Unknown,
            state_type: crate::core::state::StateType::Special,
            root_state: None,
            modal_state: None,
            visible_buttons: Vec::new(),
            confidence: 0.0,
            matched_template: None,
            match_box: None,
            match_center: None,
            display_name: "Game Window Not Found".to_string(),
        };
    }

    if !win.is_focused {
        return DetectionResult {
            state: GameState::Unknown,
            state_type: crate::core::state::StateType::Special,
            root_state: None,
            modal_state: None,
            visible_buttons: Vec::new(),
            confidence: 0.0,
            matched_template: None,
            match_box: None,
            match_center: None,
            display_name: "Game Window Not Focused".to_string(),
        };
    }

    let frame = match capturer.capture_region(win.x, win.y, win.width, win.height) {
        Some(f) => f,
        None => {
            return DetectionResult {
                state: GameState::Unknown,
                state_type: crate::core::state::StateType::Special,
                root_state: None,
                modal_state: None,
                visible_buttons: Vec::new(),
                confidence: 0.0,
                matched_template: None,
                match_box: None,
                match_center: None,
                display_name: "Capture Failed".to_string(),
            };
        }
    };

    if save_screenshot {
        let _ = frame.save("last_screenshot.png");
    }

    let mut res = detector.detect_with_context(&frame, last_known_root);

    if is_paused {
        res.display_name = format!("{} [PAUSED]", res.display_name);
    } else if let Some(am) = action_manager {
        let mut action_results = Vec::new();
        if am.has_active_sequence() {
            if let Some(r) = am.evaluate_sequence(res.state, &frame, &mut detector.matcher) {
                action_results.push(r);
            }
        } else {
            // Evaluate and execute any active automated actions (e.g. alliance_help, gift claims)
            action_results = am.evaluate(res.state, &frame, &mut detector.matcher);
        }

        for action_res in action_results {
            if action_res.executed {
                if let Some((cx, cy)) = action_res.click_coords {
                    let screen_x = win.x + cx;
                    let screen_y = win.y + cy;
                    println!(
                        "\n[Auto-Action] '{}' triggered -> Clicking at screen ({}, {}) [{} | save_cursor: {}]",
                        action_res.action_name,
                        screen_x,
                        screen_y,
                        action_res.reason,
                        action_res.save_cursor
                    );
                    crate::io::input::send_x11_click_ex(win.window_id, screen_x as i16, screen_y as i16, action_res.save_cursor);
                } else if let Some(((sx, sy), (ex, ey))) = action_res.drag_coords {
                    let screen_sx = win.x + sx;
                    let screen_sy = win.y + sy;
                    let screen_ex = win.x + ex;
                    let screen_ey = win.y + ey;
                    println!(
                        "\n[Auto-Action] '{}' triggered -> Dragging from ({}, {}) to ({}, {}) [{} | save_cursor: {} | duration: {}ms]",
                        action_res.action_name,
                        screen_sx,
                        screen_sy,
                        screen_ex,
                        screen_ey,
                        action_res.reason,
                        action_res.save_cursor,
                        action_res.drag_duration_ms
                    );
                    let has_templates = !action_res.sweep_templates.is_empty();
                    let templates = action_res.sweep_templates.clone();
                    let mut cb = || -> bool {
                        if !has_templates { return false; }
                        if let Some(frame) = capturer.capture_region(win.x, win.y, win.width, win.height) {
                            for t in &templates {
                                let res = detector.matcher.find_match(&frame, t, 0.7, None);
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
                }
            }
        }
    }

    res
}
