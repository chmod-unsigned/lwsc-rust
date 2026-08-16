use image::open;
use lwsc2::core::{GameState, StateDetector, StateType, load_state_definitions};

#[test]
fn test_yaml_states_loading() {
    let defs = load_state_definitions("config/states.yaml")
        .expect("failed to load config/states.yaml");
    
    assert!(defs.len() >= 8, "Expected at least 8 states defined in YAML");
    
    let base_def = defs.iter().find(|d| d.state == GameState::Base).expect("Base state missing in YAML");
    assert_eq!(base_def.state_type, StateType::Root);
    assert!(base_def.roi.is_some());
    assert!(base_def.parent_states().is_empty());

    let world_def = defs.iter().find(|d| d.state == GameState::WorldMap).expect("WorldMap state missing in YAML");
    assert_eq!(world_def.state_type, StateType::Root);
    assert!(world_def.roi.is_some());

    let area_def = defs.iter().find(|d| d.state == GameState::Area).expect("Area state missing in YAML");
    assert_eq!(area_def.state_type, StateType::Root);
    assert!(area_def.roi.is_some());

    let main_shop_def = defs.iter().find(|d| d.state == GameState::MainShop).expect("MainShop missing in YAML");
    assert_eq!(main_shop_def.state_type, StateType::Modal);
    assert_eq!(main_shop_def.parent_states(), vec![GameState::Base, GameState::Area]);
}

#[test]
fn test_detect_root_assets() {
    let mut detector = StateDetector::new(".");

    let test_cases = [
        ("roi/AREA/screen.png", GameState::Area),
        ("roi/WORLD_MAP/screen.png", GameState::WorldMap),
    ];

    for (path, expected_state) in test_cases {
        if !std::path::Path::new(path).exists() {
            println!("[SKIP] {} does not exist", path);
            continue;
        }

        let img = open(path).expect("failed to open image").to_rgba8();
        let res = detector.detect(&img);
        
        println!(
            "\n[Detection Result: {}]\n  Detected State : {}\n  Expected State : {}\n  Confidence     : {:.2}%\n  Matched Template: {:?}\n  Resolved Root  : {:?}\n  Visible Buttons: {:?}",
            path,
            res.state,
            expected_state,
            res.confidence * 100.0,
            res.matched_template,
            res.root_state,
            res.visible_buttons.iter().map(|b| &b.id).collect::<Vec<_>>()
        );

        assert_eq!(res.state, expected_state, "State mismatch for image {}", path);
        assert_eq!(res.root_state, Some(expected_state), "Root mismatch for image {}", path);
        assert!(res.confidence >= 0.80);
    }
}

#[test]
fn test_detect_root_fast() {
    let mut detector = StateDetector::new(".");

    let test_cases = [
        ("roi/AREA/screen.png", GameState::Area),
        ("roi/WORLD_MAP/screen.png", GameState::WorldMap),
    ];

    for (path, expected_state) in test_cases {
        if !std::path::Path::new(path).exists() {
            continue;
        }

        let img = open(path).expect("failed to open image").to_rgba8();
        let root_res = detector.detect_root(&img);
        assert!(root_res.is_some(), "Failed detect_root on {}", path);
        let res = root_res.unwrap();
        assert_eq!(res.state, expected_state);
        assert_eq!(res.root_state, Some(expected_state));
        assert!(res.confidence >= 0.80);
    }
}
