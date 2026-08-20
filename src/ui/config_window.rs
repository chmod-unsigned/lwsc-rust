//! Native eframe/egui Configuration & Action Manager Window (Ctrl+O).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use eframe::egui;

use crate::core::action::{ActionManager, SequenceDefinition};
use crate::core::state_thread::StateDetectorThread;
use crate::vision::window_tracker::WindowTracker;

static IS_EVENT_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);
static TOGGLE_VISIBILITY: AtomicBool = AtomicBool::new(false);

pub struct ConfigWindow;

impl ConfigWindow {
    /// Opens the configuration window in a background thread if not already open.
    pub fn open_or_toggle(
        action_manager: Arc<ActionManager>,
        state_thread: StateDetectorThread,
        window_tracker: WindowTracker,
    ) -> Option<JoinHandle<()>> {
        if IS_EVENT_LOOP_RUNNING.load(Ordering::SeqCst) {
            TOGGLE_VISIBILITY.store(true, Ordering::SeqCst);
            return None;
        }
        IS_EVENT_LOOP_RUNNING.store(true, Ordering::SeqCst);

        let handle = thread::Builder::new()
            .name("EguiConfigWindowThread".to_string())
            .spawn(move || {
                struct WindowGuard;
                impl Drop for WindowGuard {
                    fn drop(&mut self) {
                        IS_EVENT_LOOP_RUNNING.store(false, Ordering::SeqCst);
                    }
                }
                let _guard = WindowGuard;

                let mut options = eframe::NativeOptions {
                    viewport: egui::ViewportBuilder::default()
                        .with_inner_size([720.0, 540.0])
                        .with_title("LWSC2 - Configuration & Action Manager"),
                    ..Default::default()
                };

                #[cfg(target_os = "linux")]
                {
                    options.event_loop_builder = Some(Box::new(|builder| {
                        use winit::platform::x11::EventLoopBuilderExtX11;
                        use winit::platform::wayland::EventLoopBuilderExtWayland;
                        EventLoopBuilderExtX11::with_any_thread(builder, true);
                        EventLoopBuilderExtWayland::with_any_thread(builder, true);
                    }));
                }

                // No need to instantiate app here anymore, we do it in the Box::new closure

                let result = eframe::run_native(
                    "LWSC2 Configuration",
                    options,
                    Box::new(|cc| {
                        // Apply a custom GTK-like light theme
                        let mut visuals = eframe::egui::Visuals::light();
                        visuals.window_rounding = 8.0.into();
                        visuals.menu_rounding = 8.0.into();
                        visuals.widgets.noninteractive.rounding = 4.0.into();
                        visuals.widgets.inactive.rounding = 4.0.into();
                        visuals.widgets.hovered.rounding = 4.0.into();
                        visuals.widgets.active.rounding = 4.0.into();
                        
                        // Subtle gray background like standard desktop windows
                        visuals.window_fill = eframe::egui::Color32::from_rgb(245, 245, 245);
                        visuals.panel_fill = eframe::egui::Color32::from_rgb(245, 245, 245);
                        
                        cc.egui_ctx.set_visuals(visuals);
                        
                        Box::new(Lwsc2ConfigApp::new(action_manager, state_thread, window_tracker))
                    }),
                );
                println!("[ConfigWindow] eframe::run_native finished with result: {:?}", result);
            })
            .ok();

        handle
    }

    pub fn is_open() -> bool {
        IS_EVENT_LOOP_RUNNING.load(Ordering::SeqCst)
    }
}

#[derive(PartialEq)]
enum ConfigTab {
    Dashboard,
    Actions,
    Sequences,
}

// removed unused import

struct Lwsc2ConfigApp {
    action_manager: Arc<ActionManager>,
    state_thread: StateDetectorThread,
    window_tracker: WindowTracker,
    notification_msg: String,
    notification_expire: std::time::Instant,
    current_tab: ConfigTab,
    is_visible: bool,
    sequences: Vec<SequenceDefinition>,
    actions: Vec<crate::core::action::ActionDefinition>,
}

impl Lwsc2ConfigApp {
    fn new(
        action_manager: Arc<ActionManager>,
        state_thread: StateDetectorThread,
        window_tracker: WindowTracker,
    ) -> Self {
        let sequences = action_manager.list_sequences();
        let actions = action_manager.list_actions();
        Self {
            action_manager,
            state_thread,
            window_tracker,
            notification_msg: String::new(),
            notification_expire: std::time::Instant::now(),
            current_tab: ConfigTab::Dashboard,
            is_visible: true,
            sequences,
            actions,
        }
    }

    fn notify(&mut self, msg: impl Into<String>) {
        self.notification_msg = msg.into();
        self.notification_expire = std::time::Instant::now() + std::time::Duration::from_secs(3);
    }
}

impl eframe::App for Lwsc2ConfigApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        if TOGGLE_VISIBILITY.load(Ordering::SeqCst) {
            self.is_visible = !self.is_visible;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.is_visible));
            TOGGLE_VISIBILITY.store(false, Ordering::SeqCst);
        }

        if ctx.input(|i| i.viewport().close_requested()) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.is_visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Save to states.yaml").clicked() {
                    // Propagate edits to ActionManager
                    for seq in &self.sequences {
                        self.action_manager.update_sequence(seq.clone());
                    }
                    for act in &self.actions {
                        self.action_manager.update_action(act.clone());
                    }
                    if save_config_to_yaml(&self.action_manager, "config/states.yaml") {
                        self.notify("Saved configuration to states.yaml");
                    } else {
                        self.notify("Error saving to states.yaml");
                    }
                }
                if ui.button("Reload config").clicked() {
                    if self.action_manager.reload_from_yaml("config/states.yaml").is_ok() {
                        self.sequences = self.action_manager.list_sequences();
                        self.actions = self.action_manager.list_actions();
                        self.notify("Reloaded config from states.yaml");
                    } else {
                        self.notify("Error reloading states.yaml");
                    }
                }
                ui.add_space(20.0);
                if !self.notification_msg.is_empty() && std::time::Instant::now() < self.notification_expire {
                    ui.label(egui::RichText::new(&self.notification_msg).color(egui::Color32::GREEN));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LWSC2 BOT CONFIGURATION");
            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, ConfigTab::Dashboard, "Dashboard");
                ui.selectable_value(&mut self.current_tab, ConfigTab::Actions, "Actions");
                ui.selectable_value(&mut self.current_tab, ConfigTab::Sequences, "Sequences");
            });
            ui.separator();

            match self.current_tab {
                ConfigTab::Dashboard => {
                    let win_info = self.window_tracker.get_window_info();
                    let current_state = self.state_thread.get_current_state();
                    let current_root = self.state_thread.get_current_root_state();

                    ui.group(|ui| {
                        ui.label(format!("Target Window: {} (Focused: {})", win_info.title, win_info.is_focused));
                        ui.label(format!("Game State: {:?}", current_state));
                        ui.label(format!("Root State: {:?}", current_root));
                    });
                }
                ConfigTab::Actions => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Automated Actions");
                        for action in self.actions.iter_mut() {
                            let label = if !action.display_name.is_empty() {
                                format!("Action: {} ({})", action.display_name, action.name)
                            } else {
                                format!("Action: {}", action.name)
                            };
                            egui::CollapsingHeader::new(label)
                                .id_source(format!("action_header_{}", action.name))
                                .default_open(false)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut action.enabled, "Enabled");
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Display Name:");
                                        ui.text_edit_singleline(&mut action.display_name);
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Description:");
                                        ui.text_edit_singleline(&mut action.description);
                                    });
                                    ui.separator();
                                    
                                    ui.horizontal(|ui| {
                                        ui.label("Action Type:");
                                        egui::ComboBox::from_id_source(format!("type_{}", action.name))
                                            .selected_text(action.action_type.as_str())
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::ClickTemplate, "Click Template");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::ClickCoords, "Click Coords");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::ClickRoi, "Click ROI");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::KeyPress, "Key Press");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::DragDrop, "Drag & Drop");
                                                ui.selectable_value(&mut action.action_type, crate::core::action::ActionType::Custom, "Custom");
                                            });
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Cooldown (s):");
                                        ui.add(egui::DragValue::new(&mut action.cooldown_s).speed(0.1).clamp_range(0.0..=600.0));
                                        ui.label("Priority:");
                                        ui.add(egui::DragValue::new(&mut action.priority).speed(1).clamp_range(1..=10));
                                    });
                                    
                                    if action.action_type == crate::core::action::ActionType::DragDrop {
                                        ui.horizontal(|ui| {
                                            ui.label("Drag Start (X, Y):");
                                            let mut start_x = action.drag_start.map(|c| c.0).unwrap_or(0.5);
                                            let mut start_y = action.drag_start.map(|c| c.1).unwrap_or(0.5);
                                            ui.add(egui::DragValue::new(&mut start_x).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut start_y).speed(0.01).clamp_range(0.0..=1.0));
                                            action.drag_start = Some((start_x, start_y));
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Drag End (X, Y):");
                                            let mut end_x = action.drag_end.map(|c| c.0).unwrap_or(0.5);
                                            let mut end_y = action.drag_end.map(|c| c.1).unwrap_or(0.5);
                                            ui.add(egui::DragValue::new(&mut end_x).speed(0.01).clamp_range(0.0..=1.0));
                                            ui.add(egui::DragValue::new(&mut end_y).speed(0.01).clamp_range(0.0..=1.0));
                                            action.drag_end = Some((end_x, end_y));
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Duration (ms):");
                                            ui.add(egui::DragValue::new(&mut action.drag_duration_ms).speed(10.0).clamp_range(100..=10000));
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("Scan POI Path:");
                                            let mut t = action.template.clone().unwrap_or_default();
                                            if ui.text_edit_singleline(&mut t).changed() {
                                                if t.is_empty() {
                                                    action.template = None;
                                                } else {
                                                    action.template = Some(t);
                                                }
                                            }
                                        });
                                    }
                                    
                                    ui.horizontal(|ui| {
                                        ui.label("Min Confidence:");
                                        ui.add(egui::DragValue::new(&mut action.min_confidence).speed(0.01).clamp_range(0.1..=1.0));
                                        ui.checkbox(&mut action.save_cursor, "Save Cursor");
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Shortcut:");
                                        let mut shortcut = action.shortcut.clone().unwrap_or_default();
                                        if ui.text_edit_singleline(&mut shortcut).changed() {
                                            if shortcut.is_empty() {
                                                action.shortcut = None;
                                            } else {
                                                action.shortcut = Some(shortcut);
                                            }
                                        }
                                    });
                                });
                        }
                        ui.add_space(10.0);
                        if ui.button("+ Add Action").clicked() {
                            self.actions.push(crate::core::action::ActionDefinition::new(
                                &format!("new_action_{}", self.actions.len()),
                                "New Action",
                            ));
                        }
                    });
                }
                ConfigTab::Sequences => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Automated Sequences");
                        for seq in self.sequences.iter_mut() {
                            egui::CollapsingHeader::new(format!("Sequence: {}", seq.name))
                                .default_open(false)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut seq.enabled, "Enabled");
                                    });
                                    if !seq.description.is_empty() {
                                        ui.label(egui::RichText::new(&seq.description).italics());
                                    }
                                    ui.separator();
                                    
                                    let mut step_to_remove = None;
                                    let available_actions: Vec<String> = self.actions.iter().map(|a| a.name.clone()).collect();
                                    
                                    for (step_idx, step) in seq.steps.iter_mut().enumerate() {
                                        ui.group(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(format!("Step {}: ", step_idx + 1));
                                                egui::ComboBox::from_id_source(format!("{}_step_{}", seq.name, step_idx))
                                                    .selected_text(&step.action)
                                                    .width(150.0)
                                                    .show_ui(ui, |ui| {
                                                        for act_name in &available_actions {
                                                            ui.selectable_value(&mut step.action, act_name.clone(), act_name);
                                                        }
                                                    });
                                                ui.label("Timeout (s):");
                                                ui.add(egui::DragValue::new(&mut step.timeout_s).speed(0.1).clamp_range(0.1..=100.0));
                                                if ui.button("🗑").on_hover_text("Remove Step").clicked() {
                                                    step_to_remove = Some(step_idx);
                                                }
                                            });
                                        });
                                    }
                                    if let Some(idx) = step_to_remove {
                                        seq.steps.remove(idx);
                                    }
                                    if ui.button("+ Add Step").clicked() {
                                        let default_action = available_actions.first().cloned().unwrap_or_else(|| "unknown".to_string());
                                        seq.steps.push(crate::core::action::SequenceStep {
                                            action: default_action,
                                            timeout_s: 5.0,
                                        });
                                    }
                                    
                                    ui.separator();
                                    ui.heading("Schedules");
                                    if seq.schedules.is_none() {
                                        if ui.button("+ Add Schedules").clicked() {
                                            seq.schedules = Some(crate::core::action::SequenceSchedules {
                                                every_day: None, monday: None, tuesday: None, wednesday: None,
                                                thursday: None, friday: None, saturday: None, sunday: None,
                                            });
                                        }
                                    } else {
                                        if ui.button("🗑 Remove All Schedules").clicked() {
                                            seq.schedules = None;
                                        } else if let Some(schedules) = &mut seq.schedules {
                                            let days = [
                                                ("every_day", &mut schedules.every_day),
                                                ("monday", &mut schedules.monday),
                                                ("tuesday", &mut schedules.tuesday),
                                                ("wednesday", &mut schedules.wednesday),
                                                ("thursday", &mut schedules.thursday),
                                                ("friday", &mut schedules.friday),
                                                ("saturday", &mut schedules.saturday),
                                                ("sunday", &mut schedules.sunday),
                                            ];
                                            for (day_name, day_opt) in days {
                                                ui.horizontal(|ui| {
                                                    ui.label(format!("{}: ", day_name));
                                                    if day_opt.is_none() {
                                                        if ui.button("+").clicked() {
                                                            *day_opt = Some(vec!["12:00".to_string()]);
                                                        }
                                                    } else if let Some(day_vec) = day_opt {
                                                        let mut to_remove = None;
                                                        for (i, time_str) in day_vec.iter_mut().enumerate() {
                                                            ui.add(egui::TextEdit::singleline(time_str).desired_width(50.0));
                                                            if ui.button("x").clicked() {
                                                                to_remove = Some(i);
                                                            }
                                                        }
                                                        if let Some(idx) = to_remove {
                                                            day_vec.remove(idx);
                                                        }
                                                        if ui.button("+").clicked() {
                                                            day_vec.push("12:00".to_string());
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                });
                        }
                    });
                }
            }
        });
    }
}

fn save_config_to_yaml(manager: &ActionManager, yaml_path: &str) -> bool {
    let content = match std::fs::read_to_string(yaml_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let mut cfg: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let updated_actions = manager.list_actions();
    if let Some(map) = cfg.as_mapping_mut() {
        if let Some(actions_seq) = map.get_mut(&serde_yaml::Value::String("actions".to_string())).and_then(|v| v.as_sequence_mut()) {
            for action_val in actions_seq.iter_mut() {
                if let Some(action_map) = action_val.as_mapping_mut() {
                    let name_opt = action_map.get(&serde_yaml::Value::String("name".to_string()))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    if let Some(name) = name_opt {
                        if let Some(act_def) = updated_actions.iter().find(|a| a.name.eq_ignore_ascii_case(&name)) {
                            action_map.insert(
                                serde_yaml::Value::String("display_name".to_string()),
                                serde_yaml::Value::String(act_def.display_name.clone()),
                            );
                            action_map.insert(
                                serde_yaml::Value::String("description".to_string()),
                                serde_yaml::Value::String(act_def.description.clone()),
                            );
                            action_map.insert(
                                serde_yaml::Value::String("enabled".to_string()),
                                serde_yaml::Value::Bool(act_def.enabled),
                            );
                            action_map.insert(
                                serde_yaml::Value::String("cooldown_s".to_string()),
                                serde_yaml::Value::Number(serde_yaml::Number::from(act_def.cooldown_s as f64)),
                            );
                            action_map.insert(
                                serde_yaml::Value::String("priority".to_string()),
                                serde_yaml::Value::Number(serde_yaml::Number::from(act_def.priority)),
                            );
                            action_map.insert(
                                serde_yaml::Value::String("min_confidence".to_string()),
                                serde_yaml::Value::Number(serde_yaml::Number::from(act_def.min_confidence as f64)),
                            );
                            action_map.insert(
                                serde_yaml::Value::String("action_type".to_string()),
                                serde_yaml::Value::String(act_def.action_type.as_str().to_string()),
                            );
                            if let Some((sx, sy)) = act_def.drag_start {
                                action_map.insert(
                                    serde_yaml::Value::String("drag_start".to_string()),
                                    serde_yaml::Value::Sequence(vec![
                                        serde_yaml::Value::Number(serde_yaml::Number::from(sx as f64)),
                                        serde_yaml::Value::Number(serde_yaml::Number::from(sy as f64)),
                                    ]),
                                );
                            } else {
                                action_map.remove(&serde_yaml::Value::String("drag_start".to_string()));
                            }
                            if let Some((ex, ey)) = act_def.drag_end {
                                action_map.insert(
                                    serde_yaml::Value::String("drag_end".to_string()),
                                    serde_yaml::Value::Sequence(vec![
                                        serde_yaml::Value::Number(serde_yaml::Number::from(ex as f64)),
                                        serde_yaml::Value::Number(serde_yaml::Number::from(ey as f64)),
                                    ]),
                                );
                            } else {
                                action_map.remove(&serde_yaml::Value::String("drag_end".to_string()));
                            }
                            action_map.insert(
                                serde_yaml::Value::String("drag_duration_ms".to_string()),
                                serde_yaml::Value::Number(serde_yaml::Number::from(act_def.drag_duration_ms)),
                            );
                            if let Some(t) = &act_def.template {
                                action_map.insert(
                                    serde_yaml::Value::String("template".to_string()),
                                    serde_yaml::Value::String(t.clone()),
                                );
                            }
                            action_map.insert(
                                serde_yaml::Value::String("save_cursor".to_string()),
                                serde_yaml::Value::Bool(act_def.save_cursor),
                            );
                            if let Some(shortcut) = &act_def.shortcut {
                                action_map.insert(
                                    serde_yaml::Value::String("shortcut".to_string()),
                                    serde_yaml::Value::String(shortcut.clone()),
                                );
                            } else {
                                action_map.remove(&serde_yaml::Value::String("shortcut".to_string()));
                            }
                        }
                    }
                }
            }
            
            // Append new actions that are in updated_actions but not in YAML
            let existing_names: Vec<String> = actions_seq.iter().filter_map(|val| {
                val.as_mapping().and_then(|m| m.get(&serde_yaml::Value::String("name".to_string())))
                   .and_then(|v| v.as_str()).map(|s| s.to_lowercase())
            }).collect();
            
            for act_def in &updated_actions {
                if !existing_names.contains(&act_def.name.to_lowercase()) {
                    let mut new_map = serde_yaml::Mapping::new();
                    new_map.insert(serde_yaml::Value::String("name".to_string()), serde_yaml::Value::String(act_def.name.clone()));
                    new_map.insert(serde_yaml::Value::String("display_name".to_string()), serde_yaml::Value::String(act_def.display_name.clone()));
                    new_map.insert(serde_yaml::Value::String("description".to_string()), serde_yaml::Value::String(act_def.description.clone()));
                    new_map.insert(serde_yaml::Value::String("enabled".to_string()), serde_yaml::Value::Bool(act_def.enabled));
                    new_map.insert(serde_yaml::Value::String("cooldown_s".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(act_def.cooldown_s as f64)));
                    new_map.insert(serde_yaml::Value::String("priority".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(act_def.priority)));
                    new_map.insert(serde_yaml::Value::String("min_confidence".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(act_def.min_confidence as f64)));
                    new_map.insert(serde_yaml::Value::String("save_cursor".to_string()), serde_yaml::Value::Bool(act_def.save_cursor));
                    new_map.insert(serde_yaml::Value::String("action_type".to_string()), serde_yaml::Value::String(act_def.action_type.as_str().to_string()));
                    if let Some((sx, sy)) = act_def.drag_start {
                        new_map.insert(serde_yaml::Value::String("drag_start".to_string()), serde_yaml::Value::Sequence(vec![
                            serde_yaml::Value::Number(serde_yaml::Number::from(sx as f64)),
                            serde_yaml::Value::Number(serde_yaml::Number::from(sy as f64)),
                        ]));
                    }
                    if let Some((ex, ey)) = act_def.drag_end {
                        new_map.insert(serde_yaml::Value::String("drag_end".to_string()), serde_yaml::Value::Sequence(vec![
                            serde_yaml::Value::Number(serde_yaml::Number::from(ex as f64)),
                            serde_yaml::Value::Number(serde_yaml::Number::from(ey as f64)),
                        ]));
                    }
                    new_map.insert(serde_yaml::Value::String("drag_duration_ms".to_string()), serde_yaml::Value::Number(serde_yaml::Number::from(act_def.drag_duration_ms)));
                    if let Some(t) = &act_def.template {
                        new_map.insert(serde_yaml::Value::String("template".to_string()), serde_yaml::Value::String(t.clone()));
                    }
                    if let Some(shortcut) = &act_def.shortcut {
                        new_map.insert(serde_yaml::Value::String("shortcut".to_string()), serde_yaml::Value::String(shortcut.clone()));
                    }
                    actions_seq.push(serde_yaml::Value::Mapping(new_map));
                }
            }
        }
        
        let updated_sequences = manager.list_sequences();
        if let Some(sequences_seq) = map.get_mut(&serde_yaml::Value::String("sequences".to_string())).and_then(|v| v.as_sequence_mut()) {
            for seq_val in sequences_seq.iter_mut() {
                if let Some(seq_map) = seq_val.as_mapping_mut() {
                    let name_opt = seq_map.get(&serde_yaml::Value::String("name".to_string()))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    if let Some(name) = name_opt {
                        if let Some(seq_def) = updated_sequences.iter().find(|s| s.name.eq_ignore_ascii_case(&name)) {
                            seq_map.insert(
                                serde_yaml::Value::String("enabled".to_string()),
                                serde_yaml::Value::Bool(seq_def.enabled),
                            );
                            
                            // Rebuild steps
                            let mut new_steps_seq = serde_yaml::Sequence::new();
                            for new_step in &seq_def.steps {
                                let mut step_map = serde_yaml::Mapping::new();
                                step_map.insert(
                                    serde_yaml::Value::String("action".to_string()),
                                    serde_yaml::Value::String(new_step.action.clone()),
                                );
                                step_map.insert(
                                    serde_yaml::Value::String("timeout_s".to_string()),
                                    serde_yaml::Value::Number(serde_yaml::Number::from(new_step.timeout_s as f64)),
                                );
                                new_steps_seq.push(serde_yaml::Value::Mapping(step_map));
                            }
                            seq_map.insert(
                                serde_yaml::Value::String("steps".to_string()),
                                serde_yaml::Value::Sequence(new_steps_seq)
                            );
                            
                            // Rebuild schedules
                            if let Some(schedules) = &seq_def.schedules {
                                let mut sched_map = serde_yaml::Mapping::new();
                                
                                let days = [
                                    ("every_day", &schedules.every_day),
                                    ("monday", &schedules.monday),
                                    ("tuesday", &schedules.tuesday),
                                    ("wednesday", &schedules.wednesday),
                                    ("thursday", &schedules.thursday),
                                    ("friday", &schedules.friday),
                                    ("saturday", &schedules.saturday),
                                    ("sunday", &schedules.sunday),
                                ];
                                
                                for (day_name, day_opt) in days.iter() {
                                    if let Some(day_vec) = day_opt {
                                        if !day_vec.is_empty() {
                                            let mut day_seq = serde_yaml::Sequence::new();
                                            for time_str in day_vec.iter() {
                                                day_seq.push(serde_yaml::Value::String(time_str.clone()));
                                            }
                                            sched_map.insert(
                                                serde_yaml::Value::String(day_name.to_string()),
                                                serde_yaml::Value::Sequence(day_seq)
                                            );
                                        }
                                    }
                                }
                                
                                if !sched_map.is_empty() {
                                    seq_map.insert(
                                        serde_yaml::Value::String("schedules".to_string()),
                                        serde_yaml::Value::Mapping(sched_map)
                                    );
                                } else {
                                    seq_map.remove(&serde_yaml::Value::String("schedules".to_string()));
                                }
                            } else {
                                seq_map.remove(&serde_yaml::Value::String("schedules".to_string()));
                            }
                        }
                    }
                }
            }
        }

        if let Ok(updated_str) = serde_yaml::to_string(&cfg) {
            return std::fs::write(yaml_path, updated_str).is_ok();
        }
    }
    false
}
