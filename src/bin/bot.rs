//! Standalone Bot Runner in Rust.

use std::time::Duration;
use clap::Parser;
use lwsc2::core::GameState;
use lwsc2::engine::GameBot;

#[derive(Parser, Debug)]
#[command(name = "bot", about = "LWSC2 - Game State Detection Bot (Rust)")]
struct Args {
    #[arg(long, default_value = "Last War", help = "Target window title")]
    window: String,

    #[arg(long, default_value_t = 500, help = "Periodic state check interval in ms")]
    interval_ms: u64,

    #[arg(long, help = "List all detected open windows and exit")]
    list_windows: bool,

    #[arg(long, help = "List all registered game states and template assets")]
    list_states: bool,

    #[arg(long, help = "List all configured automated actions, ROIs, and active status")]
    list_actions: bool,

    #[arg(long, aliases = ["root"], help = "Define initial game state root (BASE or WORLD_MAP)")]
    root_state: Option<String>,

    #[arg(long, help = "Run in headless mode without launching the configuration GUI window")]
    headless: bool,
}

fn main() {
    let args = Args::parse();

    if args.list_windows {
        let wm = lwsc2::vision::WindowManager::new("");
        let windows = wm.list_all_windows();
        println!("\n=== Detected Open Windows ===");
        for (id, title, x, y, w, h, is_focused) in windows {
            let focus_str = if is_focused { " [FOCUSED]" } else { "" };
            println!("ID: 0x{:x} | Pos: ({}, {}) | Dim: {}x{} | Title: {}{}", id, x, y, w, h, title, focus_str);
        }
        return;
    }

    if args.list_states {
        println!("\n=== Registered Game States ===");
        for defn in &*lwsc2::core::STATE_DEFINITIONS {
            println!("State: {:<20} | Type: {:<10} | Name: {}", defn.state.name(), defn.state_type.as_str(), defn.display_name);
        }
        return;
    }

    if args.list_actions {
        let actions = lwsc2::core::load_actions_from_config("config/states.yaml").unwrap_or_default();
        println!("\n=== Configured Automated Actions ===");
        for action in &actions {
            let status = if action.enabled { "[ACTIVE]" } else { "[INACTIVE]" };
            println!(
                "Action: {:<20} {} | State: {:?} | Type: {:?} | Cooldown: {:.1}s",
                action.name, status, action.state, action.action_type, action.cooldown_s
            );
            if let Some(roi) = action.roi {
                println!("  ROI: xmin={:.2}, xmax={:.2}, ymin={:.2}, ymax={:.2}", roi.xmin, roi.xmax, roi.ymin, roi.ymax);
            }
        }
        return;
    }

    let initial_root = args.root_state.as_deref().and_then(GameState::from_str);

    let bot = GameBot::new(
        &args.window,
        Duration::from_millis(args.interval_ms),
        ".",
        true,
        true,
        initial_root,
    );
    if args.headless {
        bot.start_headless();
    } else {
        bot.start();
    }
}
