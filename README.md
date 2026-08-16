# LWSC2 - Last War Survival GameBot (Rust Edition)

High-performance, state-aware automation bot and game state detection engine written in 100% Rust.

## Quick Start

### 1. Launch the Bot
```bash
./run.sh
```
or with cargo directly:
```bash
cargo run --release --bin bot
```

### Options:
- `--window "Custom Title"`: specify target window title (default: `"Last War"`)
- `--interval-ms 500`: set state check interval in ms (default: `500ms`)
- `--root-state BASE` (or `WORLD_MAP`): explicitly define the initial Game State Root

---

## What Happens When Run:

1. **Independent Window Tracking Thread**: Continuously monitors the position `(X, Y)` and dimensions `(Width x Height)` of the game window.
2. **Strict Bounded Capture**: Screen capturing is strictly limited to the game window rectangle.
3. **Independent State Detector & Root Tracker**: Runs in background and is **triggered immediately upon every mouse click** (both bot clicks and manual clicks) and on periodic intervals. Maintains and resolves the active root screen (`BASE` or `WORLD_MAP`).
4. **Instant Hierarchical Identification**: Evaluates templates across state layers (Popups $\to$ Sub-Modals $\to$ Modals $\to$ Root screens) and maps any modal back to its root game state.
5. **Displays Startup State Banner**:
   ```text
   ====================================================================
     LWSC2 - Last War Survival GameBot Initialized (Rust Edition)
     [Window Tracker Thread: ACTIVE] [State Detector Thread: ACTIVE]
   ====================================================================
    [Game Window Geometry & Position]
      Status       : FOUND (wmctrl, Window ID: 0x03800007)
      Position     : X=240 px, Y=110 px
      Dimensions   : 800 x 1280 px (Width x Height)
      Center Point : (640, 750)
      Aspect Ratio : 5:8
   --------------------------------------------------------------------
    [Startup Game State]
      Root State   : BASE
      State Name   : BASE
      Display Name : Base (Headquarter)
      Layer        : root
      Confidence   : 95.40%
      Matched By   : roi/BASE/expected.png
   ====================================================================
   Bot is active: Continuous detection on ANY mouse or keyboard event. Press Ctrl+C to exit.

   [State Change (mouse_click)] BASE ➔ WORLD_MAP
   [State Change (keyboard)] WORLD_MAP ➔ SEARCH
   ```

---

## Global Shortcuts & Hotkeys

The bot listens continuously for global hotkeys across the entire desktop:

| Shortcut | Description |
| :--- | :--- |
| **`Ctrl+O`** | **Open / Toggle Native Configuration & Action Manager Window** |
| **`Ctrl+S`** | Force an immediate manual State Detection pass |
| **`Ctrl+H`** | Print active shortcuts & help to the terminal |
| **`Ctrl+P`** | Pause / Resume state detection activity |
| **`Ctrl+C`** | Graceful stop and clean exit |

### Interactive Configuration Window (`Ctrl+O`)
Pressing `Ctrl+O` opens a native X11 configuration panel allowing you to:
- Inspect active Game State, Root State, and Game Window geometry in real-time.
- Toggle automated ROI-gated actions **Active / Inactive** with a mouse click or keys `[1-9]`.
- Press **`S`** to save the modified actions back to `config/states.yaml`.
- Press **`R`** to reload configuration.
- Press **`Esc`** or **`Ctrl+O`** to close the window.

---

## CLI Tools & Commands

- **List Configured Actions & ROIs**:
  ```bash
  cargo run -- --list-actions
  ```
- **List All States**:
  ```bash
  cargo run -- --list-states
  ```
- **Inspect Static Image**:
  ```bash
  cargo run -- --detect-image path/to/image.png
  ```
- **Calculate Navigation Path**:
  ```bash
  cargo run -- --path BASE SEARCH_SPECIAL
  ```
- **Calculate & Calibrate State ROIs**:
  ```bash
  cargo run --release --bin calc_roi -- roi
  ```
- **Run Test Suite**:
  ```bash
  cargo run -- --list-actions
  cargo test
  ```

---

## Project Structure

```
lwsc2/
├── Cargo.toml
├── config/
│   └── states.yaml            # Game states, ROIs, and parent hierarchy
├── roi/                       # Per-state expected templates & screenshots
├── src/
│   ├── lib.rs                 # Crate root
│   ├── main.rs                # CLI entrypoint
│   ├── bin/
│   │   └── bot.rs             # Standalone GameBot runner
│   ├── core/
│   │   ├── state.rs           # GameState enum & StateDefinition registry
│   │   ├── state_graph.rs     # Navigation graph & Dijkstra pathfinding
│   │   ├── detector.rs        # Hierarchical state detector
│   │   └── state_thread.rs    # Independent click-triggered StateDetectorThread
│   ├── vision/
│   │   ├── matching.rs        # Fast alpha-masked TemplateMatcher
│   │   ├── window.rs          # Window manager & geometry inspector
│   │   ├── window_tracker.rs  # Independent WindowTracker thread
│   │   └── screen.rs          # Strictly bounded screen capturer (X11)
│   ├── engine/
│   │   └── bot.rs             # GameBot engine orchestrating threads
│   └── io/
│       └── input.rs           # Click dispatcher & mouse event interceptor
├── tests/
│   ├── test_core.rs
│   └── test_detector.rs
└── run.sh                     # Runner shortcut
```
