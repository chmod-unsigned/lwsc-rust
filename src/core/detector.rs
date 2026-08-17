//! Hierarchical Game State & Button Detector in Rust.

use std::path::Path;
use image::RgbaImage;

use crate::core::state::{
    GameState, StateType, StateDefinition, STATE_DEFINITIONS,
};
use crate::core::button::{
    ButtonDefinition, ButtonDetection, load_buttons_from_config,
};
use crate::vision::matching::TemplateMatcher;

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub state: GameState,
    pub state_type: StateType,
    pub root_state: Option<GameState>,
    pub modal_state: Option<GameState>,
    pub visible_buttons: Vec<ButtonDetection>,
    pub confidence: f32,
    pub matched_template: Option<String>,
    pub match_box: Option<(u32, u32, u32, u32)>,
    pub match_center: Option<(u32, u32)>,
    pub display_name: String,
}

pub struct StateDetector {
    pub matcher: TemplateMatcher,
    pub definitions: Vec<StateDefinition>,
    pub buttons: Vec<ButtonDefinition>,
}

impl StateDetector {
    pub fn new<P: AsRef<Path>>(asset_root: P) -> Self {
        let buttons = load_buttons_from_config("config/states.yaml").unwrap_or_default();
        Self {
            matcher: TemplateMatcher::new(asset_root),
            definitions: STATE_DEFINITIONS.clone(),
            buttons,
        }
    }

    pub fn with_definitions<P: AsRef<Path>>(
        asset_root: P,
        definitions: Vec<StateDefinition>,
        buttons: Vec<ButtonDefinition>,
    ) -> Self {
        Self {
            matcher: TemplateMatcher::new(asset_root),
            definitions,
            buttons,
        }
    }

    /// Fast root state detection evaluated strictly against Root layer states (Base, Area, WorldMap)
    /// using their respective bottom-right ROIs, picking the candidate where all templates match.
    pub fn detect_root(&mut self, screen: &RgbaImage) -> Option<DetectionResult> {
        let (w, h) = screen.dimensions();
        let mut best: Option<DetectionResult> = None;

        for defn in &self.definitions {
            if defn.state_type != StateType::Root {
                continue;
            }

            let templates = defn.resolved_templates();
            if templates.is_empty() {
                continue;
            }

            let roi_px = defn.roi.map(|r| r.to_pixel_box(w, h));

            let mut all_matched = true;
            let mut min_confidence = 1.0f32;
            let mut primary_match = None;

            for template in &templates {
                let match_res = self.matcher.find_match(
                    screen,
                    template,
                    defn.min_confidence,
                    roi_px,
                );

                if !match_res.matched {
                    all_matched = false;
                    break;
                }

                if match_res.confidence < min_confidence {
                    min_confidence = match_res.confidence;
                }
                if primary_match.is_none() {
                    primary_match = Some(match_res);
                }
            }

            if all_matched {
                if let Some(match_res) = primary_match {
                    let is_better = best.as_ref().map(|b| min_confidence > b.confidence).unwrap_or(true);
                    if is_better {
                        best = Some(DetectionResult {
                            state: defn.state,
                            state_type: defn.state_type,
                            root_state: Some(defn.state),
                            modal_state: None,
                            visible_buttons: Vec::new(),
                            confidence: min_confidence,
                            matched_template: Some(templates.join(", ")),
                            match_box: Some((
                                match_res.box_x,
                                match_res.box_y,
                                match_res.width,
                                match_res.height,
                            )),
                            match_center: Some((match_res.center_x, match_res.center_y)),
                            display_name: defn.display_name.clone(),
                        });
                    }
                }
            }
        }
        best
    }

    /// Detect all visible buttons currently present on screen for the given or current screen state.
    pub fn detect_buttons(&mut self, screen: &RgbaImage, active_state: Option<GameState>) -> Vec<ButtonDetection> {
        let (w, h) = screen.dimensions();
        let mut detected = Vec::new();

        for btn in &self.buttons {
            // If active_state is known, check if this button can appear on it
            if let Some(state) = active_state {
                if !btn.parent_states.is_empty() && !btn.parent_states.contains(&state) {
                    continue;
                }
            }

            let templates = btn.resolved_templates();
            if templates.is_empty() {
                continue;
            }

            let roi_px = btn.roi.map(|r| r.to_pixel_box(w, h));
            let mut all_matched = true;
            let mut min_confidence = 1.0f32;
            let mut matches = Vec::new();

            for template in &templates {
                let match_res = self.matcher.find_match(
                    screen,
                    template,
                    btn.min_confidence,
                    roi_px,
                );

                if !match_res.matched {
                    all_matched = false;
                    break;
                }

                if match_res.confidence < min_confidence {
                    min_confidence = match_res.confidence;
                }
                matches.push((template.clone(), match_res));
            }

            if all_matched && !matches.is_empty() {
                if let Some(click_match) = crate::core::button::select_click_match(&matches, btn.click_template.as_deref()) {
                    detected.push(ButtonDetection {
                        id: btn.id.clone(),
                        display_name: btn.display_name.clone(),
                        target_state: btn.target_state,
                        confidence: min_confidence,
                        matched_template: templates.join(", "),
                        match_box: (
                            click_match.box_x,
                            click_match.box_y,
                            click_match.width,
                            click_match.height,
                        ),
                        match_center: (click_match.center_x as i32, click_match.center_y as i32),
                        save_cursor: btn.save_cursor,
                    });
                }
            }
        }

        detected
    }

    pub fn is_in_state(&mut self, screen: &RgbaImage, expected: GameState) -> bool {
        let defn = match self.definitions.iter().find(|d| d.state == expected) {
            Some(d) => d,
            None => return false,
        };

        let templates = defn.resolved_templates();
        if templates.is_empty() {
            return false;
        }

        let (w, h) = screen.dimensions();
        let roi_px = defn.roi.map(|r| r.to_pixel_box(w, h));

        for template in &templates {
            let res = self.matcher.find_match(screen, template, defn.min_confidence, roi_px);
            if !res.matched {
                return false;
            }
        }
        true
    }

    /// Complete state detection pass that always determines the active root game state (Base, Area, WorldMap),
    /// checks if any modal/sub-modal overlay is active on top of it, and detects all visible buttons.
    pub fn detect(&mut self, screen: &RgbaImage) -> DetectionResult {
        self.detect_with_context(screen, None)
    }

    pub fn detect_with_context(&mut self, screen: &RgbaImage, last_known_root: Option<GameState>) -> DetectionResult {
        let (w, h) = screen.dimensions();

        // 1. Always evaluate Root screen (Base, Area, WorldMap)
        let detected_root_res = self.detect_root(screen);
        let resolved_root = detected_root_res.as_ref().map(|r| r.state).or(last_known_root);

        // 2. Check for overlays: Popup -> SubModal -> Modal
        let overlay_layers = [
            StateType::Popup,
            StateType::SubModal,
            StateType::Modal,
        ];

        for layer in overlay_layers {
            let mut best_in_layer: Option<DetectionResult> = None;

            for defn in &self.definitions {
                if defn.state_type != layer {
                    continue;
                }

                let templates = defn.resolved_templates();
                if templates.is_empty() {
                    continue;
                }

                let roi_px = defn.roi.map(|r| r.to_pixel_box(w, h));

                let mut all_matched = true;
                let mut min_confidence = 1.0f32;
                let mut primary_match = None;

                for template in &templates {
                    let match_res = self.matcher.find_match(
                        screen,
                        template,
                        defn.min_confidence,
                        roi_px,
                    );

                    if !match_res.matched {
                        all_matched = false;
                        break;
                    }

                    if match_res.confidence < min_confidence {
                        min_confidence = match_res.confidence;
                    }
                    if primary_match.is_none() {
                        primary_match = Some(match_res);
                    }
                }

                if all_matched {
                    if let Some(match_res) = primary_match {
                        let is_better = best_in_layer.as_ref().map(|b| min_confidence > b.confidence).unwrap_or(true);
                        if is_better {
                            let root_for_modal = resolved_root.or_else(|| defn.state.root_state());
                            best_in_layer = Some(DetectionResult {
                                state: defn.state,
                                state_type: defn.state_type,
                                root_state: root_for_modal,
                                modal_state: Some(defn.state),
                                visible_buttons: Vec::new(),
                                confidence: min_confidence,
                                matched_template: Some(templates.join(", ")),
                                match_box: Some((
                                    match_res.box_x,
                                    match_res.box_y,
                                    match_res.width,
                                    match_res.height,
                                )),
                                match_center: Some((match_res.center_x, match_res.center_y)),
                                display_name: defn.display_name.clone(),
                            });
                        }
                    }
                }
            }

            if let Some(mut matched) = best_in_layer {
                matched.visible_buttons = self.detect_buttons(screen, Some(matched.state));
                return matched;
            }
        }

        // 3. If no modal is active, return the detected root state with visible buttons matching the root state
        if let Some(mut root_res) = detected_root_res {
            root_res.modal_state = None;
            root_res.visible_buttons = self.detect_buttons(screen, Some(root_res.state));
            return root_res;
        }

        // 4. Fallback if completely unknown
        let visible_buttons = self.detect_buttons(screen, resolved_root);
        DetectionResult {
            state: resolved_root.unwrap_or(GameState::Unknown),
            state_type: if resolved_root.is_some() { StateType::Root } else { StateType::Special },
            root_state: resolved_root,
            modal_state: None,
            visible_buttons,
            confidence: 0.0,
            matched_template: None,
            match_box: None,
            match_center: None,
            display_name: resolved_root.map(|r| r.name().to_string()).unwrap_or_else(|| "Unknown".to_string()),
        }
    }
}
