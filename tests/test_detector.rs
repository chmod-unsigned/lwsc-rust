use image::{Rgba, RgbaImage};
use lwsc2::core::{GameState, StateDetector};

#[test]
fn test_detector_unknown_on_blank() {
    let mut detector = StateDetector::new(".");
    let blank = RgbaImage::from_pixel(300, 300, Rgba([180, 120, 70, 255]));
    let res = detector.detect(&blank);
    assert_eq!(res.state, GameState::Unknown);
    assert_eq!(res.root_state, None);
}

#[test]
fn test_detector_matches_real_template() {
    let template_path = "roi/BASE/expected.png";
    if !std::path::Path::new(template_path).exists() {
        return;
    }

    let tmpl_img = match image::open(template_path) {
        Ok(img) => img.to_rgba8(),
        Err(_) => return,
    };

    let (tw, th) = tmpl_img.dimensions();
    let screen_w = 1000;
    let screen_h = 1000;
    let mut screen = RgbaImage::from_pixel(screen_w, screen_h, Rgba([40, 40, 40, 255]));

    // Place template at bottom-right corner (within 88-100% X, 86-100% Y ROI)
    let pos_x = 900;
    let pos_y = 900;

    for y in 0..th {
        for x in 0..tw {
            let p = tmpl_img.get_pixel(x, y);
            screen.put_pixel(pos_x + x, pos_y + y, *p);
        }
    }

    let mut detector = StateDetector::new(".");
    let res = detector.detect(&screen);
    assert_eq!(res.state, GameState::Base);
    assert_eq!(res.root_state, Some(GameState::Base));
}

#[test]
fn test_detector_requires_all_multiple_templates() {
    use lwsc2::core::{StateDefinition, StateType};

    let base_tmpl_path = "roi/BASE/expected.png";
    let area_tmpl_path = "roi/AREA/expected.png";
    if !std::path::Path::new(base_tmpl_path).exists() || !std::path::Path::new(area_tmpl_path).exists() {
        return;
    }

    let base_img = image::open(base_tmpl_path).unwrap().to_rgba8();
    let area_img = image::open(area_tmpl_path).unwrap().to_rgba8();

    // Create custom state definition that requires BOTH templates
    let multi_template_state = StateDefinition {
        state: GameState::MainShop,
        state_type: StateType::Modal,
        display_name: "Multi Test Modal".to_string(),
        templates: vec![base_tmpl_path.to_string(), area_tmpl_path.to_string()],
        parent_names: vec!["BASE".to_string()],
        roi: None,
        min_confidence: 0.85,
        description: "Requires both templates".to_string(),
    };

    let mut detector = StateDetector::with_definitions(".", vec![multi_template_state], vec![]);

    // 1. Screen containing ONLY template 1 (base_img)
    let mut screen1 = RgbaImage::from_pixel(1000, 1000, Rgba([30, 30, 30, 255]));
    for y in 0..base_img.height() {
        for x in 0..base_img.width() {
            screen1.put_pixel(100 + x, 100 + y, *base_img.get_pixel(x, y));
        }
    }

    // Should NOT match because template 2 is missing!
    assert!(!detector.is_in_state(&screen1, GameState::MainShop));
    let res1 = detector.detect(&screen1);
    assert_ne!(res1.state, GameState::MainShop);

    // 2. Screen containing BOTH template 1 and template 2
    let mut screen2 = screen1.clone();
    for y in 0..area_img.height() {
        for x in 0..area_img.width() {
            screen2.put_pixel(500 + x, 500 + y, *area_img.get_pixel(x, y));
        }
    }

    // Now BOTH are present -> MUST match!
    assert!(detector.is_in_state(&screen2, GameState::MainShop));
    let res2 = detector.detect(&screen2);
    assert_eq!(res2.state, GameState::MainShop);
    assert!(res2.confidence >= 0.85);
}

#[test]
fn test_detector_matches_expected_directory() {
    let screen_path = "roi/ALLIANCE_GIFTS_REGULAR/screen.png";
    if !std::path::Path::new(screen_path).exists() {
        return;
    }

    let img = image::open(screen_path).expect("open alliance gifts regular screen").to_rgba8();
    let mut detector = StateDetector::new(".");
    let res = detector.detect(&img);

    assert_eq!(res.state, GameState::AllianceGiftsRegular);
    assert_eq!(res.root_state, Some(GameState::Base));
    assert!(res.confidence >= 0.85);
}

#[test]
fn test_detector_matches_radar_tasks_expected_directory() {
    let screen_path = "roi/RADAR_TASKS/screen.png";
    if !std::path::Path::new(screen_path).exists() {
        return;
    }

    let img = image::open(screen_path).expect("open radar tasks screen").to_rgba8();
    let mut detector = StateDetector::new(".");
    let res = detector.detect(&img);

    assert_eq!(res.state, GameState::RadarTasks);
    assert_eq!(res.root_state, Some(GameState::Base));
    assert!(res.confidence >= 0.85);
}
