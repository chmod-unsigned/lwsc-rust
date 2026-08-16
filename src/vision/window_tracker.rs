//! Independent background thread tracking game window geometry in Rust.

use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::vision::window::{WindowManager, WindowInfo};

pub type GeometryCallback = Box<dyn Fn(&WindowInfo, &WindowInfo) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct WindowTracker {
    info: Arc<RwLock<WindowInfo>>,
    running: Arc<AtomicBool>,
}

impl WindowTracker {
    pub fn start(
        target_title: &str,
        poll_interval: Duration,
        on_change: Option<GeometryCallback>,
    ) -> (Self, JoinHandle<()>) {
        let manager = WindowManager::new(target_title);
        let initial_info = manager.get_window_info();
        let info_arc = Arc::new(RwLock::new(initial_info));
        let running_arc = Arc::new(AtomicBool::new(true));

        let tracker = Self {
            info: Arc::clone(&info_arc),
            running: Arc::clone(&running_arc),
        };

        let info_thread = Arc::clone(&info_arc);
        let running_thread = Arc::clone(&running_arc);

        let handle = thread::Builder::new()
            .name("WindowTrackerThread".to_string())
            .spawn(move || {
                while running_thread.load(Ordering::Relaxed) {
                    let fresh_info = manager.get_window_info();
                    let mut changed = false;
                    let mut old_clone = None;

                    {
                        if let Ok(mut lock) = info_thread.write() {
                            if lock.is_found && fresh_info.is_found {
                                if lock.x != fresh_info.x
                                    || lock.y != fresh_info.y
                                    || lock.width != fresh_info.width
                                    || lock.height != fresh_info.height
                                {
                                    changed = true;
                                    old_clone = Some(lock.clone());
                                }
                            }
                            *lock = fresh_info.clone();
                        }
                    }

                    if changed {
                        if let (Some(ref cb), Some(ref old)) = (&on_change, old_clone) {
                            cb(old, &fresh_info);
                        }
                    }

                    thread::sleep(poll_interval);
                }
            })
            .expect("Failed to spawn WindowTrackerThread");

        (tracker, handle)
    }

    pub fn get_window_info(&self) -> WindowInfo {
        self.info.read().unwrap().clone()
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
