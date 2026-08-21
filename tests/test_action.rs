use image::open;
use lwsc2::core::{
    load_actions_from_config, ActionDefinition, ActionManager, ActionType, GameState, NormalizedROI, SequenceDefinition, SequenceStep,
};
use lwsc2::vision::matching::TemplateMatcher;

#[test]
fn test_actions_yaml_loading() {
    let actions = load_actions_from_config("config/states.yaml")
        .expect("failed to load actions from config/states.yaml");

    assert!(!actions.is_empty(), "Actions list should not be empty");

    let help_action = actions.iter().find(|a| a.name == "alliance_help").expect("alliance_help missing");
    assert!(help_action.enabled, "alliance_help should be active/enabled");
    assert_eq!(help_action.button.as_deref(), Some("HELP_BUTTON"));
    assert_eq!(help_action.template.as_deref(), Some("roi/HELP/expected.png"));
    assert!(help_action.roi.is_some());
    assert!(help_action.save_cursor);
    assert_eq!(help_action.parent_states, vec![GameState::Base, GameState::Area]);
    assert_eq!(help_action.shortcut.as_deref(), Some("ctrl+1"));

    let gift_action = actions.iter().find(|a| a.name == "alliance_gift_claim").expect("alliance_gift_claim missing");
    assert_eq!(gift_action.shortcut.as_deref(), Some("ctrl+2"));

    let radar_action = actions.iter().find(|a| a.name == "radar_task_claim").expect("radar_task_claim missing");
    assert!(!radar_action.enabled, "radar_task_claim should be inactive/disabled");
    assert_eq!(radar_action.button.as_deref(), Some("RADAR_TASKS_BUTTON"));
    assert_eq!(radar_action.template.as_deref(), Some("roi/RADAR_TASKS_BUTTON/expected.png"));

    let loot_action = actions.iter().find(|a| a.name == "loot_claim").expect("loot_claim missing");
    assert!(loot_action.enabled, "loot_claim should be active/enabled");
    assert_eq!(loot_action.button.as_deref(), Some("LOOT_BUTTON"));
    assert_eq!(loot_action.shortcut.as_deref(), Some("ctrl+3"));
    assert_eq!(loot_action.parent_states, vec![GameState::Base, GameState::Area]);

    let loot_claim_all_action = actions.iter().find(|a| a.name == "loot_claim_all").expect("loot_claim_all missing");
    assert!(loot_claim_all_action.enabled, "loot_claim_all should be active/enabled");
    assert_eq!(loot_claim_all_action.button.as_deref(), Some("LOOT_CLAIM_ALL_BUTTON"));
    assert_eq!(loot_claim_all_action.shortcut.as_deref(), Some("ctrl+4"));
    assert_eq!(loot_claim_all_action.parent_states, vec![GameState::Loot]);

    let gold_mine_action = actions.iter().find(|a| a.name == "search_gold_mine").expect("search_gold_mine missing");
    assert_eq!(gold_mine_action.action_type, ActionType::ClickTemplate);
    assert_eq!(gold_mine_action.template.as_deref(), Some("poi/gold_mine.png"));
}

#[test]
fn test_sequences_yaml_loading() {
    let sequences = lwsc2::core::load_sequences_from_config("config/states.yaml")
        .expect("load sequences");
    assert_eq!(sequences.len(), 3);
    let names: Vec<String> = sequences.iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"morning_routine".to_string()));
    assert!(names.contains(&"search_gold_mine".to_string()));
    assert!(names.contains(&"sequence_loot_claim".to_string()));

    let morning = sequences.iter().find(|s| s.name == "morning_routine").unwrap();
    assert_eq!(morning.steps.len(), 4);
    assert_eq!(morning.steps[0].action, "loot_claim");
    assert_eq!(morning.steps[1].action, "loot_claim_all");
    assert_eq!(morning.steps[2].action, "main_shop");
    assert_eq!(morning.steps[3].action, "main_shop_mall_hot_sale_packs_claim");
}

#[test]
fn test_action_manager_enable_disable() {
    let actions = vec![
        ActionDefinition {
            name: "test_action_1".to_string(),
            display_name: "Test action 1".to_string(),
            description: "Test action 1".to_string(),
            enabled: true,
            button: None,
            state: None,
            parent_states: Vec::new(),
            action_type: ActionType::ClickTemplate,
            template: Some("roi/HELP/expected.png".to_string()),
            click_template: None,
            roi: Some(NormalizedROI::new(0.64, 0.77, 0.86, 0.98)),
            coords: None,
            drag_start: None,
            drag_end: None,
            drag_duration_ms: 1000,
            key_name: None,
            min_confidence: 0.85,
            cooldown_s: 2.0,
            priority: 1,
            save_cursor: true,
            shortcut: Some("ctrl+1".to_string()),
            script: None,
            args: Vec::new(),
            last_executed: None,
        },
        ActionDefinition {
            name: "test_action_2".to_string(),
            display_name: "Test action 2".to_string(),
            description: "Test action 2".to_string(),
            enabled: false,
            button: None,
            state: Some(GameState::AllianceGiftsRegular),
            parent_states: Vec::new(),
            action_type: ActionType::ClickRoi,
            template: None,
            click_template: None,
            roi: Some(NormalizedROI::new(0.50, 0.66, 0.32, 0.54)),
            coords: None,
            drag_start: None,
            drag_end: None,
            drag_duration_ms: 1000,
            key_name: None,
            min_confidence: 0.85,
            cooldown_s: 5.0,
            priority: 2,
            save_cursor: false,
            shortcut: None,
            script: None,
            args: Vec::new(),
            last_executed: None,
        },
    ];

    let manager = ActionManager::from_actions(actions);

    assert_eq!(manager.get_shortcuts(), vec![("action:test_action_1".to_string(), "ctrl+1".to_string())]);

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
    let manager = ActionManager::from_actions(actions);

    let mut matcher = TemplateMatcher::new(".");
    let help_screen_path = "roi/HELP/screen.png";

    if std::path::Path::new(help_screen_path).exists() {
        let img = open(help_screen_path).expect("open help screen").to_rgba8();

        // Keep only alliance_help enabled to make evaluate instant in debug profile
        {
            let mut list = manager.actions.write().unwrap();
            for a in list.iter_mut() {
                if a.name != "alliance_help" {
                    a.enabled = false;
                }
            }
        }

        let results = manager.evaluate(GameState::Base, &img, &mut matcher);
        let help_res = results.iter().find(|r| r.action_name == "alliance_help");

        assert!(help_res.is_some(), "alliance_help should be evaluated");
        let res = help_res.unwrap();
        assert!(res.executed, "alliance_help should be executed on HELP screen: {}", res.reason);

        // Subsequent evaluation immediately after should hit cooldown
        let cooldown_results = manager.evaluate(GameState::Base, &img, &mut matcher);
        let cooldown_res = cooldown_results.iter().find(|r| r.action_name == "alliance_help");
        assert!(cooldown_res.is_some());
        let cd_res = cooldown_res.unwrap();
        assert!(!cd_res.executed, "Should be gated by cooldown");
    }
}

#[test]
fn test_action_skipped_when_parent_state_does_not_match() {
    let actions = load_actions_from_config("config/states.yaml")
        .expect("failed to load actions from config/states.yaml");
    let manager = ActionManager::from_actions(actions);
    let mut matcher = TemplateMatcher::new(".");

    let help_screen_path = "roi/HELP/screen.png";
    if std::path::Path::new(help_screen_path).exists() {
        let img = open(help_screen_path).expect("open help screen").to_rgba8();

        // alliance_help has parent_states: [BASE, AREA]
        // If current state is GameState::MainShop or GameState::Alliance, it must NOT execute!
        let results_shop = manager.evaluate(GameState::MainShop, &img, &mut matcher);
        let help_res_shop = results_shop.iter().find(|r| r.action_name == "alliance_help");
        assert!(help_res_shop.is_none(), "alliance_help must not execute in MainShop state");

        let results_alliance = manager.evaluate(GameState::Alliance, &img, &mut matcher);
        let help_res_alliance = results_alliance.iter().find(|r| r.action_name == "alliance_help");
        assert!(help_res_alliance.is_none(), "alliance_help must not execute in Alliance state");
    }
}

#[test]
fn test_execute_single_action_manual_shortcut() {
    let actions = load_actions_from_config("config/states.yaml")
        .expect("failed to load actions from config/states.yaml");
    let manager = ActionManager::from_actions(actions);
    let mut matcher = TemplateMatcher::new(".");

    let help_screen_path = "roi/HELP/screen.png";
    if std::path::Path::new(help_screen_path).exists() {
        let img = open(help_screen_path).expect("open help screen").to_rgba8();

        let res = manager.execute_single_action("alliance_help", GameState::Base, &img, &mut matcher, true, true);
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(res.executed, "alliance_help should execute on manual shortcut: {}", res.reason);
        assert!(res.click_coords.is_some());
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

#[test]
fn test_select_click_match() {
    use lwsc2::core::select_click_match;

    let matches = vec![
        ("roi/LOOT_BUTTON/expected/01_icon.png".to_string(), 101),
        ("roi/LOOT_BUTTON/expected/02_claim_button.png".to_string(), 202),
        ("roi/LOOT_BUTTON/expected/03_badge.png".to_string(), 303),
    ];

    // 1. None returns first
    assert_eq!(select_click_match(&matches, None), Some(&101));

    // 2. Target by filename
    assert_eq!(select_click_match(&matches, Some("02_claim_button.png")), Some(&202));

    // 3. Target by filename without extension
    assert_eq!(select_click_match(&matches, Some("02_claim_button")), Some(&202));

    // 4. Target by full path
    assert_eq!(select_click_match(&matches, Some("roi/LOOT_BUTTON/expected/03_badge.png")), Some(&303));

    // 5. Unknown target falls back to first
    assert_eq!(select_click_match(&matches, Some("non_existent.png")), Some(&101));
}

#[test]
fn test_action_manager_reload_updates_cooldown() {
    let actions = load_actions_from_config("config/states.yaml").expect("load actions");
    let manager = ActionManager::from_actions(actions);

    let loot_act = manager.list_actions().into_iter().find(|a| a.name == "loot_claim").expect("loot_claim exists");
    assert_eq!(loot_act.cooldown_s, 60.0);

    // Now call reload_from_yaml
    let res = manager.reload_from_yaml("config/states.yaml");
    assert!(res.is_ok(), "reload_from_yaml must succeed");

    let reloaded_loot = manager.list_actions().into_iter().find(|a| a.name == "loot_claim").expect("loot_claim exists");
    assert_eq!(reloaded_loot.cooldown_s, 60.0);
}

#[test]
fn test_sequence_step_timeout_advances_to_next_step() {
    let mut act1 = ActionDefinition::new("step1_fails", "Step 1 Fails");
    act1.state = Some(GameState::AllianceGiftsPremium); // Won't match current state Base
    
    let mut act2 = ActionDefinition::new("step2_succeeds", "Step 2 Succeeds");
    act2.action_type = ActionType::ClickCoords;
    act2.coords = Some((0.5, 0.5));

    let seq = SequenceDefinition {
        name: "test_resilient_seq".to_string(),
        description: "Test sequence resilience".to_string(),
        enabled: true,
        shortcut: None,
        schedules: None,
        repeat: false,
        steps: vec![
            SequenceStep { action: "step1_fails".to_string(), timeout_s: 0.01 },
            SequenceStep { action: "step2_succeeds".to_string(), timeout_s: 5.0 },
        ],
    };

    let manager = ActionManager::new(vec![act1, act2], vec![seq]);
    assert!(manager.trigger_sequence("test_resilient_seq"));
    assert!(manager.has_active_sequence());

    let mut matcher = TemplateMatcher::new(".");
    let dummy_screen = image::RgbaImage::new(100, 100);

    // Wait for step 1 to time out (15ms > 10ms timeout)
    std::thread::sleep(std::time::Duration::from_millis(20));

    // First evaluation: step 1 timed out -> should advance to step 2 without aborting sequence
    let res1 = manager.evaluate_sequence(GameState::Base, &dummy_screen, &mut matcher);
    assert!(res1.is_none());
    assert!(manager.has_active_sequence(), "Sequence must still be active after step 1 timeout!");

    // Second evaluation: step 2 executes successfully
    let res2 = manager.evaluate_sequence(GameState::Base, &dummy_screen, &mut matcher);
    assert!(res2.is_some(), "Step 2 should execute");
    let r2 = res2.unwrap();
    assert!(r2.executed, "Step 2 must execute successfully: {}", r2.reason);
    assert_eq!(r2.action_name, "step2_succeeds");
}
