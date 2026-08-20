//! Button definitions and detection for UI elements attached to GameStates.

use std::collections::BTreeMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::core::state::{GameState, NormalizedROI};

fn default_min_confidence() -> f32 {
    0.85
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonDefinitionEntry {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "crate::core::state::deserialize_gamestates_or_single")]
    pub parent_states: Vec<GameState>,
    #[serde(default)]
    pub target_state: Option<GameState>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub click_template: Option<String>,
    #[serde(default)]
    pub roi: Option<NormalizedROI>,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    #[serde(default)]
    pub save_cursor: bool,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A button or interactive UI element that can appear on one or more GameStates.
/// Clicks on this button can either transition to a `target_state` or execute a standalone action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonDefinition {
    /// Unique identifier for this button (e.g. "HELP", "ALLIANCE_GIFTS_BUTTON", "MAIN_SHOP")
    pub id: String,

    /// Human readable name
    pub display_name: String,

    /// The game states on which this button can appear
    #[serde(default, deserialize_with = "crate::core::state::deserialize_gamestates_or_single")]
    pub parent_states: Vec<GameState>,

    /// The destination game state if clicking this button opens another screen/menu
    #[serde(default)]
    pub target_state: Option<GameState>,

    /// Path to template image under roi/
    #[serde(default)]
    pub template: String,

    /// Optional specific template name/path to click when multiple templates exist in expected/
    /// e.g. "claim.png", "button.png", or "roi/LOOT_BUTTON/expected/claim.png"
    #[serde(default)]
    pub click_template: Option<String>,

    /// Normalized Region Of Interest (0.0 .. 1.0)
    #[serde(default)]
    pub roi: Option<NormalizedROI>,

    /// Minimum matching confidence (0.0 .. 1.0)
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,

    /// Whether to restore previous cursor position after clicking this button (default: false)
    #[serde(default)]
    pub save_cursor: bool,

    /// Optional shortcut key trigger (e.g. "ctrl+1", "f1", "alt+h")
    #[serde(default)]
    pub shortcut: Option<String>,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
}

impl ButtonDefinition {
    /// Resolves template file paths, expanding any directory paths into image files.
    pub fn resolved_templates(&self) -> Vec<String> {
        let p = std::path::Path::new(&self.template);
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
            if !results.is_empty() {
                return results;
            }
        }

        // Check fallback candidate paths (expected/ directory or direct file)
        let candidate_paths = [
            format!("{}/expected", self.template.trim_end_matches('/')),
            format!("{}.png", self.template.trim_end_matches('/')),
            format!("roi/{}/expected", self.id),
            format!("roi/{}/expected.png", self.id),
            format!("roi/{}/expected", self.id.strip_suffix("_BUTTON").unwrap_or(&self.id)),
            format!("roi/{}/expected.png", self.id.strip_suffix("_BUTTON").unwrap_or(&self.id)),
        ];

        for cand in candidate_paths {
            let cp = std::path::Path::new(&cand);
            if cp.is_dir() {
                let mut results = Vec::new();
                if let Ok(entries) = std::fs::read_dir(cp) {
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
                if !results.is_empty() {
                    return results;
                }
            } else if cp.is_file() {
                return vec![cand];
            }
        }

        if !self.template.is_empty() {
            vec![self.template.clone()]
        } else {
            vec![format!("roi/{}/expected/", self.id)]
        }
    }
}

/// Selects the matching template result to click from a list of matched templates.
/// If `click_template` is specified, finds the matching template by full path, relative path,
/// or filename (with or without extension). Otherwise, defaults to the first match.
pub fn select_click_match<'a, T>(
    matches: &'a [(String, T)],
    click_template: Option<&str>,
) -> Option<&'a T> {
    if matches.is_empty() {
        return None;
    }
    if let Some(target) = click_template {
        let target_clean = target.trim();
        if let Some((_, m)) = matches.iter().find(|(path, _)| {
            path == target_clean
                || path.ends_with(target_clean)
                || std::path::Path::new(path)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map(|f| {
                        f.eq_ignore_ascii_case(target_clean)
                            || f.strip_suffix(".png").unwrap_or(f).eq_ignore_ascii_case(target_clean)
                            || f.strip_suffix(".jpg").unwrap_or(f).eq_ignore_ascii_case(target_clean)
                            || f.strip_suffix(".jpeg").unwrap_or(f).eq_ignore_ascii_case(target_clean)
                    })
                    .unwrap_or(false)
        }) {
            return Some(m);
        }
    }
    Some(&matches[0].1)
}

/// Result of a button detected on the current screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonDetection {
    /// Button identifier
    pub id: String,

    /// Display name
    pub display_name: String,

    /// Destination game state if clicking this button transitions to another state
    pub target_state: Option<GameState>,

    /// Matching confidence score
    pub confidence: f32,

    /// Template matched
    pub matched_template: String,

    /// Pixel bounding box of the match within the screen (x, y, width, height)
    pub match_box: (u32, u32, u32, u32),

    /// Screen-relative center coordinates (x, y) for clicking
    pub match_center: (i32, i32),

    /// Whether to restore previous cursor position after clicking this button
    #[serde(default)]
    pub save_cursor: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ButtonsContent {
    WrapperMap {
        buttons: BTreeMap<String, ButtonDefinitionEntry>,
    },
    WrapperList {
        buttons: Vec<ButtonDefinition>,
    },
    DirectMap(BTreeMap<String, ButtonDefinitionEntry>),
    DirectList(Vec<ButtonDefinition>),
}

pub fn parse_buttons_from_str(content: &str) -> Result<Vec<ButtonDefinition>, Box<dyn std::error::Error>> {
    let content_parsed: ButtonsContent = serde_yaml::from_str(content)?;
    let mut buttons = Vec::new();
    match content_parsed {
        ButtonsContent::WrapperList { buttons: list } | ButtonsContent::DirectList(list) => {
            buttons = list;
        }
        ButtonsContent::WrapperMap { buttons: map } | ButtonsContent::DirectMap(map) => {
            for (key, entry) in map {
                let id = entry.id.unwrap_or_else(|| key.clone());
                let display_name = entry.display_name.unwrap_or_else(|| id.clone());
                let template = entry.template.unwrap_or_else(|| format!("roi/{}/expected/", id));
                buttons.push(ButtonDefinition {
                    id,
                    display_name,
                    parent_states: entry.parent_states,
                    target_state: entry.target_state,
                    template,
                    click_template: entry.click_template,
                    roi: entry.roi,
                    min_confidence: entry.min_confidence,
                    save_cursor: entry.save_cursor,
                    shortcut: entry.shortcut,
                    description: entry.description,
                });
            }
        }
    }
    Ok(buttons)
}

/// Loads all button definitions from YAML configuration (checks buttons.yaml first, then fallback to states.yaml).
pub fn load_buttons_from_config(path: &str) -> Result<Vec<ButtonDefinition>, Box<dyn std::error::Error>> {
    let p = Path::new(path);
    let target_path = if p.ends_with("states.yaml") && Path::new("config/buttons.yaml").exists() {
        Path::new("config/buttons.yaml")
    } else {
        p
    };

    let content = std::fs::read_to_string(target_path)?;
    parse_buttons_from_str(&content)
}
