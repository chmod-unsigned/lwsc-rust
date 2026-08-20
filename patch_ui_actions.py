import sys

with open("src/ui/config_window.rs", "r") as f:
    content = f.read()

# Add actions to Lwsc2ConfigApp
old_struct = """struct Lwsc2ConfigApp {
    action_manager: Arc<ActionManager>,
    state_thread: StateDetectorThread,
    window_tracker: WindowTracker,
    notification_msg: String,
    notification_expire: std::time::Instant,
    current_tab: ConfigTab,
    is_visible: bool,
    sequences: Vec<SequenceDefinition>,
}"""
new_struct = """use crate::core::action::ActionDefinition;

struct Lwsc2ConfigApp {
    action_manager: Arc<ActionManager>,
    state_thread: StateDetectorThread,
    window_tracker: WindowTracker,
    notification_msg: String,
    notification_expire: std::time::Instant,
    current_tab: ConfigTab,
    is_visible: bool,
    sequences: Vec<SequenceDefinition>,
    actions: Vec<ActionDefinition>,
}"""
content = content.replace(old_struct, new_struct)

old_new = """        let sequences = action_manager.list_sequences();
        Self {
            action_manager,
            state_thread,
            window_tracker,
            notification_msg: String::new(),
            notification_expire: std::time::Instant::now(),
            current_tab: ConfigTab::Dashboard,
            is_visible: true,
            sequences,
        }"""
new_new = """        let sequences = action_manager.list_sequences();
        let actions = action_manager.list_actions();
        Self {
            action_manager,
            state_thread,
            window_tracker,
            notification_msg: String::new(),
            notification_expire: std::time::Instant::now(),
            current_tab: ConfigTab::Dashboard,
            is_visible: true,
            sequences,
            actions,
        }"""
content = content.replace(old_new, new_new)


with open("src/ui/config_window.rs", "w") as f:
    f.write(content)
