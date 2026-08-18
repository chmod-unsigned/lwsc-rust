import sys

with open("src/core/action.rs", "r") as f:
    content = f.read()

trigger_sequence = """
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
"""

evaluate_sequence = """
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
            println!("[Sequence] Step {} '{}' timed out after {:.1}s. Aborting sequence '{}'.",
                active_state.current_step_index, step.action, step.timeout_s, seq.name);
            *active_lock = None;
            return None;
        }

        let res = self.execute_single_action(&step.action, current_state, screen, matcher, true);
        if let Some(ref r) = res {
            if r.executed {
                println!("[Sequence] Executed step {} '{}' of '{}'.", active_state.current_step_index, step.action, seq.name);
                if active_state.current_step_index + 1 < seq.steps.len() {
                    let mut new_state = active_state.clone();
                    new_state.current_step_index += 1;
                    new_state.step_start_time = std::time::Instant::now();
                    *active_lock = Some(new_state);
                } else {
                    println!("[Sequence] Sequence '{}' completed.", seq.name);
                    *active_lock = None;
                }
            }
        }
        res
    }

    pub fn has_active_sequence(&self) -> bool {
        self.active_sequence.read().unwrap().is_some()
    }
"""

# Insert these methods inside `impl ActionManager {` block, maybe right before `evaluate`
if "pub fn evaluate(" in content:
    content = content.replace("    pub fn evaluate(", trigger_sequence + evaluate_sequence + "    pub fn evaluate(")
    with open("src/core/action.rs", "w") as f:
        f.write(content)
    print("Methods injected successfully.")
else:
    print("Could not find evaluate method.")

