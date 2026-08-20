use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use eframe::egui;

static CLOSE_REQUESTED: AtomicBool = AtomicBool::new(false);

struct TestApp;
impl eframe::App for TestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        if CLOSE_REQUESTED.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            CLOSE_REQUESTED.store(false, Ordering::SeqCst);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello!");
        });
    }
}

fn main() {
    let handle = thread::spawn(|| {
        let mut options = eframe::NativeOptions::default();
        #[cfg(target_os = "linux")]
        {
            options.event_loop_builder = Some(Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                EventLoopBuilderExtX11::with_any_thread(builder, true);
                EventLoopBuilderExtWayland::with_any_thread(builder, true);
            }));
        }
        let _ = eframe::run_native("Test", options, Box::new(|_cc| Box::new(TestApp)));
    });

    thread::sleep(std::time::Duration::from_secs(2));
    println!("Requesting close...");
    CLOSE_REQUESTED.store(true, Ordering::SeqCst);
    
    handle.join().unwrap();
    println!("Thread joined!");
}
