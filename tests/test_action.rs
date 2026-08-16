use image::open;
use lwsc2::core::{
    load_actions_from_config, ActionDefinition, ActionManager, ActionType, GameState, NormalizedROI,
};
use lwsc2::vision::matching::TemplateMatcher;

#[test]
fn test_actions_yaml_loading() {
    let actions = load_actions_from_config("config/states.yaml")
        .expect("failed to load actions from config/states.yaml");

    assert!(!actions.is_empty(), "Actions list should not be empty");

    let help_action = actions.iter().find(|a| a.name == "alliance_help").expect("alliance_help missing");
    assert!(help_action.enabled, "alliance_help should be active/enabled");
    assert_eq!(help_action.button.as_deref(), Some("HELP"));
    assert_eq!(help_action.template.as_deref(), Some("roi/HELP/expected.png"));
    assert!(help_action.roi.is_some());
    assert!(help_action.save_cursor);
    assert_eq!(help_action.parent_states, vec![GameState::Base, GameState::Area]);

    let radar_action = actions.iter().find(|a| a.name == "radar_task_claim").expect("radar_task_claim missing");
    assert!(!radar_action.enabled, "radar_task_claim should be inactive/disabled");
    assert_eq!(radar_action.button.as_deref(), Some("RADAR_TASKS_BUTTON"));
    assert_eq!(radar_action.template.as_deref(), Some("roi/RADAR_TASKS_BUTTON/expected.png"));
}

#[test]
fn test_action_manager_enable_disable() {
    let actions = vec![
        ActionDefinition {
            name: "test_action_1".to_string(),
            description: "Test action 1".to_string(),
            enabled: true,
            button: None,
            state: None,
            parent_states: Vec::new(),
            action_type: ActionType::ClickTemplate,
            template: Some("roi/HELP/expected.png".to_string()),
            roi: Some(NormalizedROI::new(0.64, 0.77, 0.86, 0.98)),
            coords: None,
            key_name: None,
            min_confidence: 0.85,
            cooldown_s: 2.0,
            priority: 1,
            save_cursor: true,
            last_executed: None,
        },
        ActionDefinition {
            name: "test_action_2".to_string(),
            description: "Test action 2".to_string(),
            enabled: false,
            button: None,
            state: Some(GameState::AllianceGiftsRegular),
            parent_states: Vec::new(),
            action_type: ActionType::ClickRoi,
            template: None,
            roi: Some(NormalizedROI::new(0.50, 0.66, 0.32, 0.54)),
            coords: None,
            key_name: None,
            min_confidence: 0.85,
            cooldown_s: 5.0,
            priority: 2,
            save_cursor: false,
            last_executed: None,
        },
    ];

    let manager = ActionManager::new(actions);

    assert!(manager.is_action_enabled("test_action_1"));
    assert!(!manager.is_action_enabled("test_action_2"));

    // Toggle states
    assert!(manager.set_action_enabled("test_action_1", false));
    assert!(!manager.is_action_enabled("test_action_1"));

    assert!(manager.set_action_enabled("test_action_2", true));
    assert!(manager.is_action_enabled("test_action_2"));

    // Non-existent action returns false
    assert!(!manager.set_action_enabled("non_existent", true));
}

#[test]
fn test_action_evaluation_on_real_screen() {
    let actions = load_actions_from_config("config/states.yaml")
        .expect("failed to load actions from config/states.yaml");
    let manager = ActionManager::new(actions);

    let mut matcher = TemplateMatcher::new(".");
    let help_screen_path = "roi/HELP/screen.png";

    if std::path::Path::new(help_screen_path).exists() {
        let img = open(help_screen_path).expect("open help screen").to_rgba8();

        let results = manager.evaluate(GameState::Base, &img, &mut matcher);
        let help_res = results.iter().find(|r| r.action_name == "alliance_help");

        assert!(help_res.is_some(), "alliance_help should be evaluated");
        let res = help_res.unwrap();
        assert!(res.executed, "alliance_help should be executed on HELP screen: {}", res.reason);
        assert!(res.click_coords.is_some(), "Should provide click coordinates");

        // Subsequent evaluation immediately after should hit cooldown
        let cooldown_results = manager.evaluate(GameState::Base, &img, &mut matcher);
        let cooldown_res = cooldown_results.iter().find(|r| r.action_name == "alliance_help");
        assert!(cooldown_res.is_some());
        assert!(!cooldown_res.unwrap().executed, "Should be gated by cooldown");
    }
}

#[test]
fn test_human_path_generation_under_300ms() {
    use lwsc2::io::generate_human_path;

    // Test short distance
    let path_short = generate_human_path(100, 100, 150, 150, 250);
    assert!(!path_short.is_empty());
    let total_ms_short: u128 = path_short.iter().map(|(_, _, d)| d.as_millis()).sum();
    assert!(total_ms_short < 300, "Short path must take less than 300ms: {}ms", total_ms_short);
    assert_eq!(path_short.last().unwrap().0, 150);
    assert_eq!(path_short.last().unwrap().1, 150);

    // Test long diagonal screen distance
    let path_long = generate_human_path(50, 50, 1800, 950, 250);
    assert!(!path_long.is_empty());
    let total_ms_long: u128 = path_long.iter().map(|(_, _, d)| d.as_millis()).sum();
    assert!(total_ms_long < 300, "Long path must take less than 300ms: {}ms", total_ms_long);
    assert_eq!(path_long.last().unwrap().0, 1800);
    assert_eq!(path_long.last().unwrap().1, 950);
}
