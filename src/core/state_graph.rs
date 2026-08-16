//! Directed State Graph and Dijkstra Pathfinding in Rust.

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

use crate::core::state::{GameState, StateType, StateDefinition, STATE_DEFINITIONS, BUTTON_DEFINITIONS};
use crate::core::button::ButtonDefinition;
pub use crate::core::action::ActionType;

#[derive(Debug, Clone)]
pub struct TransitionEdge {
    pub from_state: GameState,
    pub to_state: GameState,
    pub action_type: ActionType,
    pub template_path: Option<String>,
    pub coords: Option<(f32, f32)>,
    pub key_name: Option<String>,
    pub cost: f32,
    pub wait_after_s: f32,
    pub description: String,
}

impl TransitionEdge {
    pub fn click_template<S1: Into<String>, S2: Into<String>>(
        from: GameState,
        to: GameState,
        template: S1,
        cost: f32,
        wait_after_s: f32,
        desc: S2,
    ) -> Self {
        Self {
            from_state: from,
            to_state: to,
            action_type: ActionType::ClickTemplate,
            template_path: Some(template.into()),
            coords: None,
            key_name: None,
            cost,
            wait_after_s,
            description: desc.into(),
        }
    }

    pub fn escape<S: Into<String>>(from: GameState, to: GameState, wait_after_s: f32, desc: S) -> Self {
        Self {
            from_state: from,
            to_state: to,
            action_type: ActionType::KeyPress,
            template_path: None,
            coords: None,
            key_name: Some("Escape".to_string()),
            cost: 1.0,
            wait_after_s,
            description: desc.into(),
        }
    }

    pub fn key_press<S1: Into<String>, S2: Into<String>>(
        from: GameState,
        to: GameState,
        key: S1,
        cost: f32,
        wait_after_s: f32,
        desc: S2,
    ) -> Self {
        Self {
            from_state: from,
            to_state: to,
            action_type: ActionType::KeyPress,
            template_path: None,
            coords: None,
            key_name: Some(key.into()),
            cost,
            wait_after_s,
            description: desc.into(),
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
struct StateNode {
    cost: f32,
    state: GameState,
}

impl Eq for StateNode {}

impl Ord for StateNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for StateNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Default)]
pub struct StateGraph {
    edges: HashMap<GameState, Vec<TransitionEdge>>,
}

impl StateGraph {
    /// Constructs state graph using global lazy-loaded STATE_DEFINITIONS and BUTTON_DEFINITIONS.
    pub fn new() -> Self {
        Self::from_definitions(&STATE_DEFINITIONS, &BUTTON_DEFINITIONS)
    }

    /// Dynamically constructs the state navigation graph from states and buttons definitions.
    pub fn from_definitions(states: &[StateDefinition], buttons: &[ButtonDefinition]) -> Self {
        let mut graph = Self {
            edges: HashMap::new(),
        };

        // 1. Root state toggles (BASE <-> AREA, BASE <-> WORLD_MAP)
        graph.add_transition(TransitionEdge::click_template(
            GameState::Base,
            GameState::Area,
            "roi/BASE/expected.png",
            1.0,
            0.8,
            "Toggle World button to enter Area view",
        ));
        graph.add_transition(TransitionEdge::click_template(
            GameState::Area,
            GameState::Base,
            "roi/AREA/expected.png",
            1.0,
            0.8,
            "Toggle Base button to return to Base",
        ));
        graph.add_transition(TransitionEdge::click_template(
            GameState::Base,
            GameState::WorldMap,
            "roi/BASE/expected.png",
            1.0,
            0.8,
            "Toggle World button to enter World Map",
        ));
        graph.add_transition(TransitionEdge::click_template(
            GameState::WorldMap,
            GameState::Base,
            "roi/WORLD_MAP/expected.png",
            1.0,
            0.8,
            "Toggle Base button to return to Base",
        ));

        // 2. Button transitions: parent_states -> target_state
        for btn in buttons {
            if let Some(target) = btn.target_state {
                for parent in &btn.parent_states {
                    graph.add_transition(TransitionEdge::click_template(
                        *parent,
                        target,
                        &btn.template,
                        1.0,
                        0.5,
                        format!("Open {} via {}", target.name(), btn.display_name),
                    ));
                }
            }
        }

        // 3. State transitions for modals / sub_modals with templates attached to parent
        for state_def in states {
            if (state_def.state_type == StateType::Modal || state_def.state_type == StateType::SubModal)
                && !state_def.templates.is_empty()
            {
                for parent in state_def.parent_states() {
                    let has_button = buttons.iter().any(|b| b.target_state == Some(state_def.state) && b.parent_states.contains(&parent));
                    if !has_button {
                        if let Some(tmpl) = state_def.templates.first() {
                            graph.add_transition(TransitionEdge::click_template(
                                parent,
                                state_def.state,
                                tmpl,
                                1.0,
                                0.5,
                                format!("Open {} from {}", state_def.state.name(), parent.name()),
                            ));
                        }
                    }
                }
            }
        }

        // 4. Modal / SubModal close transitions: state -> parent (via Escape)
        for state_def in states {
            if state_def.state_type == StateType::Modal || state_def.state_type == StateType::SubModal {
                for parent in state_def.parent_states() {
                    graph.add_transition(TransitionEdge::escape(
                        state_def.state,
                        parent,
                        0.4,
                        format!("Close {} to {} with Escape", state_def.state.name(), parent.name()),
                    ));
                }
            }
        }

        // 5. Sub-modal tab switches within parent modal (e.g. SEARCH_SPECIAL <-> SEARCH)
        for state_def in states {
            if state_def.state_type == StateType::SubModal {
                for parent in state_def.parent_states() {
                    if let Some(parent_def) = states.iter().find(|s| s.state == parent) {
                        if let Some(parent_tmpl) = parent_def.templates.first() {
                            graph.add_transition(TransitionEdge::click_template(
                                state_def.state,
                                parent,
                                parent_tmpl,
                                0.5,
                                0.3,
                                format!("Switch tab from {} to {}", state_def.state.name(), parent.name()),
                            ));
                        }
                    }
                }
            }
        }

        graph
    }

    pub fn add_transition(&mut self, edge: TransitionEdge) {
        self.edges.entry(edge.from_state).or_default().push(edge);
    }

    pub fn get_transitions(&self, state: GameState) -> &[TransitionEdge] {
        self.edges.get(&state).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn find_path(&self, start: GameState, goal: GameState) -> Option<Vec<TransitionEdge>> {
        if start == goal {
            return Some(Vec::new());
        }

        let mut distances: HashMap<GameState, f32> = HashMap::new();
        let mut previous: HashMap<GameState, (GameState, TransitionEdge)> = HashMap::new();
        let mut heap = BinaryHeap::new();

        distances.insert(start, 0.0);
        heap.push(StateNode { cost: 0.0, state: start });

        while let Some(StateNode { cost, state }) = heap.pop() {
            if state == goal {
                // Reconstruct path
                let mut path = Vec::new();
                let mut curr = goal;
                while let Some((prev_state, edge)) = previous.get(&curr) {
                    path.push(edge.clone());
                    curr = *prev_state;
                }
                path.reverse();
                return Some(path);
            }

            if cost > *distances.get(&state).unwrap_or(&f32::INFINITY) {
                continue;
            }

            for edge in self.get_transitions(state) {
                let next_state = edge.to_state;
                let next_cost = cost + edge.cost;

                if next_cost < *distances.get(&next_state).unwrap_or(&f32::INFINITY) {
                    distances.insert(next_state, next_cost);
                    previous.insert(next_state, (state, edge.clone()));
                    heap.push(StateNode { cost: next_cost, state: next_state });
                }
            }
        }

        None
    }
}
