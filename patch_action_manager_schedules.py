import sys

with open("src/core/action.rs", "r") as f:
    content = f.read()

new_methods = """
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
                    q.push_back(t);
                    println!("[Schedule] It is {}, sequence '{}' queued for execution.", current_time_str, t);
                }
            }
        }
    }
"""

if "pub fn evaluate_schedules" not in content:
    # insert before pub fn trigger_sequence
    content = content.replace("    pub fn trigger_sequence(&self, name: &str) -> bool {", new_methods + "\n    pub fn trigger_sequence(&self, name: &str) -> bool {")

with open("src/core/action.rs", "w") as f:
    f.write(content)
