//! Main CLI Entrypoint for LWSC2 in Rust.

use std::path::Path;
use std::time::{Duration, Instant};
use clap::Parser;
use colored::Colorize;

use lwsc2::core::{GameState, StateGraph, StateDetector, STATE_DEFINITIONS};
use lwsc2::engine::GameBot;
use lwsc2::vision::ScreenCapturer;

#[derive(Parser, Debug)]
#[command(name = "lwsc2", about = "LWSC2 - Last War Survival Bot State Engine (Rust Edition)")]
struct Args {
    #[arg(long, help = "List all open desktop windows and geometries")]
    list_windows: bool,

    #[arg(long, help = "List all registered game states and template assets")]
    list_states: bool,

    #[arg(long, help = "List all configured automated actions, ROIs, and active status")]
    list_actions: bool,

    #[arg(long, num_args = 2, value_names = ["FROM_STATE", "TO_STATE"], help = "Calculate navigation path between states")]
    path: Option<Vec<String>>,

    #[arg(long, value_name = "IMAGE_PATH", help = "Detect game state from a static image file")]
    detect_image: Option<String>,

    #[arg(long, help = "Run real-time screen capture state detection loop")]
    detect_live: bool,

    #[arg(long, default_value = "Last War", help = "Target window title for bot")]
    window: String,

    #[arg(long, aliases = ["root"], help = "Define initial game state root (BASE or WORLD_MAP)")]
    root_state: Option<String>,
}

fn list_states() {
    println!("\n{}", "=== Registered Game States ===".bold().bright_blue());
    for defn in &*STATE_DEFINITIONS {
        println!("\nState: {}", defn.state.name().green().bold());
        println!("  Display Name : {}", defn.display_name);
        println!("  State Layer  : {}", defn.state_type.as_str());
        println!("  Description  : {}", defn.description);
        println!("  Templates ({}):", defn.templates.len());
        for t in &defn.templates {
            let exists = if Path::new(t).exists() {
                "[OK]".green().bold()
            } else {
                "[MISSING]".red().bold()
            };
            println!("    {} {}", exists, t);
        }
    }
}

fn list_actions() {
    println!("\n{}", "=== Configured Automated Actions ===".bold().bright_blue());
    let actions = match lwsc2::core::load_actions_from_config("config/states.yaml") {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Failed to load actions from config/states.yaml: {}", e);
            return;
        }
    };

    if actions.is_empty() {
        println!("No actions configured in config/states.yaml.");
        return;
    }

    for action in &actions {
        let status_str = if action.enabled {
            "[ACTIVE]".green().bold()
        } else {
            "[INACTIVE]".yellow().dimmed()
        };

        println!("\nAction: {} {}", action.name.cyan().bold(), status_str);
        println!("  Description  : {}", action.description);
        println!("  Action Type  : {:?}", action.action_type);
        if let Some(state) = action.state {
            println!("  Gated State  : {}", state.name().bold());
        }
        if let Some(ref tmpl) = action.template {
            let exists = if Path::new(tmpl).exists() {
                "[OK]".green()
            } else {
                "[MISSING]".red()
            };
            println!("  Template     : {} {}", exists, tmpl);
        }
        if let Some(roi) = action.roi {
            println!(
                "  ROI Filter   : xmin={:.2}, xmax={:.2}, ymin={:.2}, ymax={:.2}",
                roi.xmin, roi.xmax, roi.ymin, roi.ymax
            );
        }
        println!("  Cooldown     : {:.1}s", action.cooldown_s);
        println!("  Min Conf.    : {:.0}%", action.min_confidence * 100.0);
        println!("  Priority     : {}", action.priority);
    }
}

fn test_path(from_str: &str, to_str: &str) {
    let start = match GameState::from_str(from_str) {
        Some(s) => s,
        None => {
            eprintln!("Invalid start state name: {}", from_str);
            return;
        }
    };

    let goal = match GameState::from_str(to_str) {
        Some(s) => s,
        None => {
            eprintln!("Invalid goal state name: {}", to_str);
            return;
        }
    };

    let graph = StateGraph::new();
    match graph.find_path(start, goal) {
        Some(path) => {
            println!("\nNavigation Path ({} -> {}):", start.name().bold(), goal.name().bold());
            if path.is_empty() {
                println!("  Already in target state (0 steps).");
                return;
            }
            for (i, edge) in path.iter().enumerate() {
                println!(
                    "  Step {}: [{}] -> [{}]",
                    i + 1,
                    edge.from_state.name().cyan(),
                    edge.to_state.name().green()
                );
                println!("          Action: {:?} ({})", edge.action_type, edge.description);
            }
        }
        None => {
            eprintln!("No path found between {} and {}!", start, goal);
        }
    }
}

fn detect_image(image_path: &str) {
    let p = Path::new(image_path);
    if !p.exists() {
        eprintln!("File not found: {}", image_path);
        return;
    }

    let img = match image::open(p) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            eprintln!("Failed to read image {}: {}", image_path, e);
            return;
        }
    };

    let mut detector = StateDetector::new(".");
    let start_t = Instant::now();
    let result = detector.detect(&img);
    let elapsed = start_t.elapsed();

    println!("\n{}", "=== Image Detection Result ===".bold().bright_blue());
    println!("Detected State : {}", result.state.name().green().bold());
    println!("Display Name   : {}", result.display_name);
    println!("Layer Type     : {}", result.state_type.as_str());
    if let Some(root) = result.root_state {
        println!("Resolved Root  : {}", root.name().cyan().bold());
    }
    println!("Confidence     : {:.2}%", result.confidence * 100.0);
    println!("Latency        : {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    if let Some(ref tmpl) = result.matched_template {
        println!("Matched Asset  : {}", tmpl);
    }
    if let Some(bbox) = result.match_box {
        println!("Match Bounding : {:?}", bbox);
    }
    if let Some(center) = result.match_center {
        println!("Match Center   : {:?}", center);
    }
}

fn detect_live() {
    println!("\n{}", "Starting live screen detection... (Press Ctrl+C to stop)".yellow());
    let capturer = ScreenCapturer::new();
    let mut detector = StateDetector::new(".");

    loop {
        let start_t = Instant::now();
        if let Some(frame) = capturer.capture_region(0, 0, 1920, 1080) {
            let result = detector.detect(&frame);
            let elapsed_ms = start_t.elapsed().as_secs_f64() * 1000.0;

            print!(
                "\rState: {:<20} | Conf: {:>6.1}% | Latency: {:>5.1}ms",
                result.state.name(),
                result.confidence * 100.0,
                elapsed_ms
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn list_windows() {
    let wm = lwsc2::vision::WindowManager::new("");
    let windows = wm.list_all_windows();
    println!("\n{}", "=== Currently Open Desktop Windows ===".bold().bright_blue());
    if windows.is_empty() {
        println!("No windows found via X11.");
        return;
    }
    for (id, title, x, y, w, h, is_focused) in windows {
        println!(
            "ID: {:<10} | Pos: ({:>4}, {:>4}) | Dim: {:>4}x{:<4} | Focus: {:<3} | Title: {}",
            format!("0x{:x}", id).cyan(),
            x, y, w, h,
            if is_focused { "YES".green() } else { "NO".dimmed() },
            title.bold()
        );
    }
}

fn main() {
    let args = Args::parse();

    if args.list_windows {
        list_windows();
    } else if args.list_states {
        list_states();
    } else if args.list_actions {
        list_actions();
    } else if let Some(ref path_args) = args.path {
        if path_args.len() == 2 {
            test_path(&path_args[0], &path_args[1]);
        }
    } else if let Some(ref img_path) = args.detect_image {
        detect_image(img_path);
    } else if args.detect_live {
        detect_live();
    } else {
        // Default: Run GameBot
        let initial_root = args.root_state.as_deref().and_then(GameState::from_str);
        let bot = GameBot::new(
            &args.window,
            Duration::from_millis(500),
            ".",
            true,
            true,
            initial_root,
        );
        bot.start();
    }
}
