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
    Custom,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::ClickTemplate => "click_template",
            ActionType::ClickCoords => "click_coords",
            ActionType::ClickRoi => "click_roi",
            ActionType::KeyPress => "key_press",
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
    #[serde(default)]
    pub parent_states: Vec<GameState>,

    /// Type of action to perform
    #[serde(default = "default_action_type")]
    pub action_type: ActionType,

    /// Template asset to detect within the ROI
    #[serde(default)]
    pub template: Option<String>,

    /// Region of Interest (ROI) that gates this action
    #[serde(default)]
    pub roi: Option<NormalizedROI>,

    /// Explicit click coordinates (normalized 0.0..1.0)
    #[serde(default)]
    pub coords: Option<(f32, f32)>,

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

    /// Internal runtime timestamp for cooldown enforcement
    #[serde(skip)]
    pub last_executed: Option<Instant>,
}

fn default_action_type() -> ActionType {
    ActionType::ClickTemplate
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

    /// Resolves properties (template, roi, min_confidence, save_cursor, parent_states)
    /// from a corresponding ButtonDefinition if `button` is specified.
    pub fn resolve_button(&mut self, buttons: &[crate::core::button::ButtonDefinition]) {
        if let Some(ref btn_id) = self.button {
            if let Some(btn) = buttons.iter().find(|b| b.id.eq_ignore_ascii_case(btn_id)) {
                if self.template.is_none() {
                    self.template = Some(btn.template.clone());
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
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionExecutionResult {
    pub action_name: String,
    pub executed: bool,
    pub reason: String,
    pub click_coords: Option<(i32, i32)>,
    pub save_cursor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionsConfigFile {
    #[serde(default)]
    pub actions: Vec<ActionDefinition>,
    #[serde(default)]
    pub buttons: Vec<crate::core::button::ButtonDefinition>,
}

pub struct ActionManager {
    actions: RwLock<Vec<ActionDefinition>>,
}

impl ActionManager {
    pub fn new(actions: Vec<ActionDefinition>) -> Self {
        Self {
            actions: RwLock::new(actions),
        }
    }

    pub fn new_with_buttons(mut actions: Vec<ActionDefinition>, buttons: &[crate::core::button::ButtonDefinition]) -> Self {
        for action in actions.iter_mut() {
            action.resolve_button(buttons);
        }
        Self::new(actions)
    }

    pub fn load_from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let parsed: ActionsConfigFile = serde_yaml::from_str(&content)?;
        let mut actions = parsed.actions;
        for action in actions.iter_mut() {
            action.resolve_button(&parsed.buttons);
        }
        Ok(Self::new(actions))
    }

    pub fn list_actions(&self) -> Vec<ActionDefinition> {
        self.actions.read().unwrap().clone()
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

    pub fn is_action_enabled(&self, name: &str) -> bool {
        let list = self.actions.read().unwrap();
        list.iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.enabled)
            .unwrap_or(false)
    }

    /// Evaluates all active actions against the current screen and state.
    /// Checks ROI bounds, template matching within ROI, and cooldowns.
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

            // 1. Check state condition if specified, or parent states from linked button
            if let Some(req_state) = action.state {
                if req_state != current_state {
                    continue;
                }
            } else if !action.parent_states.is_empty() {
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
                    save_cursor: action.save_cursor,
                });
                continue;
            }

            let roi_px = action.roi.map(|r| r.to_pixel_box(img_w, img_h));

            match action.action_type {
                ActionType::ClickTemplate => {
                    let tmpl_path = match action.template.as_deref() {
                        Some(p) => p,
                        None => {
                            results.push(ActionExecutionResult {
                                action_name: action.name.clone(),
                                executed: false,
                                reason: "Missing template path for ClickTemplate action".to_string(),
                                click_coords: None,
                                save_cursor: action.save_cursor,
                            });
                            continue;
                        }
                    };

                    let match_res = matcher.find_match(
                        screen,
                        tmpl_path,
                        action.min_confidence,
                        roi_px,
                    );

                    if match_res.matched {
                        action.mark_executed();
                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: true,
                            reason: format!(
                                "Template matched with {:.2}% confidence in ROI",
                                match_res.confidence * 100.0
                            ),
                            click_coords: Some((match_res.center_x as i32, match_res.center_y as i32)),
                            save_cursor: action.save_cursor,
                        });
                    }
                }
                ActionType::ClickRoi => {
                    if let Some(roi) = action.roi {
                        let center_x = ((roi.xmin + roi.xmax) * 0.5 * img_w as f32) as i32;
                        let center_y = ((roi.ymin + roi.ymax) * 0.5 * img_h as f32) as i32;
                        action.mark_executed();
                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: true,
                            reason: "Clicked center of designated ROI".to_string(),
                            click_coords: Some((center_x, center_y)),
                            save_cursor: action.save_cursor,
                        });
                    }
                }
                ActionType::ClickCoords => {
                    if let Some((nx, ny)) = action.coords {
                        let cx = (nx * img_w as f32) as i32;
                        let cy = (ny * img_h as f32) as i32;
                        action.mark_executed();
                        results.push(ActionExecutionResult {
                            action_name: action.name.clone(),
                            executed: true,
                            reason: "Clicked designated normalized coordinates".to_string(),
                            click_coords: Some((cx, cy)),
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
                        save_cursor: action.save_cursor,
                    });
                }
                ActionType::Custom => {
                    action.mark_executed();
                    results.push(ActionExecutionResult {
                        action_name: action.name.clone(),
                        executed: true,
                        reason: "Custom action triggered".to_string(),
                        click_coords: None,
                        save_cursor: action.save_cursor,
                    });
                }
            }
        }

        results
    }
}
