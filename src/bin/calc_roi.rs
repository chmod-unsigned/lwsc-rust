//! CLI Tool to automatically compute and write normalized ROIs in `config/states.yaml` and `config/buttons.yaml`
//! from state & button directories containing screenshots and expected/template images.

use std::fs;
use std::path::{Path, PathBuf};
use clap::Parser;
use colored::Colorize;
use image::open;

use lwsc2::core::{load_state_definitions, NormalizedROI};
use lwsc2::vision::matching::TemplateMatcher;

#[derive(Parser, Debug)]
#[command(
    name = "calc_roi",
    about = "Calculate normalized ROIs from screenshot/expected images and update states.yaml / buttons.yaml"
)]
struct Args {
    #[arg(
        default_value = "roi",
        help = "Directory containing state/button subfolders (e.g. roi/MAIN_SHOP/ or roi/HELP/)"
    )]
    dir: String,

    #[arg(
        long,
        default_value_t = 0.05,
        help = "Safety margin to add around detected template box (e.g. 0.05 for 5%)"
    )]
    margin: f32,

    #[arg(
        long,
        default_value_t = 0.65,
        help = "Minimum template matching confidence threshold (0.0 to 1.0)"
    )]
    min_confidence: f32,

    #[arg(
        long,
        help = "Optional custom YAML file path to update instead of searching default config/ files"
    )]
    config: Option<String>,

    #[arg(
        long,
        help = "Apply and write calculated ROIs directly to config/states.yaml and config/buttons.yaml"
    )]
    apply: bool,

    #[arg(
        long,
        help = "Filter to only process one specific state/button name (case-insensitive)"
    )]
    state: Option<String>,
}

#[derive(Debug)]
struct MatchResult {
    item_name: String,
    screen_path: PathBuf,
    template_path: PathBuf,
    image_width: u32,
    image_height: u32,
    box_x: u32,
    box_y: u32,
    box_w: u32,
    box_h: u32,
    confidence: f32,
    exact_box: (f32, f32, f32, f32), // (xmin, xmax, ymin, ymax)
    roi: NormalizedROI,
}

fn find_screenshot(dir: &Path) -> Option<PathBuf> {
    let screen_names = [
        "screen.png",
        "screen.jpg",
        "screen.jpeg",
        "screenshot.png",
        "screenshot.jpg",
        "screenshot.jpeg",
        "screen_1.png",
    ];

    for name in &screen_names {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }

    // Fallback: look for any png/jpg file with 'screen' in name
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_lowercase();
                if (lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg"))
                    && lower.contains("screen")
                {
                    return Some(p);
                }
            }
        }
    }

    None
}

fn find_expected_templates(dir: &Path, screen_path: Option<&Path>) -> Vec<PathBuf> {
    let mut templates = Vec::new();

    // 1. Check if 'expected/' subdirectory exists
    let exp_dir = dir.join("expected");
    if exp_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&exp_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    let lower = ext.to_lowercase();
                    if lower == "png" || lower == "jpg" || lower == "jpeg" {
                        templates.push(p);
                    }
                }
            }
        }
        templates.sort();
        if !templates.is_empty() {
            return templates;
        }
    }

    // 2. Check files starting with expected/template/target in dir
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if screen_path == Some(p.as_path()) {
                continue;
            }
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_lowercase();
                if (lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg"))
                    && (lower.starts_with("expected") || lower.starts_with("template") || lower.starts_with("target"))
                {
                    templates.push(p);
                }
            }
        }
    }

    templates.sort();

    if !templates.is_empty() {
        return templates;
    }

    // 3. Fallback: collect any image that is not the screen
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if screen_path == Some(p.as_path()) {
                continue;
            }
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let lower_ext = ext.to_lowercase();
                if lower_ext == "png" || lower_ext == "jpg" || lower_ext == "jpeg" {
                    templates.push(p);
                }
            }
        }
    }

    templates.sort();
    templates
}

fn update_single_yaml_file(yaml_path: &str, results: &[MatchResult]) -> Result<usize, Box<dyn std::error::Error>> {
    if !Path::new(yaml_path).exists() {
        return Ok(0);
    }

    let content = fs::read_to_string(yaml_path)?;
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut updated_count = 0;

    for res in results {
        let item_name = &res.item_name;
        let mut entry_start = None;
        let mut entry_end = None;
        let mut entry_indent = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check map key format: `NAME:` or `"NAME":`
            let is_map_key = if let Some(colon_pos) = line.find(':') {
                let key_part = line[..colon_pos].trim().trim_matches('"').trim_matches('\'');
                let after_colon = line[colon_pos + 1..].trim();
                key_part.eq_ignore_ascii_case(item_name) && (after_colon.is_empty() || after_colon.starts_with('#') || after_colon.starts_with('{'))
            } else {
                false
            };

            // Check list format: `- state: NAME` or `- id: NAME` or `- name: NAME`
            let is_list_item = if trimmed.starts_with("- state:") || trimmed.starts_with("- id:") || trimmed.starts_with("- name:") {
                let val = trimmed.split(':').nth(1).unwrap_or("").trim().trim_matches('"').trim_matches('\'');
                val.eq_ignore_ascii_case(item_name)
            } else {
                false
            };

            if is_map_key || is_list_item {
                entry_start = Some(i);
                entry_indent = line.len() - line.trim_start().len();

                // Find end of entry block
                let mut j = i + 1;
                while j < lines.len() {
                    let next_line = &lines[j];
                    let next_trimmed = next_line.trim();
                    if next_trimmed.is_empty() || next_trimmed.starts_with('#') {
                        j += 1;
                        continue;
                    }
                    let next_indent = next_line.len() - next_line.trim_start().len();
                    if next_indent <= entry_indent {
                        break;
                    }
                    j += 1;
                }
                entry_end = Some(j);
                break;
            }
        }

        let (start_idx, end_idx) = match (entry_start, entry_end) {
            (Some(s), Some(e)) => (s, e),
            _ => continue,
        };

        // Locate existing roi line(s) within the entry block
        let mut roi_start = None;
        let mut roi_end = None;

        let mut k = start_idx;
        while k < end_idx {
            let trimmed = lines[k].trim();
            if trimmed.starts_with("roi:") {
                roi_start = Some(k);
                if trimmed == "roi:" {
                    // Multi-line roi block
                    let mut m = k + 1;
                    while m < end_idx {
                        let inner_trim = lines[m].trim();
                        if inner_trim.starts_with("xmin:")
                            || inner_trim.starts_with("xmax:")
                            || inner_trim.starts_with("ymin:")
                            || inner_trim.starts_with("ymax:")
                        {
                            m += 1;
                        } else {
                            break;
                        }
                    }
                    roi_end = Some(m);
                } else {
                    roi_end = Some(k + 1);
                }
                break;
            }
            k += 1;
        }

        let indent_str = " ".repeat(entry_indent + 2);
        let new_roi_line = format!(
            "{}roi: {{ xmin: {:.2}, xmax: {:.2}, ymin: {:.2}, ymax: {:.2} }}",
            indent_str, res.roi.xmin, res.roi.xmax, res.roi.ymin, res.roi.ymax
        );

        if let (Some(rs), Some(re)) = (roi_start, roi_end) {
            lines.splice(rs..re, vec![new_roi_line]);
        } else {
            // Insert roi before min_confidence, description, or at end
            let mut insert_pos = end_idx;
            for idx in start_idx..end_idx {
                let trimmed = lines[idx].trim();
                if trimmed.starts_with("min_confidence:") || trimmed.starts_with("description:") {
                    insert_pos = idx;
                    break;
                }
            }
            lines.insert(insert_pos, new_roi_line);
        }

        updated_count += 1;
    }

    if updated_count > 0 {
        let mut new_content = lines.join("\n");
        if !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        fs::write(yaml_path, new_content)?;
    }

    Ok(updated_count)
}

fn round_norm(val: f32) -> f32 {
    (val * 100.0).round() / 100.0
}

fn main() {
    let args = Args::parse();
    let root_path = Path::new(&args.dir);

    println!(
        "\n{}",
        "=== LWSC2 State & Button ROI Calculator ==="
            .bold()
            .bright_cyan()
    );
    println!("Base search directory : {}", args.dir.bold());
    println!("Safety margin         : {:.1}%", args.margin * 100.0);
    println!("Min confidence        : {:.1}%\n", args.min_confidence * 100.0);

    if !root_path.exists() {
        eprintln!(
            "{} Directory '{}' does not exist.",
            "[ERROR]".red().bold(),
            args.dir
        );
        std::process::exit(1);
    }

    let config_targets = if let Some(ref custom_cfg) = args.config {
        vec![custom_cfg.clone()]
    } else {
        vec![
            "config/states.yaml".to_string(),
            "config/buttons.yaml".to_string(),
            "config/actions.yaml".to_string(),
        ]
    };

    // Load states configuration for fallback template lookups
    let state_defs = load_state_definitions("config/states.yaml").unwrap_or_default();

    let mut matcher = TemplateMatcher::new(".");
    let mut computed_results = Vec::new();

    let entries = match fs::read_dir(root_path) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("Failed to read directory {}: {}", args.dir, err);
            std::process::exit(1);
        }
    };

    let mut state_dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();

    state_dirs.sort();

    if state_dirs.is_empty() {
        println!(
            "{} No subdirectories found in '{}'.",
            "[INFO]".yellow().bold(),
            args.dir
        );
        return;
    }

    let total_start = std::time::Instant::now();

    for state_dir in &state_dirs {
        let folder_name = state_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if let Some(ref filter_state) = args.state {
            if !folder_name.eq_ignore_ascii_case(filter_state) {
                continue;
            }
        }

        let state_start = std::time::Instant::now();

        println!(
            "{} Checking asset folder: {}",
            "▶".bright_blue().bold(),
            folder_name.bold().green()
        );

        let screen_opt = find_screenshot(state_dir);
        let screen_path = match screen_opt {
            Some(p) => p,
            None => {
                println!(
                    "  {} No screenshot found in {} (looked for screen.png, screenshot.png...)",
                    "[SKIP]".yellow(),
                    state_dir.display()
                );
                continue;
            }
        };

        let mut template_paths = find_expected_templates(state_dir, Some(&screen_path));

        // Fallback to configured templates in states.yaml if no template in folder
        if template_paths.is_empty() {
            if let Some(def) = state_defs.iter().find(|d| d.state.name().eq_ignore_ascii_case(folder_name)) {
                for tmpl in &def.templates {
                    let p = PathBuf::from(tmpl);
                    if p.exists() {
                        template_paths.push(p);
                    }
                }
            }
        }

        if template_paths.is_empty() {
            println!(
                "  {} No expected/template image found in {}",
                "[SKIP]".yellow(),
                state_dir.display()
            );
            continue;
        }

        let screen_img = match open(&screen_path) {
            Ok(img) => img.to_rgba8(),
            Err(e) => {
                println!(
                    "  {} Failed to open screen image {}: {}",
                    "[ERROR]".red(),
                    screen_path.display(),
                    e
                );
                continue;
            }
        };

        let (w, h) = screen_img.dimensions();

        let mut all_matched = true;
        let mut min_confidence = 1.0f32;
        let mut combined_xmin = 1.0f32;
        let mut combined_xmax = 0.0f32;
        let mut combined_ymin = 1.0f32;
        let mut combined_ymax = 0.0f32;
        let mut first_match = None;

        for tmpl_path in &template_paths {
            let tmpl_str = tmpl_path.to_string_lossy();
            let res = matcher.find_match(&screen_img, &tmpl_str, args.min_confidence, None);
            if !res.matched {
                all_matched = false;
                break;
            }

            if res.confidence < min_confidence {
                min_confidence = res.confidence;
            }

            let xmin = (res.box_x as f32 / w as f32).clamp(0.0, 1.0);
            let xmax = ((res.box_x + res.width) as f32 / w as f32).clamp(0.0, 1.0);
            let ymin = (res.box_y as f32 / h as f32).clamp(0.0, 1.0);
            let ymax = ((res.box_y + res.height) as f32 / h as f32).clamp(0.0, 1.0);

            if xmin < combined_xmin { combined_xmin = xmin; }
            if xmax > combined_xmax { combined_xmax = xmax; }
            if ymin < combined_ymin { combined_ymin = ymin; }
            if ymax > combined_ymax { combined_ymax = ymax; }

            if first_match.is_none() {
                first_match = Some((res, tmpl_path.clone()));
            }
        }

        let best_match = if all_matched && !template_paths.is_empty() {
            let roi_xmin = round_norm((combined_xmin - args.margin).max(0.0));
            let roi_xmax = round_norm((combined_xmax + args.margin).min(1.0));
            let roi_ymin = round_norm((combined_ymin - args.margin).max(0.0));
            let roi_ymax = round_norm((combined_ymax + args.margin).min(1.0));

            let (fm_res, fm_path) = first_match.unwrap();
            let display_tmpl_path = if template_paths.len() == 1 {
                fm_path
            } else {
                PathBuf::from(format!("{} ({} templates)", state_dir.join("expected").display(), template_paths.len()))
            };

            Some(MatchResult {
                item_name: folder_name.to_string(),
                screen_path: screen_path.clone(),
                template_path: display_tmpl_path,
                image_width: w,
                image_height: h,
                box_x: fm_res.box_x,
                box_y: fm_res.box_y,
                box_w: fm_res.width,
                box_h: fm_res.height,
                confidence: min_confidence,
                exact_box: (combined_xmin, combined_xmax, combined_ymin, combined_ymax),
                roi: NormalizedROI::new(roi_ymin, roi_ymax, roi_xmin, roi_xmax),
            })
        } else {
            None
        };

        let state_elapsed = state_start.elapsed();

        match best_match {
            Some(res) => {
                println!("  Screen Image : {} ({}x{} px)", res.screen_path.display(), res.image_width, res.image_height);
                println!("  Matched Asset: {}", res.template_path.display().to_string().cyan());
                println!(
                    "  Confidence   : {}",
                    format!("{:.2}%", res.confidence * 100.0).bold().green()
                );
                println!(
                    "  Pixel Box    : x={}, y={}, w={}, h={}",
                    res.box_x, res.box_y, res.box_w, res.box_h
                );
                println!(
                    "  Exact Bounds : xmin={:.3}, xmax={:.3}, ymin={:.3}, ymax={:.3}",
                    res.exact_box.0, res.exact_box.1, res.exact_box.2, res.exact_box.3
                );
                println!(
                    "  Suggested ROI: {} xmin={:.2}, xmax={:.2}, ymin={:.2}, ymax={:.2}",
                    "✓".green().bold(),
                    res.roi.xmin,
                    res.roi.xmax,
                    res.roi.ymin,
                    res.roi.ymax
                );
                println!("  Elapsed Time : {:.2}ms", state_elapsed.as_secs_f64() * 1000.0);
                println!();
                computed_results.push(res);
            }
            None => {
                println!(
                    "  {} Template matching failed for all candidate templates in {} (threshold: {:.0}%, took {:.2}ms)",
                    "[NO MATCH]".red().bold(),
                    folder_name,
                    args.min_confidence * 100.0,
                    state_elapsed.as_secs_f64() * 1000.0
                );
                println!();
            }
        }
    }

    let total_elapsed = total_start.elapsed();

    println!("{}", "===============================================".bright_black());
    println!(
        "Summary: Processed {} asset directory(ies), computed {} ROI(s) in {:.2}ms.",
        state_dirs.len(),
        computed_results.len().to_string().bold().green(),
        total_elapsed.as_secs_f64() * 1000.0
    );

    if computed_results.is_empty() {
        return;
    }

    if args.apply {
        println!("\nApplying updated ROIs to configuration files...");
        let mut total_applied = 0;
        for target in &config_targets {
            match update_single_yaml_file(target, &computed_results) {
                Ok(count) => {
                    if count > 0 {
                        println!(
                            "  {} Updated {} definitions in {}",
                            "[SUCCESS]".green().bold(),
                            count,
                            target.bold()
                        );
                        total_applied += count;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "  {} Failed to update {}: {}",
                        "[ERROR]".red().bold(),
                        target,
                        e
                    );
                }
            }
        }
        println!(
            "\n{} Total definitions updated: {} across configuration files.\n",
            "[COMPLETE]".green().bold(),
            total_applied
        );
    } else {
        println!(
            "\n{} Run with '{}' or '{}' to write these ROIs directly to YAML config files.\n",
            "[DRY-RUN]".yellow().bold(),
            "--apply".bold().cyan(),
            "make calc-roi-apply".bold().cyan()
        );
    }
}
