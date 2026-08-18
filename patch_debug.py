import sys

with open("src/core/action.rs", "r") as f:
    content = f.read()

old_code = """
            } else {
                // Throttle debug prints so we don't spam the console every 25ms.
                // We'll print only if it's the very first evaluation of this step, or once every second.
                let elapsed_ms = active_state.step_start_time.elapsed().as_millis();
                if elapsed_ms < 50 || elapsed_ms % 1000 < 50 {
                    println!("[Sequence Debug] Step '{}' waiting: {}", step.action, r.reason);
                }
            }
        } else {
            let elapsed_ms = active_state.step_start_time.elapsed().as_millis();
            if elapsed_ms < 50 || elapsed_ms % 1000 < 50 {
                println!("[Sequence Debug] Step '{}' waiting (No ActionExecutionResult returned).", step.action);
            }
        }
"""

new_code = """
            } else {
                println!("[Sequence Debug] Step '{}' waiting: {}", step.action, r.reason);
            }
        } else {
            println!("[Sequence Debug] Step '{}' waiting (No ActionExecutionResult returned).", step.action);
        }
"""

if old_code.strip() in content:
    content = content.replace(old_code, new_code)
else:
    # manual replace
    pass

with open("src/core/action.rs", "w") as f:
    f.write(content.replace(old_code.strip(), new_code.strip()))
