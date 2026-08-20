fn main() {
    let mut matcher = lwsc2::vision::matching::TemplateMatcher::new(".");
    let tmpl = "roi/MAIN_SHOP_BUTTON/expected.png";
    let roi = Some(( (0.87 * 1634.0) as u32, (0.0 * 1038.0) as u32, (1.0 * 1634.0) as u32, (0.09 * 1038.0) as u32 ));

    for screen_name in &["roi/BASE/screen.png", "roi/AREA/screen.png", "roi/MAIN_SHOP_BUTTON/screen.png"] {
        if std::path::Path::new(screen_name).exists() {
            let img = image::open(screen_name).unwrap().to_rgba8();
            let (w, h) = img.dimensions();
            let res_with_roi = matcher.find_match(&img, tmpl, 0.5, Some(((0.87 * w as f32) as u32, (0.0 * h as f32) as u32, w, (0.09 * h as f32) as u32)));
            let res_full = matcher.find_match(&img, tmpl, 0.5, None);
            println!("Screen: {} ({}x{}) -> in ROI: matched={}, conf={:.2}% ({}, {}) | Full screen: matched={}, conf={:.2}% ({}, {})",
                screen_name, w, h,
                res_with_roi.matched, res_with_roi.confidence * 100.0, res_with_roi.center_x, res_with_roi.center_y,
                res_full.matched, res_full.confidence * 100.0, res_full.center_x, res_full.center_y
            );
        }
    }
}
