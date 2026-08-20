use eframe::egui;

fn main() {
    let mut options = eframe::NativeOptions::default();
    
    #[cfg(target_os = "linux")]
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::unix::EventLoopBuilderExtUnix;
            builder.with_any_thread(true);
        }));
    }
}
