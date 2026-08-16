use lwsc2::core::{GameState, StateGraph};

#[test]
fn test_state_graph_pathfinding() {
    let graph = StateGraph::new();
    
    // Direct
    let path = graph.find_path(GameState::Base, GameState::WorldMap).expect("path should exist");
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].from_state, GameState::Base);
    assert_eq!(path[0].to_state, GameState::WorldMap);

    // Multi-step: Base -> WorldMap/Area -> Search -> SearchSpecial
    let path2 = graph.find_path(GameState::Base, GameState::SearchSpecial).expect("path should exist");
    assert_eq!(path2.len(), 3);
    assert!(path2[0].to_state == GameState::WorldMap || path2[0].to_state == GameState::Area);
    assert_eq!(path2[1].to_state, GameState::Search);
    assert_eq!(path2[2].to_state, GameState::SearchSpecial);
}

#[test]
fn test_state_from_str() {
    assert_eq!(GameState::from_str("BASE"), Some(GameState::Base));
    assert_eq!(GameState::from_str("world_map"), Some(GameState::WorldMap));
    assert_eq!(GameState::from_str("AREA"), Some(GameState::Area));
    assert_eq!(GameState::from_str("MAIN_SHOP"), Some(GameState::MainShop));
    assert_eq!(GameState::from_str("ALLIANCE"), Some(GameState::Alliance));
    assert_eq!(GameState::from_str("ALLIANCE_GIFTS_REGULAR"), Some(GameState::AllianceGiftsRegular));
    assert_eq!(GameState::from_str("ALLIANCE_GIFTS_PREMIUM"), Some(GameState::AllianceGiftsPremium));
    assert_eq!(GameState::from_str("ALLIANCE_GIFTS_RARE"), Some(GameState::AllianceGiftsPremium));
    assert_eq!(GameState::from_str("ALLIANCE_GIFTS"), Some(GameState::AllianceGiftsRegular));
    assert_eq!(GameState::from_str("ALLIANCE_GIFT"), Some(GameState::AllianceGiftsRegular));
    assert_eq!(GameState::from_str("RADAR_TASKS"), Some(GameState::RadarTasks));
    assert_eq!(GameState::from_str("RADAR_TASK"), Some(GameState::RadarTasks));
    assert_eq!(GameState::from_str("INVALID"), None);
}

#[test]
fn test_root_state_properties() {
    assert!(GameState::Base.is_root());
    assert!(GameState::WorldMap.is_root());
    assert!(GameState::Area.is_root());
    assert!(!GameState::MainShop.is_root());
    assert!(!GameState::Alliance.is_root());
    assert!(!GameState::Mail.is_root());
    assert!(!GameState::AllianceGiftsRegular.is_root());
    assert!(!GameState::AllianceGiftsPremium.is_root());
    assert!(!GameState::Search.is_root());
    assert!(!GameState::SearchSpecial.is_root());
    assert!(!GameState::RadarTasks.is_root());
    assert!(!GameState::Unknown.is_root());

    assert_eq!(GameState::ROOT_STATES, &[GameState::Base, GameState::WorldMap, GameState::Area]);
}

#[test]
fn test_root_state_resolution() {
    // Root states resolve to themselves
    assert_eq!(GameState::Base.root_state(), Some(GameState::Base));
    assert_eq!(GameState::WorldMap.root_state(), Some(GameState::WorldMap));
    assert_eq!(GameState::Area.root_state(), Some(GameState::Area));

    // Modals attached to Base / Area
    assert_eq!(GameState::MainShop.root_state(), Some(GameState::Base));
    assert_eq!(GameState::MainShopHotDeals.root_state(), Some(GameState::Base));
    assert_eq!(GameState::MainShopMall.root_state(), Some(GameState::Base));
    assert_eq!(GameState::Alliance.root_state(), Some(GameState::Base));
    assert_eq!(GameState::AllianceGiftsRegular.root_state(), Some(GameState::Base));
    assert_eq!(GameState::AllianceGiftsPremium.root_state(), Some(GameState::Base));
    assert_eq!(GameState::Mail.root_state(), Some(GameState::Base));
    assert_eq!(GameState::RadarTasks.root_state(), Some(GameState::Base));

    // Modals attached to Area
    assert_eq!(GameState::Search.root_state(), Some(GameState::Area));
    assert_eq!(GameState::SearchSpecial.root_state(), Some(GameState::Area));

    // Unknown has no root
    assert_eq!(GameState::Unknown.root_state(), None);
}

#[test]
fn test_button_definitions_loading() {
    let buttons = lwsc2::core::load_buttons_from_config("config/states.yaml").expect("buttons should load");
    assert!(!buttons.is_empty());
    
    let help_btn = buttons.iter().find(|b| b.id == "HELP").expect("HELP button should exist");
    assert_eq!(help_btn.parent_states, vec![GameState::Base, GameState::Area]);
    assert_eq!(help_btn.target_state, None);
    assert!(help_btn.save_cursor);

    let radar_btn = buttons.iter().find(|b| b.id == "RADAR_TASKS_BUTTON").expect("RADAR_TASKS_BUTTON should exist");
    assert_eq!(radar_btn.parent_states, vec![GameState::Base, GameState::Area]);
    assert_eq!(radar_btn.target_state, Some(GameState::RadarTasks));
    assert!(!radar_btn.save_cursor);

    let gifts_btn = buttons.iter().find(|b| b.id == "ALLIANCE_GIFTS_BUTTON").expect("ALLIANCE_GIFTS_BUTTON should exist");
    assert_eq!(gifts_btn.parent_states, vec![GameState::Alliance]);
    assert_eq!(gifts_btn.target_state, Some(GameState::AllianceGiftsRegular));

    let prem_btn = buttons.iter().find(|b| b.id == "ALLIANCE_GIFTS_PREMIUM_BUTTON").expect("ALLIANCE_GIFTS_PREMIUM_BUTTON should exist");
    assert_eq!(prem_btn.parent_states, vec![GameState::Alliance, GameState::AllianceGiftsRegular]);
    assert_eq!(prem_btn.target_state, Some(GameState::AllianceGiftsPremium));
}

#[test]
fn test_shortcuts_loading() {
    let shortcuts = lwsc2::core::load_shortcuts_from_config("config/states.yaml");
    assert_eq!(shortcuts.toggle_pause, "ctrl+p");
    assert_eq!(shortcuts.open_config, "ctrl+o");
    assert_eq!(shortcuts.force_detect, "ctrl+s");
    assert_eq!(shortcuts.show_help, "ctrl+h");
}
