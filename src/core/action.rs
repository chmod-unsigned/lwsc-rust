//! Configurable Actions depending on ROI and active/inactive state in Rust.

use std::path::Path;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use image::RgbaImage;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::core::state::{GameState, NormalizedROI};
use crate::vision::matching::TemplateMatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    ClickTemplate,
    ClickCoords,
    ClickRoi,
    KeyPress,
    DragDrop,
    Custom,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::ClickTemplate => "click_template",
            ActionType::ClickCoords => "click_coords",
            ActionType::ClickRoi => "click_roi",
            ActionType::KeyPress => "key_press",
            ActionType::DragDrop => "drag_drop",
            ActionType::Custom => "custom",
        }
    }
}

impl Serialize for ActionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ActionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "click_template" | "clicktemplate" => Ok(ActionType::ClickTemplate),
            "click_coords" | "clickcoords" => Ok(ActionType::ClickCoords),
            "click_roi" | "clickroi" => Ok(ActionType::ClickRoi),
            "key_press" | "keypress" => Ok(ActionType::KeyPress),
            "drag_drop" | "dragdrop" => Ok(ActionType::DragDrop),
            "custom" => Ok(ActionType::Custom),
            _ => Err(serde::de::Error::custom(format!("unknown action type: {}", s))),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_cooldown() -> f32 {
    1.0
}

fn default_min_confidence() -> f32 {
    0.80
}

fn default_priority() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub name: String,
    
    #[serde(default)]
    pub display_name: String,
    
    #[serde(default)]
    pub description: String,

    /// Enabled / Active status of the action (accepts 'enabled' or 'active' in YAML)
    #[serde(default = "default_true", alias = "active")]
    pub enabled: bool,

    /// Optional link to a UI Button ID (e.g. "HELP", "ALLIANCE_GIFTS_BUTTON", "RADAR_TASKS_BUTTON")
    #[serde(default)]
    pub button: Option<String>,

    /// Optional explicit state requirement (e.g. ALLIANCE_GIFTS_REGULAR)
    #[serde(default)]
    pub state: Option<GameState>,

    /// Allowed parent states where this action can trigger (inherited from button if not set)
    #[serde(default, alias = "parent_state")]
    pub parent_states: Vec<GameState>,

    /// Type of action to perform
    #[serde(default = "default_action_type")]
    pub action_type: ActionType,

    /// Template asset to detect within the ROI
    #[serde(default)]
    pub template: Option<String>,

    /// Optional specific template name/path to click when multiple templates exist in expected/
    #[serde(default)]
    pub click_template: Option<String>,

    /// Region of Interest (ROI) that gates this action
    #[serde(default)]
    pub roi: Option<NormalizedROI>,

    /// Explicit click coordinates (normalized 0.0..1.0)
    #[serde(default)]
    pub coords: Option<(f32, f32)>,

    /// Drag start coordinates (normalized 0.0..1.0)
    #[serde(default)]
    pub drag_start: Option<(f32, f32)>,

    /// Drag end coordinates (normalized 0.0..1.0)
    #[serde(default)]
    pub drag_end: Option<(f32, f32)>,

    /// Drag duration in milliseconds (default: 1000)
    #[serde(default = "default_drag_duration")]
    pub drag_duration_ms: u64,

    /// Key name for KeyPress actions
    #[serde(default)]
    pub key_name: Option<String>,

    /// Minimum matching confidence threshold for template in ROI
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,

    /// Cooldown between consecutive executions in seconds
    #[serde(default = "default_cooldown")]
    pub cooldown_s: f32,

    /// Action execution priority (lower number = higher priority)
    #[serde(default = "default_priority")]
    pub priority: u32,

    /// Whether to restore cursor position after clicking (default: false)
    #[serde(default)]
    pub save_cursor: bool,

    /// Optional shortcut key trigger (e.g. "ctrl+1", "f1", "alt+h")
    #[serde(default)]
    pub shortcut: Option<String>,

    /// Path to a Python script to execute for Custom action types
    #[serde(default, alias = "python_script", alias = "command")]
    pub script: Option<String>,

    /// Optional extra CLI arguments to pass to the python script
    #[serde(default, alias = "script_args")]
    pub args: Vec<String>,

    /// Internal runtime timestamp for cooldown enforcement
    #[serde(skip)]
    pub last_executed: Option<Instant>,
}

impl ActionDefinition {
    pub fn new(name: &str, display_name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: String::new(),
            enabled: true,
            button: None,
            state: None,
            parent_states: vec![],
            action_type: ActionType::ClickTemplate,
            template: None,
            click_template: None,
            roi: None,
            coords: None,
            drag_start: None,
            drag_end: None,
            drag_duration_ms: default_drag_duration(),
            key_name: None,
            min_confidence: default_min_confidence(),
            cooldown_s: default_cooldown(),
            priority: default_priority(),
            save_cursor: false,
            shortcut: None,
            script: None,
            args: Vec::new(),
            last_executed: None,
        }
    }
}

fn default_action_type() -> ActionType {
    ActionType::ClickTemplate
}

fn default_drag_duration() -> u64 {
    1000
}

impl ActionDefinition {
    pub fn is_on_cooldown(&self) -> bool {
        if let Some(last) = self.last_executed {
            last.elapsed() < Duration::from_secs_f32(self.cooldown_s)
        } else {
            false
        }
    }

    pub fn mark_executed(&mut self) {
        self.last_executed = Some(Instant::now());
    }

    /// Resolves template file paths, expanding any directory paths into image files.
    pub fn resolved_templates(&self) -> Vec<String> {
        let tmpl_source = self.template.as_ref().or(self.click_template.as_ref());
        if let Some(t) = tmpl_source {
            let p = std::path::Path::new(t);
            if p.is_dir() {
                let mut results = Vec::new();
                if let Ok(entries) = std::fs::read_dir(p) {
                    for entry in entries.flatten() {
                        let ep = entry.path();
                        if let Some(ext) = ep.extension().and_then(|e| e.to_str()) {
                            let lower = ext.to_lowercase();
                            if lower == "png" || lower == "jpg" || lower == "jpeg" {
                                results.push(ep.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                results.sort();
                results
            } else {
                vec![t.clone()]
            }
        } else {
            Vec::new()
        }
    }

    /// Resolves properties (template, click_template, roi, min_confidence, save_cursor, parent_states, shortcut)
    /// from a corresponding ButtonDefinition if `button` is specified.
    pub fn resolve_button(&mut self, buttons: &[crate::core::button::ButtonDefinition]) {
        if let Some(ref btn_id) = self.button {
            if let Some(btn) = buttons.iter().find(|b| b.id.eq_ignore_ascii_case(btn_id)) {
                if self.template.is_none() {
                    self.template = Some(btn.template.clone());
                }
                if self.click_template.is_none() {
                    self.click_template = btn.click_template.clone();
                }
                if self.roi.is_none() {
                    self.roi = btn.roi;
                }
                if (self.min_confidence - default_min_confidence()).abs() < f32::EPSILON {
                    self.min_confidence = btn.min_confidence;
                }
                if !self.save_cursor && btn.save_cursor {
                    self.save_cursor = true;
                }
                if self.parent_states.is_empty() {
                    self.parent_states = btn.parent_states.clone();
                }
                if self.shortcut.is_none() {
                    self.shortcut = btn.shortcut.clone();
                }
            }
        }
    }
}

/// Executes a Python script asynchronously in the background.
pub fn run_python_script(
    script: &str,
    extra_args: &[String],
    window_id: Option<u32>,
    current_state: Option<&str>,
    screenshot_path: Option<&str>,
) {
    let python_bin = if Path::new("scripts/venv/bin/python3").exists() {
        "scripts/venv/bin/python3"
    } else if Path::new("venv/bin/python3").exists() {
        "venv/bin/python3"
    } else {
        "python3"
    };

    let script_path = if Path::new(script).exists() {
        script.to_string()
    } else if Path::new(&format!("scripts/{}", script)).exists() {
        format!("scripts/{}", script)
    } else {
        script.to_string()
    };

    let mut cmd = std::process::Command::new(python_bin);
    cmd.arg(&script_path);

    if let Some(wid) = window_id {
        cmd.arg("--window-id").arg(wid.to_string());
    }
    if let Some(st) = current_state {
        cmd.arg("--state").arg(st);
    }
    if let Some(sc) = screenshot_path {
        cmd.arg("--screenshot").arg(sc);
    }
    for arg in extra_args {
        cmd.arg(arg);
    }

    println!("[Custom Action] Launching Python script: {} {} {:?}", python_bin, script_path, extra_args);

    let script_display = script_path.clone();
    std::thread::spawn(move || {
        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stdout.is_empty() {
                    println!("[Python Script '{}' stdout]:\n{}", script_display, stdout.trim());
                }
                if !output.status.success() || !stderr.is_empty() {
                    eprintln!("[Python Script '{}' stderr] (code: {:?}):\n{}", script_display, output.status.code(), stderr.trim());
                }
            }
            Err(e) => {
                eprintln!("[Custom Action] Failed to execute Python script '{}': {}", script_display, e);
            }
        }
    });
}

#[derive(Debug, Clone)]
pub struct ActionExecutionResult {
    pub action_name: String,
    pub executed: bool,
    pub reason: String,
    pub click_coords: Option<(i32, i32)>,
    pub drag_coords: Option<((i32, i32), (i32, i32))>,
    pub drag_duration_ms: u64,
    pub sweep_templates: Vec<String>,
    pub script: Option<String>,
    pub script_args: Vec<String>,
    pub save_cursor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceStep {
    pub action: String,
    #[serde(default = "default_timeout", alias = "timeout")]
    pub timeout_s: f32,
}

fn default_timeout() -> f32 {
    5.0
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SequenceSchedules {
    #[serde(default)]
    pub every_day: Option<Vec<String>>,
    #[serde(default)]
    pub monday: Option<Vec<String>>,
    #[serde(default)]
    pub tuesday: Option<Vec<String>>,
    #[serde(default)]
    pub wednesday: Option<Vec<String>>,
    #[serde(default)]
    pub thursday: Option<Vec<String>>,
    #[serde(default)]
    pub friday: Option<Vec<String>>,
    #[serde(default)]
    pub saturday: Option<Vec<String>>,
    #[serde(default)]
    pub sunday: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDefinitionEntry {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_true", alias = "active")]
    pub enabled: bool,
    #[serde(default)]
    pub button: Option<String>,
    #[serde(default)]
    pub state: Option<GameState>,
    #[serde(default, alias = "parent_state", deserialize_with = "crate::core::state::deserialize_gamestates_or_single")]
    pub parent_states: Vec<GameState>,
    #[serde(default = "default_action_type")]
    pub action_type: ActionType,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub click_template: Option<String>,
    #[serde(default)]
    pub roi: Option<NormalizedROI>,
    #[serde(default)]
    pub coords: Option<(f32, f32)>,
    #[serde(default)]
    pub drag_start: Option<(f32, f32)>,
    #[serde(default)]
    pub drag_end: Option<(f32, f32)>,
    #[serde(default = "default_drag_duration")]
    pub drag_duration_ms: u64,
    #[serde(default)]
    pub key_name: Option<String>,
    #[serde(default)]
    pub min_confidence: Option<f32>,
    #[serde(default = "default_cooldown")]
    pub cooldown_s: f32,
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default)]
    pub save_cursor: bool,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default, alias = "python_script", alias = "command")]
    pub script: Option<String>,
    #[serde(default, alias = "script_args")]
    pub args: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ActionsContent {
    WrapperMap {
        actions: std::collections::BTreeMap<String, ActionDefinitionEntry>,
    },
    WrapperList {
        actions: Vec<ActionDefinition>,
    },
    DirectMap(std::collections::BTreeMap<String, ActionDefinitionEntry>),
    DirectList(Vec<ActionDefinition>),
}

pub fn parse_actions_from_str(content: &str) -> Result<Vec<ActionDefinition>, Box<dyn std::error::Error>> {
    let content_parsed: ActionsContent = serde_yaml::from_str(content)?;
    let mut actions = Vec::new();
    match content_parsed {
        ActionsContent::WrapperList { actions: list } | ActionsContent::DirectList(list) => {
            actions = list;
        }
        ActionsContent::WrapperMap { actions: map } | ActionsContent::DirectMap(map) => {
            for (key, entry) in map {
                let name = entry.name.unwrap_or_else(|| key.clone());
                let display_name = entry.display_name.unwrap_or_else(|| name.clone());
                let description = entry.description.unwrap_or_default();
                let min_confidence = entry.min_confidence.unwrap_or_else(default_min_confidence);
                actions.push(ActionDefinition {
                    name,
                    display_name,
                    description,
                    enabled: entry.enabled,
                    button: entry.button,
                    state: entry.state,
                    parent_states: entry.parent_states,
                    action_type: entry.action_type,
                    template: entry.template,
                    click_template: entry.click_template,
                    roi: entry.roi,
                    coords: entry.coords,
                    drag_start: entry.drag_start,
                    drag_end: entry.drag_end,
                    drag_duration_ms: entry.drag_duration_ms,
                    key_name: entry.key_name,
                    min_confidence,
                    cooldown_s: entry.cooldown_s,
                    priority: entry.priority,
                    save_cursor: entry.save_cursor,
                    shortcut: entry.shortcut,
                    script: entry.script,
                    args: entry.args,
                    last_executed: None,
                });
            }
        }
    }
    Ok(actions)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceDefinitionEntry {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, alias = "loop")]
    pub repeat: bool,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub schedules: Option<SequenceSchedules>,
    #[serde(default)]
    pub steps: Vec<SequenceStep>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SequencesContent {
    WrapperMap {
        sequences: std::collections::BTreeMap<String, SequenceDefinitionEntry>,
    },
    WrapperList {
        sequences: Vec<SequenceDefinition>,
    },
    DirectMap(std::collections::BTreeMap<String, SequenceDefinitionEntry>),
    DirectList(Vec<SequenceDefinition>),
}

pub fn parse_sequences_from_str(content: &str) -> Result<Vec<SequenceDefinition>, Box<dyn std::error::Error>> {
    let content_parsed: SequencesContent = serde_yaml::from_str(content)?;
    let mut sequences = Vec::new();
    match content_parsed {
        SequencesContent::WrapperList { sequences: list } | SequencesContent::DirectList(list) => {
            sequences = list;
        }
        SequencesContent::WrapperMap { sequences: map } | SequencesContent::DirectMap(map) => {
            for (key, entry) in map {
                let name = entry.name.unwrap_or_else(|| key.clone());
                let description = entry.description.unwrap_or_default();
                sequences.push(SequenceDefinition {
                    name,
                    description,
                    enabled: entry.enabled,
                    repeat: entry.repeat,
                    shortcut: entry.shortcut,
                    schedules: entry.schedules,
                    steps: entry.steps,
                });
            }
        }
    }
    Ok(sequences)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, alias = "loop")]
    pub repeat: bool,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub schedules: Option<SequenceSchedules>,
    #[serde(default)]
    pub steps: Vec<SequenceStep>,
}

#[derive(Debug, Clone)]
pub struct ActiveSequenceState {
    pub sequence_name: String,
    pub current_step_index: usize,
    pub step_start_time: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionsConfigFile {
    #[serde(default)]
    pub actions: Vec<ActionDefinition>,
    #[serde(default)]
    pub buttons: Vec<crate::core::button::ButtonDefinition>,
    #[serde(default)]
    pub sequences: Vec<SequenceDefinition>,
}

use std::collections::{HashMap, VecDeque};
use chrono::{Local, Datelike, Timelike};

pub struct ActionManager {
    pub actions: RwLock<Vec<ActionDefinition>>,
    sequences: RwLock<Vec<SequenceDefinition>>,
    active_sequence: RwLock<Option<ActiveSequenceState>>,
    sequence_queue: RwLock<VecDeque<String>>,
    sequence_last_run: RwLock<HashMap<String, String>>, // sequence_name -> "YYYY-MM-DD HH:MM"
}

impl ActionManager {
    pub fn new(actions: Vec<ActionDefinition>, sequences: Vec<SequenceDefinition>) -> Self {
        Self {
            actions: RwLock::new(actions),
            sequences: RwLock::new(sequences),
            active_sequence: RwLock::new(None),
            sequence_queue: RwLock::new(VecDeque::new()),
            sequence_last_run: RwLock::new(HashMap::new()),
        }
    }

    pub fn from_actions(actions: Vec<ActionDefinition>) -> Self {
        Self::new(actions, Vec::new())
    }

    pub fn new_with_buttons(mut actions: Vec<ActionDefinition>, buttons: &[crate::core::button::ButtonDefinition], sequences: Vec<SequenceDefinition>) -> Self {
        for action in actions.iter_mut() {
            action.resolve_button(buttons);
        }
        Self::new(actions, sequences)
    }

    pub fn load_from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let parsed: ActionsConfigFile = serde_yaml::from_str(&content)?;
        let mut actions = parsed.actions;
        for action in actions.iter_mut() {
            action.resolve_button(&parsed.buttons);
        }
        Ok(Self::new(actions, parsed.sequences))
    }

    /// Reloads all actions and button references from YAML configuration file,
    /// updating cooldowns, priorities, templates, shortcuts, enabled states, etc.,
    /// while preserving runtime cooldown execution timestamps (`last_executed`).
    pub fn reload_from_yaml<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let loaded_actions = crate::core::state::load_actions_from_config(path.as_ref())?;
        let loaded_sequences = crate::core::state::load_sequences_from_config(path.as_ref()).unwrap_or_default();
        let mut list = self.actions.write().unwrap();
        let mut seq_list = self.sequences.write().unwrap();

        // Build map of existing last_executed timestamps
        let mut last_exec_map = std::collections::HashMap::new();
        for a in list.iter() {
            if let Some(ts) = a.last_executed {
                last_exec_map.insert(a.name.to_lowercase(), ts);
            }
        }

        let mut new_list = loaded_actions;
        for action in new_list.iter_mut() {
            if let Some(ts) = last_exec_map.get(&action.name.to_lowercase()) {
                action.last_executed = Some(*ts);
            }
        }

        *list = new_list;
        *seq_list = loaded_sequences;
        Ok(())
    }

    pub fn list_actions(&self) -> Vec<ActionDefinition> {
        self.actions.read().unwrap().clone()
    }

    pub fn list_sequences(&self) -> Vec<SequenceDefinition> {
        self.sequences.read().unwrap().clone()
    }

    pub fn update_sequence(&self, updated_seq: SequenceDefinition) -> bool {
        let mut list = self.sequences.write().unwrap();
        if let Some(seq) = list.iter_mut().find(|s| s.name.eq_ignore_ascii_case(&updated_seq.name)) {
            *seq = updated_seq;
            true
        } else {
            false
        }
    }

    pub fn update_action(&self, updated_action: ActionDefinition) -> bool {
        let mut list = self.actions.write().unwrap();
        if let Some(action) = list.iter_mut().find(|a| a.name.eq_ignore_ascii_case(&updated_action.name)) {
            *action = updated_action;
            true
        } else {
            false
        }
    }

    pub fn set_action_enabled(&self, name: &str, enabled: bool) -> bool {
        let mut list = self.actions.write().unwrap();
        if let Some(action) = list.iter_mut().find(|a| a.name.eq_ignore_ascii_case(name)) {
            action.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn set_action_cooldown(&self, name: &str, cooldown: f32) -> bool {
        let mut list = self.actions.write().unwrap();
        if let Some(action) = list.iter_mut().find(|a| a.name.eq_ignore_ascii_case(name)) {
            action.cooldown_s = cooldown;
            true
        } else {
            false
        }
    }

    pub fn is_action_enabled(&self, name: &str) -> bool {
        let list = self.actions.read().unwrap();
        list.iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.enabled)
            .unwrap_or(false)
    }

    /// Returns list of all defined action shortcuts as `(trigger_id, shortcut_spec)`
    pub fn get_shortcuts(&self) -> Vec<(String, String)> {
        let mut shortcuts = Vec::new();
        
        let list = self.actions.read().unwrap();
        for a in list.iter() {
            if let Some(ref s) = a.shortcut {
                shortcuts.push((format!("action:{}", a.name), s.clone()));
            }
        }

        let seqs = self.sequences.read().unwrap();
        for seq in seqs.iter() {
            if let Some(ref s) = seq.shortcut {
                shortcuts.push((format!("sequence:{}", seq.name), s.clone()));
            }
        }

        shortcuts
    }

    /// Directly evaluates and executes a single action by name (e.g. triggered via manual keyboard shortcut).
    pub fn execute_single_action(
        &self,
        name: &str,
        current_state: GameState,
        screen: &RgbaImage,
        matcher: &mut TemplateMatcher,
        bypass_cooldown: bool,
        bypass_state_check: bool,
    ) -> Option<ActionExecutionResult> {
        let (img_w, img_h) = screen.dimensions();
        let mut list = self.actions.write().unwrap();
        let action = list.iter_mut().find(|a| a.name.eq_ignore_ascii_case(name))?;

        // 1. Check state condition if specified, and parent states
        //    (skipped for sequence steps — user manually triggered them)
        if !bypass_state_check {
            if let Some(req_state) = action.state {
                if req_state != current_state {
                    return Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: false,
                        reason: format!("Current state {:?} does not match required state {:?}", current_state, req_state),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    });
                }
            }
            if !action.parent_states.is_empty() && !action.parent_states.contains(&current_state) {
                return Some(ActionExecutionResult {
                    action_name: action.name.clone(),
                    executed: false,
                    reason: format!("Current state {:?} is not in parent_states {:?}", current_state, action.parent_states),
                    click_coords: None,
                    drag_coords: None,
                    drag_duration_ms: action.drag_duration_ms,
                    sweep_templates: Vec::new(),
                    script: None,
                    script_args: Vec::new(),
                    save_cursor: action.save_cursor,
                });
            }
        }

        // 2. Check cooldown if not bypassed
        if !bypass_cooldown && action.is_on_cooldown() {
            return Some(ActionExecutionResult {
                action_name: action.name.clone(),
                executed: false,
                reason: format!("Action is on cooldown ({:.1}s)", action.cooldown_s),
                click_coords: None,
                drag_coords: None,
                drag_duration_ms: action.drag_duration_ms,
                sweep_templates: Vec::new(),
                script: None,
                script_args: Vec::new(),
                save_cursor: action.save_cursor,
            });
        }

        let roi_px = action.roi.map(|r| r.to_pixel_box(img_w, img_h));

        match action.action_type {
            ActionType::ClickTemplate => {
                let templates = action.resolved_templates();
                if templates.is_empty() {
                    return Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: false,
                        reason: "Missing template path for ClickTemplate action".to_string(),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    });
                }

                let mut all_matched = true;
                let mut min_confidence = 1.0f32;
                let mut matches = Vec::new();

                for tmpl_path in &templates {
                    let match_res = matcher.find_match(
                        screen,
                        tmpl_path,
                        action.min_confidence,
                        roi_px,
                    );

                    if !match_res.matched {
                        all_matched = false;
                        break;
                    }

                    if match_res.confidence < min_confidence {
                        min_confidence = match_res.confidence;
                    }
                    matches.push((tmpl_path.clone(), match_res));
                }

                if all_matched && !matches.is_empty() {
                    let click_match = crate::core::button::select_click_match(&matches, action.click_template.as_deref())
                        .unwrap_or(&matches[0].1);
                    action.mark_executed();
                    let jx = random_jitter(click_match.width as f32 * 0.18);
                    let jy = random_jitter(click_match.height as f32 * 0.18);
                    let click_x = (click_match.center_x as f32 + jx).round() as i32;
                    let click_y = (click_match.center_y as f32 + jy).round() as i32;

                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: format!(
                            "Template matched with {:.2}% confidence in ROI (manual shortcut)",
                            min_confidence * 100.0
                        ),
                        click_coords: Some((click_x, click_y)),
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                } else {
                    let failed_conf = matches.iter().map(|(_, m)| m.confidence).fold(0.0f32, f32::max);
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: false,
                        reason: format!("Template match failed (confidence: {:.2}%)", failed_conf * 100.0),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                }
            }
            ActionType::ClickRoi => {
                if let Some(roi) = action.roi {
                    let w = (roi.xmax - roi.xmin) * img_w as f32;
                    let h = (roi.ymax - roi.ymin) * img_h as f32;
                    let center_x = (roi.xmin + roi.xmax) * 0.5 * img_w as f32 + random_jitter(w * 0.15);
                    let center_y = (roi.ymin + roi.ymax) * 0.5 * img_h as f32 + random_jitter(h * 0.15);
                    action.mark_executed();
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: "Clicked center of designated ROI".to_string(),
                        click_coords: Some((center_x.round() as i32, center_y.round() as i32)),
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                } else {
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: false,
                        reason: "Missing ROI definition for ClickRoi action".to_string(),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                }
            }
            ActionType::ClickCoords => {
                if let Some((nx, ny)) = action.coords {
                    let cx = (nx * img_w as f32) + random_jitter(4.0);
                    let cy = (ny * img_h as f32) + random_jitter(4.0);
                    action.mark_executed();
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: "Clicked designated normalized coordinates".to_string(),
                        click_coords: Some((cx.round() as i32, cy.round() as i32)),
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                } else {
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: false,
                        reason: "Missing coordinates (coords: [X, Y]) for ClickCoords action".to_string(),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                }
            }
            ActionType::KeyPress => {
                if action.key_name.is_none() {
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: false,
                        reason: "Missing key_name for KeyPress action".to_string(),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                } else {
                    action.mark_executed();
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: format!("Key press action: {:?}", action.key_name),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                }
            }
            ActionType::DragDrop => {
                if let (Some(start), Some(end)) = (action.drag_start, action.drag_end) {
                    let sx = (start.0 * img_w as f32).round() as i32;
                    let sy = (start.1 * img_h as f32).round() as i32;
                    let ex = (end.0 * img_w as f32).round() as i32;
                    let ey = (end.1 * img_h as f32).round() as i32;
                    action.mark_executed();
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: "DragDrop action triggered".to_string(),
                        click_coords: None,
                        drag_coords: Some(((sx, sy), (ex, ey))),
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: action.resolved_templates(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                } else {
                    Some(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: false,
                        reason: "Missing drag_start or drag_end coordinates for DragDrop action".to_string(),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    })
                }
            }
            ActionType::Custom => {
                action.mark_executed();
                Some(ActionExecutionResult {
                    action_name: action.name.clone(),
                    executed: true,
                    reason: format!("Custom script action triggered: {:?}", action.script),
                    click_coords: None,
                    drag_coords: None,
                    drag_duration_ms: action.drag_duration_ms,
                    sweep_templates: Vec::new(),
                    script: action.script.clone(),
                    script_args: action.args.clone(),
                    save_cursor: action.save_cursor,
                })
            }
        }
    }

    /// Evaluates all active actions against the current screen and state.
    /// Checks ROI bounds, template matching within ROI, and cooldowns.


    pub fn pop_sequence_queue(&self) -> Option<String> {
        let mut q = self.sequence_queue.write().unwrap();
        q.pop_front()
    }

    pub fn evaluate_schedules(&self) {
        let now = Local::now();
        let current_time_str = format!("{:02}:{:02}", now.hour(), now.minute());
        let current_day_time_str = format!("{} {}", now.format("%Y-%m-%d"), current_time_str);

        let weekday = now.weekday();

        let seqs = self.sequences.read().unwrap();
        let mut to_trigger = Vec::new();

        for seq in seqs.iter() {
            if !seq.enabled { continue; }
            if let Some(ref scheds) = seq.schedules {
                let mut planned_times = Vec::new();
                if let Some(ref ed) = scheds.every_day {
                    planned_times.extend(ed.iter());
                }
                match weekday {
                    chrono::Weekday::Mon => if let Some(ref d) = scheds.monday { planned_times.extend(d.iter()); },
                    chrono::Weekday::Tue => if let Some(ref d) = scheds.tuesday { planned_times.extend(d.iter()); },
                    chrono::Weekday::Wed => if let Some(ref d) = scheds.wednesday { planned_times.extend(d.iter()); },
                    chrono::Weekday::Thu => if let Some(ref d) = scheds.thursday { planned_times.extend(d.iter()); },
                    chrono::Weekday::Fri => if let Some(ref d) = scheds.friday { planned_times.extend(d.iter()); },
                    chrono::Weekday::Sat => if let Some(ref d) = scheds.saturday { planned_times.extend(d.iter()); },
                    chrono::Weekday::Sun => if let Some(ref d) = scheds.sunday { planned_times.extend(d.iter()); },
                }

                if planned_times.iter().any(|t| *t == &current_time_str) {
                    let mut lr_lock = self.sequence_last_run.write().unwrap();
                    let last_run = lr_lock.get(&seq.name);
                    if last_run.map(|s| s != &current_day_time_str).unwrap_or(true) {
                        lr_lock.insert(seq.name.clone(), current_day_time_str.clone());
                        to_trigger.push(seq.name.clone());
                    }
                }
            }
        }
        
        if !to_trigger.is_empty() {
            let mut q = self.sequence_queue.write().unwrap();
            for t in to_trigger {
                if !q.contains(&t) {
                    println!("[Schedule] It is {}, sequence '{}' queued for execution.", current_time_str, t);
                    q.push_back(t);
                }
            }
        }
    }

    pub fn trigger_sequence(&self, name: &str) -> bool {
        let seqs = self.sequences.read().unwrap();
        if let Some(seq) = seqs.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
            if seq.enabled && !seq.steps.is_empty() {
                let mut active = self.active_sequence.write().unwrap();
                *active = Some(ActiveSequenceState {
                    sequence_name: seq.name.clone(),
                    current_step_index: 0,
                    step_start_time: std::time::Instant::now(),
                });
                return true;
            }
        }
        false
    }

    pub fn evaluate_sequence(
        &self,
        current_state: GameState,
        screen: &image::RgbaImage,
        matcher: &mut crate::vision::matching::TemplateMatcher,
    ) -> Option<ActionExecutionResult> {
        let mut active_lock = self.active_sequence.write().unwrap();
        let active_state = active_lock.clone()?;

        let seqs = self.sequences.read().unwrap();
        let seq = seqs.iter().find(|s| s.name == active_state.sequence_name)?;

        if active_state.current_step_index >= seq.steps.len() {
            *active_lock = None;
            return None;
        }

        let step = &seq.steps[active_state.current_step_index];
        
        if active_state.step_start_time.elapsed() > std::time::Duration::from_secs_f32(step.timeout_s) {
            println!("[Sequence] Step {} '{}' timed out after {:.1}s. Continuing to next step in sequence '{}'.",
                active_state.current_step_index, step.action, step.timeout_s, seq.name);
            if active_state.current_step_index + 1 < seq.steps.len() {
                let mut new_state = active_state.clone();
                new_state.current_step_index += 1;
                new_state.step_start_time = std::time::Instant::now();
                *active_lock = Some(new_state);
            } else if seq.repeat {
                println!("[Sequence] Sequence '{}' step timed out on last step. Looping back to step 0...", seq.name);
                let mut new_state = active_state.clone();
                new_state.current_step_index = 0;
                new_state.step_start_time = std::time::Instant::now();
                *active_lock = Some(new_state);
            } else {
                println!("[Sequence] Sequence '{}' finished (last step timed out).", seq.name);
                *active_lock = None;
            }
            return None;
        }

        let res = self.execute_single_action(&step.action, current_state, screen, matcher, true, true);
        if let Some(ref r) = res {
            if r.executed {
                println!("[Sequence] Executed step {} '{}' of '{}'.", active_state.current_step_index, step.action, seq.name);
                if active_state.current_step_index + 1 < seq.steps.len() {
                    let mut new_state = active_state.clone();
                    new_state.current_step_index += 1;
                    new_state.step_start_time = std::time::Instant::now();
                    *active_lock = Some(new_state);
                } else if seq.repeat {
                    println!("[Sequence] Sequence '{}' completed iteration. Looping back to step 0...", seq.name);
                    let mut new_state = active_state.clone();
                    new_state.current_step_index = 0;
                    new_state.step_start_time = std::time::Instant::now();
                    *active_lock = Some(new_state);
                } else {
                    println!("[Sequence] Sequence '{}' completed.", seq.name);
                    *active_lock = None;
                }
            } else {
                println!("[Sequence Debug] Step '{}' waiting: {}", step.action, r.reason);
            }
        } else {
            println!("[Sequence Debug] Step '{}' waiting (No ActionExecutionResult returned).", step.action);
        }
        res
    }

    pub fn has_active_sequence(&self) -> bool {
        self.active_sequence.read().unwrap().is_some()
    }
    pub fn evaluate(
        &self,
        current_state: GameState,
        screen: &RgbaImage,
        matcher: &mut TemplateMatcher,
    ) -> Vec<ActionExecutionResult> {
        let (img_w, img_h) = screen.dimensions();
        let mut results = Vec::new();
        let mut list = self.actions.write().unwrap();

        // Sort by priority
        list.sort_by_key(|a| a.priority);

        for action in list.iter_mut() {
            if !action.enabled {
                continue;
            }

            // 1. Check state condition if specified, and parent states from linked button
            if let Some(req_state) = action.state {
                if req_state != current_state {
                    continue;
                }
            }
            if !action.parent_states.is_empty() {
                if !action.parent_states.contains(&current_state) {
                    continue;
                }
            }

            // 2. Check cooldown
            if action.is_on_cooldown() {
                results.push(ActionExecutionResult {
                    action_name: action.name.clone(),
                    executed: false,
                    reason: format!("Action is on cooldown ({:.1}s)", action.cooldown_s),
                    click_coords: None,
                    drag_coords: None,
                    drag_duration_ms: action.drag_duration_ms,
                    sweep_templates: Vec::new(),
                    script: None,
                    script_args: Vec::new(),
                    save_cursor: action.save_cursor,
                });
                continue;
            }

            let roi_px = action.roi.map(|r| r.to_pixel_box(img_w, img_h));

            match action.action_type {
                ActionType::ClickTemplate => {
                    let templates = action.resolved_templates();
                    if templates.is_empty() {
                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: false,
                            reason: "Missing template path for ClickTemplate action".to_string(),
                            click_coords: None,
                            drag_coords: None,
                            drag_duration_ms: action.drag_duration_ms,
                            sweep_templates: Vec::new(),
                            script: None,
                            script_args: Vec::new(),
                            save_cursor: action.save_cursor,
                        });
                        continue;
                    }

                    let mut all_matched = true;
                    let mut min_confidence = 1.0f32;
                    let mut matches = Vec::new();

                    for tmpl_path in &templates {
                        let match_res = matcher.find_match(
                            screen,
                            tmpl_path,
                            action.min_confidence,
                            roi_px,
                        );

                        if !match_res.matched {
                            all_matched = false;
                            break;
                        }

                        if match_res.confidence < min_confidence {
                            min_confidence = match_res.confidence;
                        }
                        matches.push((tmpl_path.clone(), match_res));
                    }

                    if all_matched && !matches.is_empty() {
                        let click_match = crate::core::button::select_click_match(&matches, action.click_template.as_deref())
                            .unwrap_or(&matches[0].1);
                        action.mark_executed();
                        let jx = random_jitter(click_match.width as f32 * 0.18);
                        let jy = random_jitter(click_match.height as f32 * 0.18);
                        let click_x = (click_match.center_x as f32 + jx).round() as i32;
                        let click_y = (click_match.center_y as f32 + jy).round() as i32;

                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: true,
                            reason: format!(
                                "Template matched with {:.2}% confidence in ROI",
                                min_confidence * 100.0
                            ),
                            click_coords: Some((click_x, click_y)),
                            drag_coords: None,
                            drag_duration_ms: action.drag_duration_ms,
                            sweep_templates: Vec::new(),
                            script: None,
                            script_args: Vec::new(),
                            save_cursor: action.save_cursor,
                        });
                    }
                }
                ActionType::ClickRoi => {
                    if let Some(roi) = action.roi {
                        let w = (roi.xmax - roi.xmin) * img_w as f32;
                        let h = (roi.ymax - roi.ymin) * img_h as f32;
                        let center_x = (roi.xmin + roi.xmax) * 0.5 * img_w as f32 + random_jitter(w * 0.15);
                        let center_y = (roi.ymin + roi.ymax) * 0.5 * img_h as f32 + random_jitter(h * 0.15);
                        action.mark_executed();
                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: true,
                            reason: "Clicked center of designated ROI".to_string(),
                            click_coords: Some((center_x.round() as i32, center_y.round() as i32)),
                            drag_coords: None,
                            drag_duration_ms: action.drag_duration_ms,
                            sweep_templates: Vec::new(),
                            script: None,
                            script_args: Vec::new(),
                            save_cursor: action.save_cursor,
                        });
                    }
                }
                ActionType::ClickCoords => {
                    if let Some((nx, ny)) = action.coords {
                        let cx = (nx * img_w as f32) + random_jitter(4.0);
                        let cy = (ny * img_h as f32) + random_jitter(4.0);
                        action.mark_executed();
                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: true,
                            reason: "Clicked designated normalized coordinates".to_string(),
                            click_coords: Some((cx.round() as i32, cy.round() as i32)),
                            drag_coords: None,
                            drag_duration_ms: action.drag_duration_ms,
                            sweep_templates: Vec::new(),
                            script: None,
                            script_args: Vec::new(),
                            save_cursor: action.save_cursor,
                        });
                    }
                }
                ActionType::KeyPress => {
                    action.mark_executed();
                    results.push(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: format!("Key press action: {:?}", action.key_name),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: None,
                        script_args: Vec::new(),
                        save_cursor: action.save_cursor,
                    });
                }
                ActionType::DragDrop => {
                    let templates = action.resolved_templates();
                    if !templates.is_empty() {
                        let mut all_matched = true;
                        for tmpl_path in &templates {
                            let match_res = matcher.find_match(
                                screen,
                                tmpl_path,
                                action.min_confidence,
                                roi_px,
                            );
                            if !match_res.matched {
                                all_matched = false;
                                break;
                            }
                        }
                        if !all_matched {
                            continue;
                        }
                    }

                    if let (Some(start), Some(end)) = (action.drag_start, action.drag_end) {
                        let sx = (start.0 * img_w as f32).round() as i32;
                        let sy = (start.1 * img_h as f32).round() as i32;
                        let ex = (end.0 * img_w as f32).round() as i32;
                        let ey = (end.1 * img_h as f32).round() as i32;
                        action.mark_executed();
                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: true,
                            reason: "DragDrop action triggered".to_string(),
                            click_coords: None,
                            drag_coords: Some(((sx, sy), (ex, ey))),
                            drag_duration_ms: action.drag_duration_ms,
                            sweep_templates: action.resolved_templates(),
                            script: None,
                            script_args: Vec::new(),
                            save_cursor: action.save_cursor,
                        });
                    }
                }
                ActionType::Custom => {
                    action.mark_executed();
                    results.push(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: format!("Custom script action triggered: {:?}", action.script),
                        click_coords: None,
                        drag_coords: None,
                        drag_duration_ms: action.drag_duration_ms,
                        sweep_templates: Vec::new(),
                        script: action.script.clone(),
                        script_args: action.args.clone(),
                        save_cursor: action.save_cursor,
                    });
                }
            }
        }

        results
    }
}

fn random_jitter(range: f32) -> f32 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x12345678);
    let mut state = nanos.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let norm = (state >> 40) as f32 / 16777216.0;
    (norm * 2.0 - 1.0) * range
}
