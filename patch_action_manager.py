import sys

with open("src/core/action.rs", "r") as f:
    content = f.read()

# I will define SequenceSchedules
new_structs = """
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

"""

if "pub struct SequenceSchedules" not in content:
    content = content.replace("#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct SequenceDefinition {", new_structs + "#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct SequenceDefinition {")

# Add schedules to SequenceDefinition
old_seq_def = """pub struct SequenceDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub steps: Vec<SequenceStep>,
}"""

new_seq_def = """pub struct SequenceDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub schedules: Option<SequenceSchedules>,
    #[serde(default)]
    pub steps: Vec<SequenceStep>,
}"""

content = content.replace(old_seq_def, new_seq_def)


old_manager = """pub struct ActionManager {
    actions: RwLock<Vec<ActionDefinition>>,
    sequences: RwLock<Vec<SequenceDefinition>>,
    active_sequence: RwLock<Option<ActiveSequenceState>>,
}"""

new_manager = """use std::collections::{HashMap, VecDeque};
use chrono::{Local, Datelike, Timelike};

pub struct ActionManager {
    actions: RwLock<Vec<ActionDefinition>>,
    sequences: RwLock<Vec<SequenceDefinition>>,
    active_sequence: RwLock<Option<ActiveSequenceState>>,
    sequence_queue: RwLock<VecDeque<String>>,
    sequence_last_run: RwLock<HashMap<String, String>>, // sequence_name -> "YYYY-MM-DD HH:MM"
}"""

content = content.replace(old_manager, new_manager)


old_manager_new = """    pub fn new(actions: Vec<ActionDefinition>, sequences: Vec<SequenceDefinition>) -> Self {
        Self {
            actions: RwLock::new(actions),
            sequences: RwLock::new(sequences),
            active_sequence: RwLock::new(None),
        }
    }"""

new_manager_new = """    pub fn new(actions: Vec<ActionDefinition>, sequences: Vec<SequenceDefinition>) -> Self {
        Self {
            actions: RwLock::new(actions),
            sequences: RwLock::new(sequences),
            active_sequence: RwLock::new(None),
            sequence_queue: RwLock::new(VecDeque::new()),
            sequence_last_run: RwLock::new(HashMap::new()),
        }
    }"""

content = content.replace(old_manager_new, new_manager_new)


with open("src/core/action.rs", "w") as f:
    f.write(content)
