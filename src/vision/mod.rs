pub mod matching;
pub mod window;
pub mod window_tracker;
pub mod screen;

pub use matching::{TemplateMatcher, MatchResult};
pub use window::{WindowManager, WindowInfo};
pub use window_tracker::WindowTracker;
pub use screen::ScreenCapturer;
