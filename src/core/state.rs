//! Game state definitions, metadata, and YAML configuration for Last War: Survival in Rust.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::path::Path;
use std::sync::LazyLock;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DEFAULT_STATES_YAML: &str = include_str!("../../config/states.yaml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameState {
    Unknown,
    Loading,

    // Root States (Base gameplay screens)
    Base,
    WorldMap,
    Area,

    // Modals / Overlay Menus
    MainShop,
    MainShopHotDeals,
    MainShopMall,
    MainShopMallHotSalePacks,
    MainShopMallOfficialEvent,
    MainShopMallWeeklyDeal,
    Alliance,
    AllianceGiftsRegular,
    AllianceGiftsPremium,
    Search,
    SearchSpecial,
    RadarTasks,
    Mail,
    Loot,
    Inventory,
    InventorySpecial,
}

impl GameState {
    pub const HEADQUARTER: GameState = GameState::Base;
    pub const MAP: GameState = GameState::WorldMap;
    pub const ALLIANCE_GIFT: GameState = GameState::AllianceGiftsRegular;
    pub const ALLIANCE_GIFTS: GameState = GameState::AllianceGiftsRegular;

    pub const ROOT_STATES: &'static [GameState] = &[GameState::Base, GameState::WorldMap, GameState::Area];

    /// Returns true if this state is one of the base Root screens (Base, WorldMap, or Area).
    pub fn is_root(&self) -> bool {
        matches!(self, GameState::Base | GameState::WorldMap | GameState::Area)
    }

    /// Resolves the root game state (Base, WorldMap, or Area) by traversing the parent hierarchy.
    pub fn root_state(&self) -> Option<GameState> {
        if self.is_root() {
            return Some(*self);
        }

        let mut visited = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(*self);

        while let Some(curr) = queue.pop_front() {
            if curr.is_root() {
                return Some(curr);
            }
            if visited.contains(&curr) {
                continue;
            }
            visited.push(curr);

            if let Some(def) = get_state_definition(curr) {
                for parent in def.parent_states() {
                    if parent.is_root() {
                        return Some(parent);
                    }
                    queue.push_back(parent);
                }
            }
        }

        None
    }

    pub fn name(&self) -> &'static str {
        match self {
            GameState::Unknown => "UNKNOWN",
            GameState::Loading => "LOADING",
            GameState::Base => "BASE",
            GameState::WorldMap => "WORLD_MAP",
            GameState::Area => "AREA",
            GameState::MainShop => "MAIN_SHOP",
            GameState::MainShopHotDeals => "MAIN_SHOP_HOT_DEALS",
            GameState::MainShopMall => "MAIN_SHOP_MALL",
            GameState::MainShopMallHotSalePacks => "MAIN_SHOP_MALL_HOT_SALE_PACKS",
            GameState::MainShopMallOfficialEvent => "MAIN_SHOP_MALL_OFFICIAL_EVENT",
            GameState::MainShopMallWeeklyDeal => "MAIN_SHOP_MALL_WEEKLY_DEAL",
            GameState::Alliance => "ALLIANCE",
            GameState::AllianceGiftsRegular => "ALLIANCE_GIFTS_REGULAR",
            GameState::AllianceGiftsPremium => "ALLIANCE_GIFTS_PREMIUM",
            GameState::Search => "SEARCH",
            GameState::SearchSpecial => "SEARCH_SPECIAL",
            GameState::RadarTasks => "RADAR_TASKS",
            GameState::Mail => "MAIL",
            GameState::Loot => "LOOT",
            GameState::Inventory => "INVENTORY",
            GameState::InventorySpecial => "INVENTORY_SPECIAL",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "UNKNOWN" => Some(GameState::Unknown),
            "LOADING" => Some(GameState::Loading),
            "BASE" | "HEADQUARTER" => Some(GameState::Base),
            "WORLD_MAP" | "MAP" => Some(GameState::WorldMap),
            "AREA" => Some(GameState::Area),
            "MAIN_SHOP" => Some(GameState::MainShop),
            "MAIN_SHOP_HOT_DEALS" => Some(GameState::MainShopHotDeals),
            "MAIN_SHOP_MALL" => Some(GameState::MainShopMall),
            "MAIN_SHOP_MALL_HOT_SALE_PACKS" | "MAIN_SHOP_HOT_SALE_PACKS" => Some(GameState::MainShopMallHotSalePacks),
            "MAIN_SHOP_MALL_OFFICIAL_EVENT" | "MAIN_SHOP_MAIN_OFFICIAL_EVENT" | "MAIN_SHOP_OFFICIAL_EVENT" => Some(GameState::MainShopMallOfficialEvent),
            "MAIN_SHOP_MALL_WEEKLY_DEAL" | "MAIN_SHOP_WEEKLY_DEAL" | "MAIN_SHOP_MALL_WEEKLY_DEALS" => Some(GameState::MainShopMallWeeklyDeal),
            "ALLIANCE" => Some(GameState::Alliance),
            "ALLIANCE_GIFTS_REGULAR" | "ALLIANCE_GIFTS" | "ALLIANCE_GIFT" => Some(GameState::AllianceGiftsRegular),
            "ALLIANCE_GIFTS_PREMIUM" | "ALLIANCE_GIFTS_RARE" | "ALLIANCE_GIFT_PREMIUM" => Some(GameState::AllianceGiftsPremium),
            "SEARCH" => Some(GameState::Search),
            "SEARCH_SPECIAL" => Some(GameState::SearchSpecial),
            "RADAR_TASKS" | "RADAR_TASK" => Some(GameState::RadarTasks),
            "MAIL" => Some(GameState::Mail),
            "LOOT" => Some(GameState::Loot),
            "INVENTORY" => Some(GameState::Inventory),
            "INVENTORY_SPECIAL" => Some(GameState::InventorySpecial),
            _ => None,
        }
    }
}

impl Serialize for GameState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for GameState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        GameState::from_str(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown game state: {}", s)))
    }
}

impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateType {
    Root,
    Modal,
    SubModal,
    Popup,
    Button,
    Special,
}

impl StateType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StateType::Root => "root",
            StateType::Modal => "modal",
            StateType::SubModal => "sub_modal",
            StateType::Popup => "popup",
            StateType::Button => "button",
            StateType::Special => "special",
        }
    }

    pub fn is_root(&self) -> bool {
        matches!(self, StateType::Root)
    }

    pub fn is_modal(&self) -> bool {
        matches!(self, StateType::Modal | StateType::SubModal | StateType::Popup)
    }

    pub fn is_button(&self) -> bool {
        matches!(self, StateType::Button)
    }
}

impl Serialize for StateType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for StateType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "root" => Ok(StateType::Root),
            "modal" => Ok(StateType::Modal),
            "sub_modal" | "submodal" => Ok(StateType::SubModal),
            "popup" => Ok(StateType::Popup),
            "button" | "btn" => Ok(StateType::Button),
            "special" => Ok(StateType::Special),
            _ => Err(serde::de::Error::custom(format!("unknown state type: {}", s))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedROI {
    pub ymin: f32,
    pub ymax: f32,
    pub xmin: f32,
    pub xmax: f32,
}

impl NormalizedROI {
    pub const fn new(ymin: f32, ymax: f32, xmin: f32, xmax: f32) -> Self {
        Self { ymin, ymax, xmin, xmax }
    }

    pub fn to_pixel_box(&self, img_w: u32, img_h: u32) -> (u32, u32, u32, u32) {
        let x1 = (self.xmin * img_w as f32) as u32;
        let y1 = (self.ymin * img_h as f32) as u32;
        let x2 = (self.xmax * img_w as f32).min(img_w as f32) as u32;
        let y2 = (self.ymax * img_h as f32).min(img_h as f32) as u32;
        (x1, y1, x2, y2)
    }
}

pub fn default_min_confidence() -> f32 {
    0.80
}

pub fn deserialize_parent_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ParentHelper {
        Null,
        Single(String),
        Multiple(Vec<String>),
    }

    let opt: Option<ParentHelper> = Option::deserialize(deserializer)?;
    match opt {
        None | Some(ParentHelper::Null) => Ok(Vec::new()),
        Some(ParentHelper::Single(s)) => Ok(vec![s]),
        Some(ParentHelper::Multiple(v)) => Ok(v),
    }
}

pub fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        Single(String),
        Multiple(Vec<String>),
    }

    let opt: Option<StringOrVec> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(Vec::new()),
        Some(StringOrVec::Single(s)) => Ok(vec![s]),
        Some(StringOrVec::Multiple(v)) => Ok(v),
    }
}

pub fn deserialize_gamestates_or_single<'de, D>(deserializer: D) -> Result<Vec<GameState>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SingleOrVec {
        Single(GameState),
        Multiple(Vec<GameState>),
    }

    let opt: Option<SingleOrVec> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(Vec::new()),
        Some(SingleOrVec::Single(s)) => Ok(vec![s]),
        Some(SingleOrVec::Multiple(v)) => Ok(v),
    }
}

fn default_state_type() -> StateType {
    StateType::Modal
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDefinitionEntry {
    #[serde(default)]
    pub state: Option<GameState>,
    #[serde(rename = "type", default = "default_state_type")]
    pub state_type: StateType,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub templates: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_parent_names", rename = "parent")]
    pub parent_names: Vec<String>,
    #[serde(default)]
    pub roi: Option<NormalizedROI>,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDefinition {
    pub state: GameState,
    #[serde(rename = "type")]
    pub state_type: StateType,
    pub display_name: String,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub templates: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_parent_names", rename = "parent")]
    pub parent_names: Vec<String>,
    #[serde(default)]
    pub roi: Option<NormalizedROI>,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    #[serde(default)]
    pub description: String,
}

impl StateDefinition {
    /// Resolves all template image file paths for this state.
    /// Expands any directory paths (e.g. `roi/<STATE>/expected/`) into all contained image files (`.png`, `.jpg`, `.jpeg`), sorted.
    pub fn resolved_templates(&self) -> Vec<String> {
        let mut results = Vec::new();

        for tmpl in &self.templates {
            let p = Path::new(tmpl);
            if p.is_dir() {
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
            } else {
                results.push(tmpl.clone());
            }
        }

        results.sort();
        results
    }

    pub fn parent_states(&self) -> Vec<GameState> {
        let mut resolved = Vec::new();
        for name in &self.parent_names {
            if let Some(st) = GameState::from_str(name) {
                resolved.push(st);
            } else if let Some(btn) = get_button_definition(name) {
                resolved.extend(btn.parent_states);
            }
        }
        resolved
    }

    pub fn parent_state(&self) -> Option<GameState> {
        self.parent_states().into_iter().next()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    #[serde(default = "default_toggle_pause")]
    pub toggle_pause: String,
    #[serde(default = "default_open_config")]
    pub open_config: String,
    #[serde(default = "default_force_detect")]
    pub force_detect: String,
    #[serde(default = "default_show_help")]
    pub show_help: String,
}

fn default_toggle_pause() -> String {
    "ctrl+p".to_string()
}
fn default_open_config() -> String {
    "ctrl+o".to_string()
}
fn default_force_detect() -> String {
    "ctrl+s".to_string()
}
fn default_show_help() -> String {
    "ctrl+h".to_string()
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            toggle_pause: default_toggle_pause(),
            open_config: default_open_config(),
            force_detect: default_force_detect(),
            show_help: default_show_help(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StatesContent {
    WrapperMap {
        states: BTreeMap<String, StateDefinitionEntry>,
    },
    WrapperList {
        states: Vec<StateDefinition>,
    },
    DirectMap(BTreeMap<String, StateDefinitionEntry>),
    DirectList(Vec<StateDefinition>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ShortcutsContent {
    Wrapper {
        shortcuts: ShortcutsConfig,
    },
    Direct(ShortcutsConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatesConfigFile {
    #[serde(default)]
    pub shortcuts: ShortcutsConfig,
    #[serde(default)]
    pub states: Vec<StateDefinition>,
    #[serde(default)]
    pub buttons: Vec<crate::core::button::ButtonDefinition>,
    #[serde(default)]
    pub actions: Vec<crate::core::action::ActionDefinition>,
    #[serde(default)]
    pub sequences: Vec<crate::core::action::SequenceDefinition>,
}

pub static STATE_DEFINITIONS: LazyLock<Vec<StateDefinition>> = LazyLock::new(|| {
    load_state_definitions_or_default(Some("config/states.yaml"))
});

pub static BUTTON_DEFINITIONS: LazyLock<Vec<crate::core::button::ButtonDefinition>> = LazyLock::new(|| {
    crate::core::button::load_buttons_from_config("config/buttons.yaml")
        .or_else(|_| crate::core::button::load_buttons_from_config("config/states.yaml"))
        .unwrap_or_default()
});

pub static SHORTCUTS_CONFIG: LazyLock<ShortcutsConfig> = LazyLock::new(|| {
    load_shortcuts_from_config("config/shortcuts.yaml")
});

pub fn get_state_definition(state: GameState) -> Option<StateDefinition> {
    STATE_DEFINITIONS.iter().find(|def| def.state == state).cloned()
}

pub fn get_button_definition(id: &str) -> Option<crate::core::button::ButtonDefinition> {
    BUTTON_DEFINITIONS.iter().find(|b| b.id.eq_ignore_ascii_case(id)).cloned()
}

pub fn load_shortcuts_from_config<P: AsRef<Path>>(path: P) -> ShortcutsConfig {
    let p = path.as_ref();
    let candidates = [
        p.to_path_buf(),
        Path::new("config/shortcuts.yaml").to_path_buf(),
        Path::new("config/states.yaml").to_path_buf(),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(candidate) {
                if let Ok(parsed) = serde_yaml::from_str::<ShortcutsContent>(&content) {
                    return match parsed {
                        ShortcutsContent::Wrapper { shortcuts } => shortcuts,
                        ShortcutsContent::Direct(shortcuts) => shortcuts,
                    };
                }
            }
        }
    }
    ShortcutsConfig::default()
}

pub fn parse_states_from_str(content: &str) -> Result<Vec<StateDefinition>, Box<dyn std::error::Error>> {
    let content_parsed: StatesContent = serde_yaml::from_str(content)?;
    let mut states = Vec::new();
    match content_parsed {
        StatesContent::WrapperList { states: list } | StatesContent::DirectList(list) => {
            states = list;
        }
        StatesContent::WrapperMap { states: map } | StatesContent::DirectMap(map) => {
            for (key, entry) in map {
                let st = entry.state.or_else(|| GameState::from_str(&key)).unwrap_or(GameState::Unknown);
                let display_name = entry.display_name.unwrap_or_else(|| key.clone());
                let description = entry.description.unwrap_or_default();
                states.push(StateDefinition {
                    state: st,
                    state_type: entry.state_type,
                    display_name,
                    templates: entry.templates,
                    parent_names: entry.parent_names,
                    roi: entry.roi,
                    min_confidence: entry.min_confidence,
                    description,
                });
            }
        }
    }
    Ok(states)
}

pub fn load_state_definitions<P: AsRef<Path>>(path: P) -> Result<Vec<StateDefinition>, Box<dyn std::error::Error>> {
    let p = path.as_ref();
    if p.exists() {
        let content = std::fs::read_to_string(p)?;
        return parse_states_from_str(&content);
    }
    parse_states_from_str(DEFAULT_STATES_YAML)
}

pub fn load_actions_from_config<P: AsRef<Path>>(path: P) -> Result<Vec<crate::core::action::ActionDefinition>, Box<dyn std::error::Error>> {
    let p = path.as_ref();
    let actions_path = if (p.ends_with("states.yaml") || p.ends_with("sequences.yaml") || p.ends_with("buttons.yaml")) && Path::new("config/actions.yaml").exists() {
        Path::new("config/actions.yaml")
    } else if Path::new("config/actions.yaml").exists() && !p.ends_with("actions.yaml") {
        Path::new("config/actions.yaml")
    } else {
        p
    };

    let content = std::fs::read_to_string(actions_path)?;
    let mut actions = crate::core::action::parse_actions_from_str(&content)?;
    
    // Resolve with button definitions
    let buttons = crate::core::button::load_buttons_from_config("config/buttons.yaml")
        .or_else(|_| crate::core::button::load_buttons_from_config("config/states.yaml"))
        .unwrap_or_default();

    for action in actions.iter_mut() {
        action.resolve_button(&buttons);
    }
    Ok(actions)
}

pub fn load_sequences_from_config<P: AsRef<Path>>(path: P) -> Result<Vec<crate::core::action::SequenceDefinition>, Box<dyn std::error::Error>> {
    let p = path.as_ref();
    let sequences_path = if (p.ends_with("states.yaml") || p.ends_with("actions.yaml") || p.ends_with("buttons.yaml")) && Path::new("config/sequences.yaml").exists() {
        Path::new("config/sequences.yaml")
    } else if Path::new("config/sequences.yaml").exists() && !p.ends_with("sequences.yaml") {
        Path::new("config/sequences.yaml")
    } else {
        p
    };

    let content = std::fs::read_to_string(sequences_path)?;
    crate::core::action::parse_sequences_from_str(&content)
}

pub fn load_state_definitions_or_default(custom_path: Option<&str>) -> Vec<StateDefinition> {
    if let Some(p) = custom_path {
        if Path::new(p).exists() {
            if let Ok(defs) = load_state_definitions(p) {
                return defs;
            }
        }
    }
    load_state_definitions("config/states.yaml").unwrap_or_default()
}
