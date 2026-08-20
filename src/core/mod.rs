pub mod state;
pub mod button;
pub mod state_graph;
pub mod detector;
pub mod state_thread;
pub mod action;

pub use state::{
    GameState, StateType, StateDefinition, NormalizedROI, STATE_DEFINITIONS, BUTTON_DEFINITIONS,
    SHORTCUTS_CONFIG, ShortcutsConfig, load_shortcuts_from_config,
    get_state_definition, load_state_definitions, load_actions_from_config, load_sequences_from_config,
    load_state_definitions_or_default,
};
pub use button::{ButtonDefinition, ButtonDetection, load_buttons_from_config, select_click_match};
pub use state_graph::{StateGraph, TransitionEdge};
pub use action::{
    ActionDefinition, ActionManager, ActionType, ActionExecutionResult, ActionsConfigFile,
    SequenceDefinition, SequenceStep, SequenceSchedules,
};
pub use detector::{StateDetector, DetectionResult};
pub use state_thread::StateDetectorThread;
