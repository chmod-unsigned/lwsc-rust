use image::open;
use lwsc2::vision::matching::TemplateMatcher;

#[test]
fn test_inspect_modal_roi() {
    let mut matcher = TemplateMatcher::new(".");

    let cases = [
        ("roi/MAIN_SHOP_HOT_DEALS/screen.png", "roi/MAIN_SHOP_HOT_DEALS/expected.png", "HOT_DEALS selected"),
        ("roi/MAIN_SHOP_MALL/screen.png", "roi/MAIN_SHOP_MALL/expected.png", "MALL selected"),
    ];

    for (img_path, tmpl_path, label) in cases {
        if !std::path::Path::new(img_path).exists() {
            println!("[SKIP] {}", img_path);
            continue;
        }

        let img = open(img_path).expect("open image").to_rgba8();
        let (w, h) = img.dimensions();

        let res = matcher.find_match(&img, tmpl_path, 0.70, None);
        println!("\n=== Inspection for {} ===", label);
        println!("  Image size   : {} x {}", w, h);
        println!("  Matched      : {}", res.matched);
        println!("  Confidence   : {:.2}%", res.confidence * 100.0);
        println!("  Pixel Box    : x={}, y={}, w={}, h={}", res.box_x, res.box_y, res.width, res.height);
        
        let xmin = (res.box_x as f32 / w as f32).max(0.0);
        let xmax = ((res.box_x + res.width) as f32 / w as f32).min(1.0);
        let ymin = (res.box_y as f32 / h as f32).max(0.0);
        let ymax = ((res.box_y + res.height) as f32 / h as f32).min(1.0);

        println!("  Exact Norm Box: xmin={:.3}, xmax={:.3}, ymin={:.3}, ymax={:.3}", xmin, xmax, ymin, ymax);
        
        // Recommended generous ROI with margin
        let margin_x = 0.05;
        let margin_y = 0.05;
        println!("  Suggested ROI : xmin={:.2}, xmax={:.2}, ymin={:.2}, ymax={:.2}", 
            (xmin - margin_x).max(0.0), 
            (xmax + margin_x).min(1.0), 
            (ymin - margin_y).max(0.0), 
            (ymax + margin_y).min(1.0)
        );
    }
}
